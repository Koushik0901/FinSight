use crate::error::{AppError, AppResult};
use crate::ApiState;
use chrono::{Datelike, Utc};
use finsight_core::repos::{budgets, goals, run};
use serde::{Deserialize, Serialize};
use specta::Type;

// ── Budget ─────────────────────────────────────────────────────────────────

/// One category's budget + actual for a month.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetEnvelope {
    pub category_id: String,
    pub category_label: String,
    pub category_color: String,
    pub group_label: String,
    /// Budget set by user for the current month (0 = not budgeted this month)
    pub budget_cents: i64,
    /// Actual outflow this month (positive = spent)
    pub spent_cents: i64,
    /// Running (budgeted − spent) carried in from prior months, anchored at the
    /// category's first-ever budgeted month. Positive = unspent rolling forward,
    /// negative = accumulated overspend.
    pub carryover_cents: i64,
    pub txn_count: i64,
}

/// Budgets joined with actual spend per category for `month`/`month_start`
/// (`month_start` = `"{month}-01"`). A `settle_up = 1` reimbursement inflow
/// nets against the category's spend (matching metrics.rs cashflow) instead of
/// being silently dropped by an `amount_cents < 0`-only filter. Extracted from
/// [`list_budget_envelopes`] so it's directly unit-testable without a Tauri
/// `AppState`.
fn budget_envelopes_for_month(
    conn: &mut rusqlite::Connection,
    month: &str,
    month_start: &str,
) -> finsight_core::CoreResult<Vec<BudgetEnvelope>> {
    // Get budgets for the month
    let budget_map: std::collections::HashMap<String, i64> =
        budgets::list_for_month(conn, month)?.into_iter().collect();

    // Get spending per category this month
    let mut stmt = conn.prepare(
        "SELECT c.id, c.label, COALESCE(c.color,''), COALESCE(g.label,''), \
                COALESCE(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                  WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                  ELSE 0 END), 0), \
                COUNT(t.id) \
         FROM categories c \
         LEFT JOIN category_groups g ON g.id = c.group_id \
         LEFT JOIN transactions t ON t.category_id = c.id AND t.posted_at >= ?1 \
         WHERE c.archived_at IS NULL \
         GROUP BY c.id, c.label, c.color, c.group_id, g.label \
         ORDER BY g.sort_order, c.sort_order",
    )?;
    let rows = stmt.query_map(rusqlite::params![month_start], |r| {
        let cat_id: String = r.get(0)?;
        Ok((cat_id.clone(), r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, budget_map.get(&cat_id).copied().unwrap_or(0)))
    })?;
    // Collect + drop the statement before the loop: carryover_into_month needs
    // the connection, which `stmt` borrows.
    let rows: Vec<_> = rows.collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    let mut out = Vec::new();
    for (cat_id, label, color, group_label, spent, txn_count, budget) in rows {
        if !finsight_core::categorize::is_budgetable_category(&cat_id) {
            continue;
        }
        let carryover_cents = budgets::carryover_into_month(conn, &cat_id, month)?;
        // Every active budgetable category is shown, budgeted or not — a category
        // with no budget and no spend yet is exactly the one a user needs to see
        // in order to budget it for the first time.
        out.push(BudgetEnvelope {
            category_id: cat_id,
            category_label: label,
            category_color: color,
            group_label,
            budget_cents: budget,
            spent_cents: spent,
            carryover_cents,
            txn_count,
        });
    }
    Ok(out)
}

pub async fn list_budget_envelopes(state: &ApiState) -> AppResult<Vec<BudgetEnvelope>> {
    let db = (*state.db).clone();
    let now = Utc::now();
    let month = now.format("%Y-%m").to_string();
    let this_month_start = now.format("%Y-%m-01").to_string();

    run(&db, move |conn| {
        budget_envelopes_for_month(conn, &month, &this_month_start)
    })
    .await
    .map_err(AppError::from)
}

pub async fn set_budget(
    state: &ApiState,
    category_id: String,
    amount_cents: i64,
) -> AppResult<()> {
    let db = (*state.db).clone();
    let month = Utc::now().format("%Y-%m").to_string();
    run(&db, move |conn| {
        budgets::set(conn, &category_id, &month, amount_cents)
    })
    .await
    .map_err(AppError::from)
}

// ── Goals ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GoalDto {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub current_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPlanRow {
    pub category_id: String,
    pub label: String,
    pub color: String,
    pub group_label: String,
    pub budget_cents: i64,
    pub m0_cents: i64,
    pub m1_cents: i64,
    pub m2_cents: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanData {
    pub income_cents: i64,
    pub categories: Vec<CategoryPlanRow>,
    pub goals: Vec<GoalDto>,
    pub sinking_funds: Vec<GoalDto>,
    pub recurring_expense_cents: i64,
    pub look_back: Vec<budgets::LookBackFact>,
}

#[derive(Debug, Clone, serde::Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanAssignment {
    pub category_id: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyActual {
    pub month: String,
    pub label: String,
    pub spent_cents: i64,
    pub budgeted_cents: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryHistory {
    pub category_id: String,
    pub label: String,
    pub color: String,
    pub monthly: Vec<MonthlyActual>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewGoalInput {
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub account_id: Option<String>,
}

fn goal_to_dto(g: goals::Goal) -> GoalDto {
    GoalDto {
        id: g.id,
        name: g.name,
        goal_type: g.goal_type,
        target_cents: g.target_cents,
        current_cents: g.current_cents,
        monthly_cents: g.monthly_cents,
        target_date: g.target_date,
        color: g.color,
        notes: g.notes,
        purpose: g.purpose,
        sort_order: g.sort_order,
        created_at: g.created_at,
        account_id: g.account_id,
    }
}

pub async fn list_goals(state: &ApiState) -> AppResult<Vec<GoalDto>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        goals::list(conn).map(|gs| gs.into_iter().map(goal_to_dto).collect())
    })
    .await
    .map_err(AppError::from)
}

pub async fn create_goal(state: &ApiState, input: NewGoalInput) -> AppResult<GoalDto> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        goals::insert(
            conn,
            goals::NewGoal {
                name: input.name,
                goal_type: input.goal_type,
                target_cents: input.target_cents,
                monthly_cents: input.monthly_cents,
                target_date: input.target_date,
                color: input.color,
                notes: input.notes,
                purpose: input.purpose,
                account_id: input.account_id,
            },
        )
        .map(goal_to_dto)
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedValue {
    pub years: i32,
    pub value_cents: i64,
    pub annual_rate: f64,
}

pub async fn project_goal_growth(
    state: &ApiState,
    goal_id: String,
    years: i32,
) -> AppResult<ProjectedValue> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        use finsight_core::repos::accounts;
        let goal = goals::get_by_id(conn, &goal_id)?;
        // Default long-run return comes from the shared, user-tunable assumption
        // (7% unless changed); a linked account's own APY overrides it.
        let default_rate =
            finsight_core::metrics::assumptions(conn).expected_annual_return_pct / 100.0;
        let annual_rate = if let Some(account_id) = &goal.account_id {
            accounts::get_by_id(conn, account_id)
                .ok()
                .and_then(|a| a.apy_pct)
                .map(|apy| apy / 100.0)
                .unwrap_or(default_rate)
        } else {
            default_rate
        };
        let value_cents = if years <= 0 {
            goal.current_cents.max(0)
        } else {
            let r = annual_rate / 12.0;
            let n = (years * 12) as i32;
            let growth = f64::powi(1.0 + r, n);
            // The current balance compounds too — the old formula projected only
            // the contribution stream and dropped the starting principal.
            let fv_present = goal.current_cents.max(0) as f64 * growth;
            // Future value of the monthly-contribution annuity. Guard r == 0
            // (e.g. a 0% APY account) which would otherwise divide by zero.
            let fv_contrib = if goal.monthly_cents > 0 {
                if r.abs() < f64::EPSILON {
                    goal.monthly_cents as f64 * n as f64
                } else {
                    goal.monthly_cents as f64 * ((growth - 1.0) / r)
                }
            } else {
                0.0
            };
            (fv_present + fv_contrib).round() as i64
        };
        Ok(ProjectedValue {
            years,
            value_cents,
            annual_rate,
        })
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GoalContributionDto {
    pub id: String,
    pub goal_id: String,
    pub amount_cents: i64,
    pub note: Option<String>,
    pub source: String,
    pub created_at: String,
}

fn contribution_to_dto(c: goals::GoalContribution) -> GoalContributionDto {
    GoalContributionDto {
        id: c.id,
        goal_id: c.goal_id,
        amount_cents: c.amount_cents,
        note: c.note,
        source: c.source,
        created_at: c.created_at,
    }
}

/// Reject contributions to account-linked goals — their balance is derived from
/// the linked account, so a manual contribution would be overwritten on the next
/// balance sync (the double-count bug this whole ledger exists to prevent).
fn ensure_manual_goal(conn: &mut rusqlite::Connection, id: &str) -> finsight_core::CoreResult<()> {
    let goal = goals::get_by_id(conn, id)?;
    if goal.account_id.is_some() {
        return Err(finsight_core::CoreError::InvalidState(
            "This goal tracks a linked account's balance — adjust the account, not the goal."
                .to_string(),
        ));
    }
    Ok(())
}

pub async fn update_goal_balance(
    state: &ApiState,
    id: String,
    current_cents: i64,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        ensure_manual_goal(conn, &id)?;
        let goal = goals::get_by_id(conn, &id)?;
        let delta = current_cents - goal.current_cents;
        if delta != 0 {
            goals::add_contribution(conn, &id, delta, Some("Balance adjustment"), "manual")?;
        }
        Ok(())
    })
    .await
    .map_err(AppError::from)
}

pub async fn contribute_to_goal(
    state: &ApiState,
    id: String,
    amount_cents: i64,
    note: Option<String>,
    source: Option<String>,
) -> AppResult<GoalContributionDto> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        ensure_manual_goal(conn, &id)?;
        goals::add_contribution(
            conn,
            &id,
            amount_cents,
            note.as_deref(),
            source.as_deref().unwrap_or("manual"),
        )
        .map(contribution_to_dto)
    })
    .await
    .map_err(AppError::from)
}

pub async fn list_goal_contributions(
    state: &ApiState,
    goal_id: String,
) -> AppResult<Vec<GoalContributionDto>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        goals::list_contributions(conn, &goal_id)
            .map(|list| list.into_iter().map(contribution_to_dto).collect())
    })
    .await
    .map_err(AppError::from)
}

pub async fn archive_goal(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| goals::archive(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn update_goal_monthly(
    state: &ApiState,
    id: String,
    monthly_cents: i64,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        goals::set_monthly_cents(conn, &id, monthly_cents)
    })
    .await
    .map_err(AppError::from)
}

pub async fn update_goal_purpose(
    state: &ApiState,
    id: String,
    purpose: Option<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        goals::set_purpose(conn, &id, purpose.as_deref())
    })
    .await
    .map_err(AppError::from)
}

pub async fn get_plan_next_month_data(state: &ApiState) -> AppResult<PlanData> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let now = Utc::now();
        let m0 = now.format("%Y-%m").to_string();
        // m1 = one month back
        let m1 = {
            let (yr, mo) = if now.month() == 1 { (now.year() - 1, 12u32) } else { (now.year(), now.month() - 1) };
            format!("{yr}-{mo:02}")
        };
        // m2 = two months back
        let m2 = {
            let m0i = now.month0() as i32 - 2i32;
            let (yr, mo) = if m0i < 0 {
                (now.year() - 1, (m0i + 12) as u32 + 1)
            } else {
                (now.year(), m0i as u32 + 1)
            };
            format!("{yr}-{mo:02}")
        };
        let m2_start = format!("{}-01", m2);

        // Average monthly income over last 3 months
        // settle_up = 0 excludes a reimbursement inflow from income (it nets
        // against expense instead — see metrics.rs income_expense_since).
        let income_cents: i64 = conn.query_row(
            &format!(
                "SELECT CAST(COALESCE(AVG(mi), 0) AS INTEGER)
                 FROM (SELECT SUM(amount_cents) AS mi
                       FROM transactions t
                       WHERE amount_cents > 0
                         AND settle_up = 0
                         AND is_transfer = 0
                         AND {}
                         AND strftime('%Y-%m', posted_at) IN (?1, ?2, ?3)
                       GROUP BY strftime('%Y-%m', posted_at))",
                finsight_core::metrics::non_investment_txn_predicate("t")
            ),
            rusqlite::params![m0, m1, m2],
            |r| r.get(0),
        )?;

        // Per-category spending for m0, m1, m2
        let budget_map: std::collections::HashMap<String, i64> =
            budgets::list_for_month(conn, &m0)?.into_iter().collect();

        let mut stmt = conn.prepare(
            "SELECT c.id, c.label, COALESCE(c.color,''), COALESCE(g.label,''),
                    COALESCE(SUM(CASE WHEN strftime('%Y-%m',t.posted_at)=?1 AND t.settle_up=1 THEN -t.amount_cents
                                      WHEN strftime('%Y-%m',t.posted_at)=?1 AND t.amount_cents<0 THEN -t.amount_cents
                                      ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN strftime('%Y-%m',t.posted_at)=?2 AND t.settle_up=1 THEN -t.amount_cents
                                      WHEN strftime('%Y-%m',t.posted_at)=?2 AND t.amount_cents<0 THEN -t.amount_cents
                                      ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN strftime('%Y-%m',t.posted_at)=?3 AND t.settle_up=1 THEN -t.amount_cents
                                      WHEN strftime('%Y-%m',t.posted_at)=?3 AND t.amount_cents<0 THEN -t.amount_cents
                                      ELSE 0 END),0)
             FROM categories c
             LEFT JOIN category_groups g ON g.id = c.group_id
             LEFT JOIN transactions t ON t.category_id = c.id
               AND t.posted_at >= ?4
             WHERE c.archived_at IS NULL
             GROUP BY c.id, c.label, c.color, g.label
             ORDER BY g.sort_order, c.sort_order",
        )?;
        let raw_rows: Vec<_> = stmt.query_map(rusqlite::params![m0, m1, m2, m2_start], |r| {
            let cat_id: String = r.get(0)?;
            Ok((
                cat_id.clone(),
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                budget_map.get(&cat_id).copied().unwrap_or(0),
            ))
        })?.collect::<rusqlite::Result<_>>()?;
        // Drop `stmt` (and its borrow of `conn`) before the mutable borrow below.
        drop(stmt);
        let mut categories = Vec::new();
        for (category_id, label, color, group_label, m0c, m1c, m2c, budget) in raw_rows {
            categories.push(CategoryPlanRow {
                category_id,
                label,
                color,
                group_label,
                budget_cents: budget,
                m0_cents: m0c,
                m1_cents: m1c,
                m2_cents: m2c,
            });
        }

        // Sinking funds get their own Plan-wizard step; everything else that's
        // still open (current < target) is a regular active goal.
        let all_goals = goals::list(conn)?;
        let (sinking, other): (Vec<_>, Vec<_>) =
            all_goals.into_iter().partition(|g| g.goal_type == "sinking-fund");
        let sinking_funds: Vec<GoalDto> = sinking.into_iter().map(goal_to_dto).collect();
        let active_goals: Vec<GoalDto> = other
            .into_iter()
            .filter(|g| g.current_cents < g.target_cents)
            .map(goal_to_dto)
            .collect();

        // Recurring expense estimate
        let cutoff = (now - chrono::Duration::days(395)).format("%Y-%m-%d").to_string();
        let recurring_expense_cents: i64 = conn.query_row(
            &format!(
                "WITH gaps AS (
               SELECT merchant_raw,
                      julianday(date(posted_at)) -
                        julianday(LAG(date(posted_at)) OVER (
                          PARTITION BY merchant_raw ORDER BY posted_at
                        )) AS gap,
                      amount_cents
               FROM transactions t WHERE posted_at >= ?1 AND is_transfer = 0 AND {}
             ),
             agg AS (
               SELECT merchant_raw, AVG(gap) AS avg_gap, MAX(amount_cents) AS last_amount
               FROM gaps WHERE gap BETWEEN 5 AND 400
               GROUP BY merchant_raw
               HAVING COUNT(*) >= 2 AND AVG(gap) < 45
             )
             SELECT COALESCE(SUM(ABS(last_amount)), 0) FROM agg WHERE last_amount < 0",
                finsight_core::metrics::non_investment_txn_predicate("t")
            ),
            rusqlite::params![cutoff],
            |r| r.get(0),
        )?;

        let look_back = budgets::look_back_facts(conn, &m0)?;

        Ok(PlanData {
            income_cents,
            categories,
            goals: active_goals,
            sinking_funds,
            recurring_expense_cents,
            look_back,
        })
    })
    .await
    .map_err(AppError::from)
}

/// Write next month's budget assignments. The Tauri wrapper additionally fires
/// a best-effort desktop notification after this returns (see
/// `crates/finsight-app/src/commands/budget.rs`); the server has no native
/// notifications in Phase 1, so this body is purely the budget write.
pub async fn apply_next_month_plan(
    state: &ApiState,
    assignments: Vec<PlanAssignment>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let now = Utc::now();
        // next month
        let (ny, nm) = if now.month() == 12 {
            (now.year() + 1, 1u32)
        } else {
            (now.year(), now.month() + 1)
        };
        let next_month = format!("{ny}-{nm:02}");
        for a in &assignments {
            if a.amount_cents > 0 {
                budgets::set(conn, &a.category_id, &next_month, a.amount_cents)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(AppError::from)
}

pub async fn list_budget_history(
    state: &ApiState,
    months: u32,
) -> AppResult<Vec<CategoryHistory>> {
    let db = (*state.db).clone();
    let months = months.clamp(1, 24);
    run(&db, move |conn| {
        let now = Utc::now();
        // Build list of month strings oldest first
        let month_list: Vec<String> = (0..months)
            .map(|i| {
                let m0 = now.month0() as i32 - i as i32;
                let (yr, mo) = if m0 < 0 {
                    (now.year() - 1, (m0 + 12) as u32 + 1)
                } else {
                    (now.year(), m0 as u32 + 1)
                };
                format!("{yr}-{mo:02}")
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let cutoff = format!("{}-01", month_list.first().unwrap());

        // Aggregate per-category per-month outflows. A `settle_up = 1` row nets
        // as `-amount_cents` (a reimbursement inflow reduces reported spend)
        // instead of being silently dropped by an `amount_cents < 0`-only filter.
        let mut stmt = conn.prepare(
            "SELECT t.category_id, strftime('%Y-%m', t.posted_at) AS mo,
                    SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents
                             WHEN t.amount_cents < 0 THEN -t.amount_cents
                             ELSE 0 END) AS cents
             FROM transactions t
             WHERE (t.amount_cents < 0 OR t.settle_up = 1)
               AND t.posted_at >= ?1
               AND t.category_id IS NOT NULL
             GROUP BY t.category_id, mo",
        )?;
        let mut spend_map: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();
        let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows.flatten() {
            spend_map.insert((row.0, row.1), row.2);
        }
        drop(stmt);

        // Same shape, for budgeted amounts.
        let mut budget_stmt = conn.prepare(
            "SELECT category_id, month, amount_cents FROM budgets WHERE month >= ?1",
        )?;
        let cutoff_month = month_list.first().unwrap().clone();
        let mut budget_map: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();
        let budget_rows = budget_stmt.query_map(rusqlite::params![cutoff_month], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in budget_rows.flatten() {
            budget_map.insert((row.0, row.1), row.2);
        }
        drop(budget_stmt);

        // Fetch all non-archived categories
        let mut cat_stmt = conn.prepare(
            "SELECT c.id, c.label, COALESCE(c.color,'')
             FROM categories c
             WHERE c.archived_at IS NULL
             ORDER BY c.sort_order",
        )?;
        let cats: Vec<(String, String, String)> = cat_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .flatten()
            .collect();
        drop(cat_stmt);

        let month_labels: Vec<String> = month_list
            .iter()
            .map(|m| {
                let mo: u32 = m[5..7].parse().unwrap_or(1);
                let names = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];
                names[(mo.saturating_sub(1)) as usize].to_string()
            })
            .collect();

        let mut result: Vec<CategoryHistory> = cats
            .into_iter()
            .filter_map(|(id, label, color)| {
                let monthly: Vec<MonthlyActual> = month_list
                    .iter()
                    .zip(month_labels.iter())
                    .map(|(m, lbl)| MonthlyActual {
                        month: m.clone(),
                        label: lbl.clone(),
                        spent_cents: spend_map
                            .get(&(id.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0),
                        budgeted_cents: budget_map
                            .get(&(id.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect();
                let total: i64 = monthly.iter().map(|m| m.spent_cents).sum();
                if total == 0 {
                    return None;
                }
                Some(CategoryHistory {
                    category_id: id,
                    label,
                    color,
                    monthly,
                })
            })
            .collect();

        // Sort by total spend descending
        result.sort_by(|a, b| {
            let ta: i64 = a.monthly.iter().map(|m| m.spent_cents).sum();
            let tb: i64 = b.monthly.iter().map(|m| m.spent_cents).sum();
            tb.cmp(&ta)
        });

        Ok(result)
    })
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("budget.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_account(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();
    }

    fn seed_category(conn: &rusqlite::Connection, id: &str, label: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
            rusqlite::params![id, label],
        )
        .unwrap();
    }

    #[test]
    fn budget_envelope_spend_nets_settle_up_inflow() {
        // A settle_up = 1 reimbursement inflow must reduce the envelope's
        // reported spend (e.g. a roommate paying back their share of groceries)
        // instead of being silently dropped by an `amount_cents < 0`-only CASE,
        // and must not push the envelope into "over budget".
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_account(&conn);
        seed_category(&conn, "food", "Food");

        let month = "2026-05";
        let month_start = "2026-05-01";
        conn.execute(
            "INSERT INTO budgets(id,category_id,month,amount_cents,created_at,updated_at) \
             VALUES('b1','food',?1,4000,datetime('now'),datetime('now'))",
            rusqlite::params![month],
        )
        .unwrap();

        // Ordinary $50 grocery expense.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,is_transfer,created_at) \
             VALUES('e1','a1','2026-05-10T00:00:00Z',-5000,'GROCERY','food','cleared',0,0,'2026-05-10T00:00:00Z')",
            [],
        )
        .unwrap();
        // A $20 settle-up reimbursement for the same category.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,is_transfer,created_at,settle_up) \
             VALUES('su1','a1','2026-05-12T00:00:00Z',2000,'FRIEND REFUND','food','cleared',0,0,'2026-05-12T00:00:00Z',1)",
            [],
        )
        .unwrap();

        let envelopes = budget_envelopes_for_month(&mut conn, month, month_start).unwrap();
        let food = envelopes
            .iter()
            .find(|e| e.category_id == "food")
            .expect("food envelope present");
        assert_eq!(
            food.spent_cents, 3000,
            "settle-up inflow nets against expense: 5000 - 2000 = 3000"
        );
        assert!(
            food.spent_cents < food.budget_cents,
            "netted spend (3000) is under the 4000 budget"
        );
    }
}

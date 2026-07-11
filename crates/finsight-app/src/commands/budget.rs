use crate::error::{AppError, AppResult};
use crate::AppState;
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
    /// Budget set by user (0 = not budgeted)
    pub budget_cents: i64,
    /// Actual outflow this month (positive = spent)
    pub spent_cents: i64,
    pub txn_count: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn list_budget_envelopes(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<BudgetEnvelope>> {
    let db = (*state.db).clone();
    let now = Utc::now();
    let month = now.format("%Y-%m").to_string();
    let this_month_start = now.format("%Y-%m-01").to_string();

    run(&db, move |conn| {
        // Get budgets for the month
        let budget_map: std::collections::HashMap<String, i64> =
            budgets::list_for_month(conn, &month)?.into_iter().collect();

        // Get spending per category this month
        let mut stmt = conn.prepare(
            "SELECT c.id, c.label, COALESCE(c.color,''), COALESCE(g.label,''), \
                    COALESCE(SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0), \
                    COUNT(t.id) \
             FROM categories c \
             LEFT JOIN category_groups g ON g.id = c.group_id \
             LEFT JOIN transactions t ON t.category_id = c.id AND t.posted_at >= ?1 \
             WHERE c.archived_at IS NULL \
             GROUP BY c.id, c.label, c.color, c.group_id, g.label \
             ORDER BY g.sort_order, c.sort_order",
        )?;
        let rows = stmt.query_map(rusqlite::params![this_month_start], |r| {
            let cat_id: String = r.get(0)?;
            Ok((cat_id.clone(), r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, budget_map.get(&cat_id).copied().unwrap_or(0)))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (cat_id, label, color, group_label, spent, txn_count, budget) = row?;
            // Only include categories that have a budget OR have spending
            if budget > 0 || spent > 0 {
                out.push(BudgetEnvelope {
                    category_id: cat_id,
                    category_label: label,
                    category_color: color,
                    group_label,
                    budget_cents: budget,
                    spent_cents: spent,
                    txn_count,
                });
            }
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_budget(
    state: tauri::State<'_, AppState>,
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
    pub recurring_expense_cents: i64,
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
    pub cents: i64,
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

#[tauri::command]
#[specta::specta]
pub async fn list_goals(state: tauri::State<'_, AppState>) -> AppResult<Vec<GoalDto>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        goals::list(conn).map(|gs| gs.into_iter().map(goal_to_dto).collect())
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_goal(
    state: tauri::State<'_, AppState>,
    input: NewGoalInput,
) -> AppResult<GoalDto> {
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

#[tauri::command]
#[specta::specta]
pub async fn project_goal_growth(
    state: tauri::State<'_, AppState>,
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

/// Set a manual goal's balance to an absolute value by appending the *delta* as a
/// ledger contribution, keeping `current_cents` a derived total. Existing callers
/// that pass an absolute balance stay correct without double-counting.
#[tauri::command]
#[specta::specta]
pub async fn update_goal_balance(
    state: tauri::State<'_, AppState>,
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

/// Append a contribution (positive) or withdrawal (negative) to a goal's ledger.
#[tauri::command]
#[specta::specta]
pub async fn contribute_to_goal(
    state: tauri::State<'_, AppState>,
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

#[tauri::command]
#[specta::specta]
pub async fn list_goal_contributions(
    state: tauri::State<'_, AppState>,
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

#[tauri::command]
#[specta::specta]
pub async fn archive_goal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| goals::archive(conn, &id))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_goal_monthly(
    state: tauri::State<'_, AppState>,
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

#[tauri::command]
#[specta::specta]
pub async fn update_goal_purpose(
    state: tauri::State<'_, AppState>,
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

#[tauri::command]
#[specta::specta]
pub async fn get_plan_next_month_data(state: tauri::State<'_, AppState>) -> AppResult<PlanData> {
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
        let income_cents: i64 = conn.query_row(
            &format!(
                "SELECT CAST(COALESCE(AVG(mi), 0) AS INTEGER)
                 FROM (SELECT SUM(amount_cents) AS mi
                       FROM transactions t
                       WHERE amount_cents > 0
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
                    COALESCE(SUM(CASE WHEN strftime('%Y-%m',t.posted_at)=?1 AND t.amount_cents<0 THEN -t.amount_cents ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN strftime('%Y-%m',t.posted_at)=?2 AND t.amount_cents<0 THEN -t.amount_cents ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN strftime('%Y-%m',t.posted_at)=?3 AND t.amount_cents<0 THEN -t.amount_cents ELSE 0 END),0)
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

        // Active goals (current < target, not archived)
        let all_goals = goals::list(conn)?;
        let active_goals: Vec<GoalDto> = all_goals
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

        Ok(PlanData {
            income_cents,
            categories,
            goals: active_goals,
            recurring_expense_cents,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn apply_next_month_plan(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
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
    .map_err(AppError::from)?;

    let notify_db = (*state.db).clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::notifications::check_and_fire(&app, &notify_db).await;
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_budget_history(
    state: tauri::State<'_, AppState>,
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

        // Aggregate per-category per-month outflows
        let mut stmt = conn.prepare(
            "SELECT t.category_id, strftime('%Y-%m', t.posted_at) AS mo,
                    SUM(-t.amount_cents) AS cents
             FROM transactions t
             WHERE t.amount_cents < 0
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
                        cents: spend_map
                            .get(&(id.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect();
                let total: i64 = monthly.iter().map(|m| m.cents).sum();
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
            let ta: i64 = a.monthly.iter().map(|m| m.cents).sum();
            let tb: i64 = b.monthly.iter().map(|m| m.cents).sum();
            tb.cmp(&ta)
        });

        Ok(result)
    })
    .await
    .map_err(AppError::from)
}

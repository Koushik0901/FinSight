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
    run(&db, move |conn| budgets::set(conn, &category_id, &month, amount_cents))
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
    pub sort_order: i64,
    pub created_at: String,
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
        sort_order: g.sort_order,
        created_at: g.created_at,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn list_goals(state: tauri::State<'_, AppState>) -> AppResult<Vec<GoalDto>> {
    let db = (*state.db).clone();
    run(&db, |conn| goals::list(conn).map(|gs| gs.into_iter().map(goal_to_dto).collect()))
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
        goals::insert(conn, goals::NewGoal {
            name: input.name,
            goal_type: input.goal_type,
            target_cents: input.target_cents,
            monthly_cents: input.monthly_cents,
            target_date: input.target_date,
            color: input.color,
            notes: input.notes,
        }).map(goal_to_dto)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_goal_balance(
    state: tauri::State<'_, AppState>,
    id: String,
    current_cents: i64,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| goals::set_current_cents(conn, &id, current_cents))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn archive_goal(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
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
    run(&db, move |conn| goals::set_monthly_cents(conn, &id, monthly_cents))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_plan_next_month_data(
    state: tauri::State<'_, AppState>,
) -> AppResult<PlanData> {
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
            "SELECT CAST(COALESCE(AVG(mi), 0) AS INTEGER)
             FROM (SELECT SUM(amount_cents) AS mi
                   FROM transactions
                   WHERE amount_cents > 0
                     AND strftime('%Y-%m', posted_at) IN (?1, ?2, ?3)
                   GROUP BY strftime('%Y-%m', posted_at))",
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
            "WITH gaps AS (
               SELECT merchant_raw,
                      julianday(date(posted_at)) -
                        julianday(LAG(date(posted_at)) OVER (
                          PARTITION BY merchant_raw ORDER BY posted_at
                        )) AS gap,
                      amount_cents
               FROM transactions WHERE posted_at >= ?1
             ),
             agg AS (
               SELECT merchant_raw, AVG(gap) AS avg_gap, MAX(amount_cents) AS last_amount
               FROM gaps WHERE gap BETWEEN 5 AND 400
               GROUP BY merchant_raw
               HAVING COUNT(*) >= 2 AND AVG(gap) < 45
             )
             SELECT COALESCE(SUM(ABS(last_amount)), 0) FROM agg WHERE last_amount < 0",
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
    .map_err(AppError::from)
}

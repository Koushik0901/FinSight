use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::Utc;
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

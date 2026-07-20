use crate::error::AppResult;
use crate::AppState;

// Types live in finsight-api now; re-exported so existing imports of
// `finsight_app::commands::budget::*` (lib.rs, tests) keep resolving.
pub use finsight_api::commands::budget::{
    BudgetEnvelope, CategoryHistory, CategoryPlanRow, GoalContributionDto, GoalDto, MonthlyActual,
    NewGoalInput, PlanAssignment, PlanData, ProjectedValue,
};

#[tauri::command]
#[specta::specta]
pub async fn list_budget_envelopes(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<BudgetEnvelope>> {
    finsight_api::commands::budget::list_budget_envelopes(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_budget(
    state: tauri::State<'_, AppState>,
    category_id: String,
    amount_cents: i64,
) -> AppResult<()> {
    finsight_api::commands::budget::set_budget(&state.api, category_id, amount_cents).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_goals(state: tauri::State<'_, AppState>) -> AppResult<Vec<GoalDto>> {
    finsight_api::commands::budget::list_goals(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_goal(
    state: tauri::State<'_, AppState>,
    input: NewGoalInput,
) -> AppResult<GoalDto> {
    finsight_api::commands::budget::create_goal(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn project_goal_growth(
    state: tauri::State<'_, AppState>,
    goal_id: String,
    years: i32,
) -> AppResult<ProjectedValue> {
    finsight_api::commands::budget::project_goal_growth(&state.api, goal_id, years).await
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
    finsight_api::commands::budget::update_goal_balance(&state.api, id, current_cents).await
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
    finsight_api::commands::budget::contribute_to_goal(&state.api, id, amount_cents, note, source)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_goal_contributions(
    state: tauri::State<'_, AppState>,
    goal_id: String,
) -> AppResult<Vec<GoalContributionDto>> {
    finsight_api::commands::budget::list_goal_contributions(&state.api, goal_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn archive_goal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::budget::archive_goal(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_goal_monthly(
    state: tauri::State<'_, AppState>,
    id: String,
    monthly_cents: i64,
) -> AppResult<()> {
    finsight_api::commands::budget::update_goal_monthly(&state.api, id, monthly_cents).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_goal_priority(
    state: tauri::State<'_, AppState>,
    id: String,
    priority: String,
    deadline_strictness: String,
) -> AppResult<()> {
    finsight_api::commands::budget::update_goal_priority(
        &state.api,
        id,
        priority,
        deadline_strictness,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_goal_purpose(
    state: tauri::State<'_, AppState>,
    id: String,
    purpose: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::budget::update_goal_purpose(&state.api, id, purpose).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_plan_next_month_data(state: tauri::State<'_, AppState>) -> AppResult<PlanData> {
    finsight_api::commands::budget::get_plan_next_month_data(&state.api).await
}

// Applies next month's budget assignments, then best-effort fires the desktop
// "budget planned" notification. The notification uses `tauri::AppHandle`
// directly (native notification plugin), so it stays here in the wrapper —
// the finsight-api body (`finsight_api::commands::budget::apply_next_month_plan`)
// is purely the budget write and has no tauri dependency.
// (Plain `//` on purpose: `///` doc comments flow into the generated
// bindings.ts and would break the Phase 1 bindings zero-diff invariant.)
#[tauri::command]
#[specta::specta]
pub async fn apply_next_month_plan(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    assignments: Vec<PlanAssignment>,
) -> AppResult<()> {
    finsight_api::commands::budget::apply_next_month_plan(&state.api, assignments).await?;

    let notify_db = (*state.api.db).clone();
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
    finsight_api::commands::budget::list_budget_history(&state.api, months).await
}

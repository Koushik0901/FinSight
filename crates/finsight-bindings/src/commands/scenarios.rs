use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::scenarios::{
    RanScenario, SavedScenarioDetail, ScenarioParamsInput, ScenarioPlanProposal, ScenarioResult,
};

/// Run a what-if projection. Returns the result plus the resolved params (so a
/// free-text scenario, whose params the server extracted, can then be saved).
#[tauri::command]
#[specta::specta]
pub async fn run_scenario(
    state: tauri::State<'_, AppState>,
    description: String,
    months: u32,
    params: Option<ScenarioParamsInput>,
) -> AppResult<RanScenario> {
    finsight_api::commands::scenarios::run_scenario(&state.api, description, months, params).await
}

/// Save a scenario durably (params + baseline + result), so it can later be
/// recomputed, compared, and checked for staleness.
#[tauri::command]
#[specta::specta]
pub async fn save_scenario(
    state: tauri::State<'_, AppState>,
    description: String,
    params: ScenarioParamsInput,
    months: u32,
) -> AppResult<SavedScenarioDetail> {
    finsight_api::commands::scenarios::save_scenario(&state.api, description, params, months).await
}

/// Active saved scenarios, each recomputed against the current baseline (so a
/// comparison across them is consistent) with a staleness flag.
#[tauri::command]
#[specta::specta]
pub async fn list_saved_scenarios(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SavedScenarioDetail>> {
    finsight_api::commands::scenarios::list_saved_scenarios(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn duplicate_scenario(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Option<SavedScenarioDetail>> {
    finsight_api::commands::scenarios::duplicate_scenario(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn archive_scenario(
    state: tauri::State<'_, AppState>,
    id: String,
    archived: bool,
) -> AppResult<()> {
    finsight_api::commands::scenarios::archive_scenario(&state.api, id, archived).await
}

/// Promote a scenario into a reviewable set of proposed plan changes. Writes
/// nothing — the proposals are for the user to approve and apply themselves.
#[tauri::command]
#[specta::specta]
pub async fn promote_scenario(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<ScenarioPlanProposal> {
    finsight_api::commands::scenarios::promote_scenario(&state.api, id).await
}

/// Revise a saved scenario's assumptions (issue #73) and re-evaluate it — the
/// original result is preserved; the returned detail adds the revised result.
#[tauri::command]
#[specta::specta]
pub async fn revise_scenario(
    state: tauri::State<'_, AppState>,
    id: String,
    params: ScenarioParamsInput,
) -> AppResult<SavedScenarioDetail> {
    finsight_api::commands::scenarios::revise_scenario(&state.api, id, params).await
}

/// Discard a scenario's revision, reverting to the original assumptions only.
#[tauri::command]
#[specta::specta]
pub async fn clear_scenario_revision(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<SavedScenarioDetail> {
    finsight_api::commands::scenarios::clear_scenario_revision(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_scenario(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::scenarios::delete_scenario(&state.api, id).await
}

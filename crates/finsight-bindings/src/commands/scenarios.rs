use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::scenarios::{ScenarioParamsInput, ScenarioResult, SavedScenario};

#[tauri::command]
#[specta::specta]
pub async fn run_scenario(
    state: tauri::State<'_, AppState>,
    description: String,
    months: u32,
    params: Option<ScenarioParamsInput>,
) -> AppResult<ScenarioResult> {
    finsight_api::commands::scenarios::run_scenario(&state.api, description, months, params).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_scenario(
    state: tauri::State<'_, AppState>,
    description: String,
    result: ScenarioResult,
) -> AppResult<SavedScenario> {
    finsight_api::commands::scenarios::save_scenario(&state.api, description, result).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_scenario_history(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SavedScenario>> {
    finsight_api::commands::scenarios::list_scenario_history(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_scenario(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::scenarios::delete_scenario(&state.api, id).await
}

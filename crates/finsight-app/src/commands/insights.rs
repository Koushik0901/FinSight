use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::AgentMemory;

pub use finsight_api::commands::insights::{HealthScore, HealthScoreBreakdown};

#[tauri::command]
#[specta::specta]
pub async fn list_agent_memory(state: tauri::State<'_, AppState>) -> AppResult<Vec<AgentMemory>> {
    finsight_api::commands::insights::list_agent_memory(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn forget_agent_memory(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::insights::forget_agent_memory(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_financial_health_score(
    state: tauri::State<'_, AppState>,
) -> AppResult<HealthScore> {
    finsight_api::commands::insights::get_financial_health_score(&state.api).await
}

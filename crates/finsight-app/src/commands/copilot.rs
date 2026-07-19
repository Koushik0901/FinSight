use crate::error::AppResult;
use crate::AppState;

// Types live in finsight-api now; re-exported so existing imports of
// `finsight_app::commands::copilot::*` (lib.rs, tests) keep resolving.
pub use finsight_api::commands::copilot::{ExecutionItemResult, ExecutionSummary};

use finsight_core::models::{AgentActionBundle, AgentExecutionEntry, AgentSession};

#[tauri::command]
#[specta::specta]
pub async fn list_agent_sessions(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AgentSession>> {
    finsight_api::commands::copilot::list_agent_sessions(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_agent_session(
    state: tauri::State<'_, AppState>,
    title: String,
    task_type: String,
) -> AppResult<AgentSession> {
    finsight_api::commands::copilot::create_agent_session(&state.api, title, task_type).await
}

#[tauri::command]
#[specta::specta]
pub async fn close_agent_session(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::copilot::close_agent_session(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_action_bundles(
    state: tauri::State<'_, AppState>,
    status_filter: Option<String>,
    session_id: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<AgentActionBundle>> {
    finsight_api::commands::copilot::list_action_bundles(
        &state.api,
        status_filter,
        session_id,
        limit,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_action_bundle(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Option<AgentActionBundle>> {
    finsight_api::commands::copilot::get_action_bundle(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn approve_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> AppResult<()> {
    finsight_api::commands::copilot::approve_action_item(&state.api, item_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn reject_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> AppResult<()> {
    finsight_api::commands::copilot::reject_action_item(&state.api, item_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_execution_log(
    state: tauri::State<'_, AppState>,
    bundle_id: String,
) -> AppResult<Vec<AgentExecutionEntry>> {
    finsight_api::commands::copilot::list_execution_log(&state.api, bundle_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn execute_action_bundle(
    state: tauri::State<'_, AppState>,
    bundle_id: String,
) -> AppResult<ExecutionSummary> {
    finsight_api::commands::copilot::execute_action_bundle(&state.api, bundle_id).await
}

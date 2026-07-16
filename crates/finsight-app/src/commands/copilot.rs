use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{AgentActionBundle, AgentExecutionEntry, AgentSession};
use finsight_core::repos::{copilot_actions, copilot_sessions, run};
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSummary {
    pub bundle_id: String,
    pub succeeded: u32,
    pub failed: u32,
    pub results: Vec<ExecutionItemResult>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionItemResult {
    pub item_id: String,
    pub action_kind: String,
    pub status: String,
    pub summary: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_agent_sessions(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AgentSession>> {
    let db = (*state.api.db).clone();
    run(&db, |conn| copilot_sessions::list(conn, 50))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_agent_session(
    state: tauri::State<'_, AppState>,
    title: String,
    task_type: String,
) -> AppResult<AgentSession> {
    let db = (*state.api.db).clone();
    run(&db, move |conn| {
        copilot_sessions::insert(conn, &title, &task_type)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn close_agent_session(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.api.db).clone();
    run(&db, move |conn| {
        copilot_sessions::set_status(conn, &id, "closed")
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_action_bundles(
    state: tauri::State<'_, AppState>,
    status_filter: Option<String>,
    session_id: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<AgentActionBundle>> {
    let db = (*state.api.db).clone();
    let limit = limit.unwrap_or(25);
    run(&db, move |conn| {
        copilot_actions::list_bundles(conn, status_filter.as_deref(), session_id.as_deref(), limit)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_action_bundle(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Option<AgentActionBundle>> {
    let db = (*state.api.db).clone();
    run(&db, move |conn| copilot_actions::get_bundle(conn, &id))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn approve_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> AppResult<()> {
    let db = (*state.api.db).clone();
    run(&db, move |conn| {
        copilot_actions::set_item_status(conn, &item_id, "approved")
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn reject_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> AppResult<()> {
    let db = (*state.api.db).clone();
    run(&db, move |conn| {
        copilot_actions::set_item_status(conn, &item_id, "rejected")
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_execution_log(
    state: tauri::State<'_, AppState>,
    bundle_id: String,
) -> AppResult<Vec<AgentExecutionEntry>> {
    let db = (*state.api.db).clone();
    run(&db, move |conn| {
        copilot_actions::list_execution_log(conn, &bundle_id)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn execute_action_bundle(
    state: tauri::State<'_, AppState>,
    bundle_id: String,
) -> AppResult<ExecutionSummary> {
    let db = (*state.api.db).clone();
    let result = run(&db, move |conn| {
        finsight_agent::executor::execute_bundle(conn, &bundle_id)
    })
    .await
    .map_err(AppError::from)?;

    Ok(ExecutionSummary {
        bundle_id: result.bundle_id,
        succeeded: result.succeeded as u32,
        failed: result.failed as u32,
        results: result
            .executed
            .into_iter()
            .map(|item| ExecutionItemResult {
                item_id: item.item_id,
                action_kind: item.action_kind,
                status: item.status,
                summary: item.result_summary,
                error: item.error,
            })
            .collect(),
    })
}

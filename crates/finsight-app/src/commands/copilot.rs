use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{AgentActionBundle, AgentExecutionEntry, AgentSession};
use finsight_core::repos::{copilot_actions, copilot_sessions, run};
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CopilotPlanResult {
    pub bundle_id: String,
    pub answer: String,
    pub assumptions: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub forecast_summary: Option<String>,
}

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
    let db = (*state.db).clone();
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
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_sessions::insert(conn, &title, &task_type)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn close_agent_session(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
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
    limit: Option<u32>,
) -> AppResult<Vec<AgentActionBundle>> {
    let db = (*state.db).clone();
    let limit = limit.unwrap_or(25);
    run(&db, move |conn| {
        copilot_actions::list_bundles(conn, status_filter.as_deref(), limit)
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
    let db = (*state.db).clone();
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
    let db = (*state.db).clone();
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
    let db = (*state.db).clone();
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
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_actions::list_execution_log(conn, &bundle_id)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn start_copilot_plan(
    state: tauri::State<'_, AppState>,
    session_id: Option<String>,
    question: String,
) -> AppResult<CopilotPlanResult> {
    use finsight_agent::{context, planner};

    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        return Err(AppError::new(
            "no_provider",
            "Configure an AI provider in Settings → Agent.",
        ));
    };
    let provider_id = provider.provider_id().to_string();
    let model_id = provider.model_id().to_string();
    let db = (*state.db).clone();

    let ctx = run(&db, |conn| Ok(context::build_context(conn)))
        .await
        .map_err(AppError::from)?;
    let llm_response = provider
        .complete_json(&planner::build_system_prompt(&ctx), &question)
        .await
        .map_err(|e| AppError::new("planner.llm", e.to_string()))?;

    let session_id_clone = session_id.clone();
    let question_clone = question.clone();
    let result = run(&db, move |conn| {
        planner::persist_plan(
            conn,
            session_id_clone.as_deref(),
            &question_clone,
            &llm_response,
            &provider_id,
            &model_id,
        )
    })
    .await
    .map_err(AppError::from)?;

    Ok(CopilotPlanResult {
        bundle_id: result.bundle.id,
        answer: result.answer,
        assumptions: result.assumptions,
        follow_up_questions: result.follow_up_questions,
        forecast_summary: result.forecast_summary,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn execute_action_bundle(
    state: tauri::State<'_, AppState>,
    bundle_id: String,
) -> AppResult<ExecutionSummary> {
    let db = (*state.db).clone();
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

use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::AgentMemory;
use finsight_core::repos::{agent_memory, run};

#[tauri::command]
#[specta::specta]
pub async fn list_agent_memory(state: tauri::State<'_, AppState>) -> AppResult<Vec<AgentMemory>> {
    let db = (*state.db).clone();
    run(&db, agent_memory::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn forget_agent_memory(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| agent_memory::forget(conn, &id)).await.map_err(AppError::from)
}

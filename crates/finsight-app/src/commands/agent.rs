// Implemented in Task 15
use crate::error::AppResult;
use tauri::State;
use crate::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CompletionProviderConfig {
    pub kind: String,
}

#[tauri::command]
#[specta::specta]
pub async fn set_completion_provider(
    _state: State<'_, AppState>,
    _config: serde_json::Value,
) -> AppResult<()> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn save_provider_api_key(
    _state: State<'_, AppState>,
    _provider_id: String,
    _api_key: String,
) -> AppResult<()> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_provider_models(
    _state: State<'_, AppState>,
) -> AppResult<Vec<String>> {
    Ok(vec![])
}

#[tauri::command]
#[specta::specta]
pub async fn test_completion_provider(
    _state: State<'_, AppState>,
    _config: serde_json::Value,
) -> AppResult<bool> {
    Ok(false)
}

#[tauri::command]
#[specta::specta]
pub async fn get_needs_review_count(
    _state: State<'_, AppState>,
) -> AppResult<u32> {
    Ok(0)
}

#[tauri::command]
#[specta::specta]
pub async fn trigger_categorize(
    _state: State<'_, AppState>,
    _import_id: Option<String>,
) -> AppResult<()> {
    Ok(())
}

use crate::{error::AppResult, AppState};

pub use finsight_api::commands::inbox::ActionItem;

#[tauri::command]
#[specta::specta]
pub async fn get_action_items(state: tauri::State<'_, AppState>) -> AppResult<Vec<ActionItem>> {
    finsight_api::commands::inbox::get_action_items(&state.api).await
}

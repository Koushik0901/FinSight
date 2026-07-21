use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::recurring::RecurringItem;

#[tauri::command]
#[specta::specta]
pub async fn list_recurring(state: tauri::State<'_, AppState>) -> AppResult<Vec<RecurringItem>> {
    finsight_api::commands::recurring::list_recurring(&state.api).await
}

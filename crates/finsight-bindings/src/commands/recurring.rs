use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::recurring::{PriceChangeDto, RecurringItem};

#[tauri::command]
#[specta::specta]
pub async fn list_recurring(state: tauri::State<'_, AppState>) -> AppResult<Vec<RecurringItem>> {
    finsight_api::commands::recurring::list_recurring(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_subscription_verdict(
    state: tauri::State<'_, AppState>,
    merchant_key: String,
    verdict: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::recurring::set_subscription_verdict(&state.api, merchant_key, verdict).await
}

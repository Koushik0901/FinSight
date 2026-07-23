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

/// Mark a detected subscription as a free trial converting on `trial_ends_at`
/// (or clear it with null); a heads-up fires shortly before (#75).
#[tauri::command]
#[specta::specta]
pub async fn set_subscription_trial(
    state: tauri::State<'_, AppState>,
    merchant_key: String,
    label: String,
    trial_ends_at: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::recurring::set_subscription_trial(&state.api, merchant_key, label, trial_ends_at).await
}

/// Mark a detected subscription cancelled as of `cancelled_at`; a charge after
/// that date is surfaced as a surprise (#75).
#[tauri::command]
#[specta::specta]
pub async fn mark_subscription_cancelled(
    state: tauri::State<'_, AppState>,
    merchant_key: String,
    label: String,
    cancelled_at: String,
) -> AppResult<()> {
    finsight_api::commands::recurring::mark_subscription_cancelled(&state.api, merchant_key, label, cancelled_at).await
}

use crate::error::AppResult;
use crate::AppState;

/// Re-exported so `crate::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY`
/// (used by `lib.rs`'s startup resume-categorization check) keeps resolving
/// now that the constant + its owning commands live in finsight-api.
pub use finsight_api::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY;

#[tauri::command]
#[specta::specta]
pub async fn get_currency(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::settings::get_currency(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_currency(state: tauri::State<'_, AppState>, currency: String) -> AppResult<()> {
    finsight_api::commands::settings::set_currency(&state.api, currency).await
}

/// Factory-reset: wipes every local financial/user-data table (accounts,
/// transactions, budgets, goals, categories, reports/insight caches,
/// scenarios, recipes, agent memory/context, review queues, etc.) while
/// preserving `settings` (provider selection, currency, toggles) and the OS
/// keychain (API keys, DB encryption key) untouched. The frontend is
/// responsible for the double-confirmation UX before calling this.
#[tauri::command]
#[specta::specta]
pub async fn delete_all_data(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::settings::delete_all_data(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_all_data_json(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::settings::export_all_data_json(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_notifications_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    finsight_api::commands::settings::get_notifications_enabled(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_notifications_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    finsight_api::commands::settings::set_notifications_enabled(&state.api, enabled).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_auto_categorize_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    finsight_api::commands::settings::get_auto_categorize_enabled(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_auto_categorize_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    finsight_api::commands::settings::set_auto_categorize_enabled(&state.api, enabled).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_all_data_csv(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::settings::export_all_data_csv(&state.api).await
}

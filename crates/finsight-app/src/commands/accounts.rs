use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{
    Account, AccountBalancePoint, AccountBalanceTimeline, AccountPatch, AccountSparkline,
    AccountSummary, NewAccount,
};

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<AccountSummary>> {
    finsight_api::commands::accounts::list_accounts(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    state: tauri::State<'_, AppState>,
    input: NewAccount,
) -> AppResult<Account> {
    finsight_api::commands::accounts::create_account(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_account(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: AccountPatch,
) -> AppResult<Account> {
    finsight_api::commands::accounts::update_account(&state.api, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn archive_account(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::accounts::archive_account(&state.api, id).await
}

/// User-confirmed "this is my real balance right now" entry point — e.g. after
/// importing CSV history that carries no balance field. Back-solves the account
/// opening so the balance model reproduces the entered value AND keeps tracking
/// as transactions change, instead of freezing a fixed snapshot that goes stale
/// (see [`accounts::set_current_balance`]).
#[tauri::command]
#[specta::specta]
pub async fn set_account_balance(
    state: tauri::State<'_, AppState>,
    id: String,
    balance_cents: i64,
) -> AppResult<()> {
    finsight_api::commands::accounts::set_account_balance(&state.api, id, balance_cents).await
}

/// Returns CSV content for one account's transactions; the caller downloads
/// it client-side (Blob + `<a download>`). No native file dialog since Phase 4
/// — the desktop shell has no local command surface to host one.
#[tauri::command]
#[specta::specta]
pub async fn export_account_csv(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<String> {
    finsight_api::commands::accounts::export_account_csv(&state.api, account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_account_balance_history(
    state: tauri::State<'_, AppState>,
    account_id: String,
    days: u32,
) -> AppResult<Vec<AccountBalancePoint>> {
    finsight_api::commands::accounts::list_account_balance_history(&state.api, account_id, days)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_account_balance_timeline(
    state: tauri::State<'_, AppState>,
    account_id: String,
    since: Option<String>,
) -> AppResult<AccountBalanceTimeline> {
    finsight_api::commands::accounts::get_account_balance_timeline(&state.api, account_id, since)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_account_balance_sparklines(
    state: tauri::State<'_, AppState>,
    days: u32,
) -> AppResult<Vec<AccountSparkline>> {
    finsight_api::commands::accounts::list_account_balance_sparklines(&state.api, days).await
}

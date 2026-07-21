use crate::error::AppResult;
use crate::AppState;

// Types live in finsight-api now; re-exported so existing imports of
// `finsight_bindings::commands::simplefin::*` (lib.rs, tests) keep resolving.
pub use finsight_api::commands::simplefin::{
    SimpleFinAccountImportRequest, SimpleFinAccountInfo, SimpleFinConnectionInfo,
    SimpleFinPurgeSummary, SimpleFinStatus, SyncSummary, TransferSuggestionInfo,
};

use finsight_core::models::{ImportCandidateWithMatches, SimpleFinAlert};

/// Claim a SimpleFin setup token and persist the resulting bridge access URL
/// plus every connection exposed by that access URL.
#[tauri::command]
#[specta::specta]
pub async fn save_simplefin_setup_token(
    state: tauri::State<'_, AppState>,
    token: String,
) -> AppResult<Vec<SimpleFinConnectionInfo>> {
    finsight_api::commands::simplefin::save_simplefin_setup_token(&state.api, token).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_simplefin_status(state: tauri::State<'_, AppState>) -> AppResult<SimpleFinStatus> {
    finsight_api::commands::simplefin::get_simplefin_status(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_connections(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinConnectionInfo>> {
    finsight_api::commands::simplefin::list_simplefin_connections(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_accounts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinAccountInfo>> {
    finsight_api::commands::simplefin::list_simplefin_accounts(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn import_simplefin_accounts(
    state: tauri::State<'_, AppState>,
    accounts: Vec<SimpleFinAccountImportRequest>,
) -> AppResult<Vec<String>> {
    finsight_api::commands::simplefin::import_simplefin_accounts(&state.api, accounts).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_simplefin_account(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<SyncSummary> {
    finsight_api::commands::simplefin::sync_simplefin_account(&state.api, account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_simplefin(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::simplefin::disconnect_simplefin(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn purge_simplefin_data(
    state: tauri::State<'_, AppState>,
) -> AppResult<SimpleFinPurgeSummary> {
    finsight_api::commands::simplefin::purge_simplefin_data(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_simplefin_connection(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> AppResult<()> {
    finsight_api::commands::simplefin::delete_simplefin_connection(&state.api, connection_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_all_simplefin_accounts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<crate::sync_scheduler::AccountSyncResult>> {
    finsight_api::commands::simplefin::sync_all_simplefin_accounts(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_simplefin_sync_settings(
    state: tauri::State<'_, AppState>,
) -> AppResult<crate::sync_scheduler::SimpleFinSyncSettings> {
    finsight_api::commands::simplefin::get_simplefin_sync_settings(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_simplefin_sync_settings(
    state: tauri::State<'_, AppState>,
    settings: crate::sync_scheduler::SimpleFinSyncSettings,
) -> AppResult<()> {
    finsight_api::commands::simplefin::set_simplefin_sync_settings(&state.api, settings).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_alerts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinAlert>> {
    finsight_api::commands::simplefin::list_simplefin_alerts(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn acknowledge_simplefin_alert(
    state: tauri::State<'_, AppState>,
    alert_id: String,
) -> AppResult<()> {
    finsight_api::commands::simplefin::acknowledge_simplefin_alert(&state.api, alert_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_transfer_suggestions(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<TransferSuggestionInfo>> {
    finsight_api::commands::simplefin::list_simplefin_transfer_suggestions(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn confirm_simplefin_transfer(
    state: tauri::State<'_, AppState>,
    transfer_id: String,
) -> AppResult<()> {
    finsight_api::commands::simplefin::confirm_simplefin_transfer(&state.api, transfer_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn reject_simplefin_transfer(
    state: tauri::State<'_, AppState>,
    transfer_id: String,
) -> AppResult<()> {
    finsight_api::commands::simplefin::reject_simplefin_transfer(&state.api, transfer_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_import_review_candidates(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<ImportCandidateWithMatches>> {
    finsight_api::commands::simplefin::list_import_review_candidates(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn accept_import_candidate_match(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    transaction_id: String,
) -> AppResult<()> {
    finsight_api::commands::simplefin::accept_import_candidate_match(
        &state.api,
        candidate_id,
        transaction_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn create_import_candidate_transaction(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
) -> AppResult<String> {
    finsight_api::commands::simplefin::create_import_candidate_transaction(
        &state.api,
        candidate_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn dismiss_import_candidate(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
) -> AppResult<()> {
    finsight_api::commands::simplefin::dismiss_import_candidate(&state.api, candidate_id).await
}

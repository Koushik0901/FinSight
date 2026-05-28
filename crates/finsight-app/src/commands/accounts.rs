use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{Account, AccountSummary, NewAccount};
use finsight_core::repos::{accounts, run};

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<AccountSummary>> {
    // `state.db` is `Arc<Db>`; deref + clone gives us an owned `Db` (cheap — it's
    // an Arc-wrapped pool internally) that we can move into the blocking closure.
    let db = (*state.db).clone();
    let result = run(&db, accounts::list_summaries)
        .await
        .map_err(AppError::from)?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    state: tauri::State<'_, AppState>,
    mut input: NewAccount,
) -> AppResult<Account> {
    // Always force source to "manual" — the frontend cannot create sample accounts.
    // Without this, a caller passing source:"sample" would have their account silently
    // wiped by clear_sample_data.
    input.source = "manual".to_string();
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::insert(conn, input))
        .await
        .map_err(AppError::from)
}

// Stubs — implemented in a later task
#[tauri::command]
#[specta::specta]
pub async fn update_account(
    _state: tauri::State<'_, AppState>,
    _id: String,
    _input: serde_json::Value,
) -> AppResult<Account> {
    Err(crate::error::AppError::new("not_implemented", "update_account not yet implemented"))
}

#[tauri::command]
#[specta::specta]
pub async fn archive_account(
    _state: tauri::State<'_, AppState>,
    _id: String,
) -> AppResult<()> {
    Err(crate::error::AppError::new("not_implemented", "archive_account not yet implemented"))
}

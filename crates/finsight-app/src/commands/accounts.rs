use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::AccountSummary;
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

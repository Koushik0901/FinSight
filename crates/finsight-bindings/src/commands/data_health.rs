//! Data durability surface (P0-4): integrity status, backups, and a safe,
//! restart-applied restore. Backups are consistent encrypted `VACUUM INTO`
//! snapshots written to `<app-data>/backups/`.

use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::data_health::{BackupInfo, DataHealth};

#[tauri::command]
#[specta::specta]
pub async fn get_data_health(state: tauri::State<'_, AppState>) -> AppResult<DataHealth> {
    finsight_api::commands::data_health::get_data_health(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_manual_backup(state: tauri::State<'_, AppState>) -> AppResult<BackupInfo> {
    finsight_api::commands::data_health::create_manual_backup(&state.api).await
}

/// Stage a restore: copy the chosen backup to `data.pending-restore.sqlcipher`.
/// The swap into `data.sqlcipher` happens on the NEXT startup, before the DB is
/// opened, so we never replace a database that has live connections. The
/// current database is itself backed up first, so a restore is reversible.
#[tauri::command]
#[specta::specta]
pub async fn stage_restore_backup(
    state: tauri::State<'_, AppState>,
    path: String,
) -> AppResult<()> {
    finsight_api::commands::data_health::stage_restore_backup(&state.api, path).await
}

/// Cancel a staged restore (delete the pending file) before the next restart.
#[tauri::command]
#[specta::specta]
pub async fn cancel_staged_restore(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::data_health::cancel_staged_restore(&state.api).await
}

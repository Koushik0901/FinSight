//! Data durability surface (P0-4): integrity status, backups, and a safe,
//! restart-applied restore. Backups are consistent encrypted `VACUUM INTO`
//! snapshots written to `<app-data>/backups/`.

use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::run;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    /// File modified time, RFC3339. Empty when unavailable.
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DataHealth {
    /// "ok" when the last integrity check passed; otherwise the error rows.
    pub integrity_status: String,
    pub integrity_checked_at: Option<String>,
    pub last_backup_at: Option<String>,
    pub db_bytes: u64,
    pub wal_bytes: u64,
    /// Non-fatal problems from the last startup derived-data cascade.
    pub startup_warnings: Vec<String>,
    pub backups: Vec<BackupInfo>,
    /// Set once a restore is staged; the app must restart to apply it.
    pub pending_restore: bool,
}

fn app_data_dir(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::new("data.path", format!("app data dir: {e}")))
}

fn file_len(p: &PathBuf) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn mtime_rfc3339(p: &std::path::Path) -> String {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
        .unwrap_or_default()
}

fn list_backup_files(dir: &std::path::Path) -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<BackupInfo> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("data.backup-") && n.ends_with(".sqlcipher"))
                .unwrap_or(false)
        })
        .map(|p| BackupInfo {
            name: p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            bytes: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
            created_at: mtime_rfc3339(&p),
            path: p.to_string_lossy().to_string(),
        })
        .collect();
    // Newest first.
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

#[tauri::command]
#[specta::specta]
pub async fn get_data_health(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<DataHealth> {
    let dir = app_data_dir(&app)?;
    let db = (*state.db).clone();
    let (integrity_status, integrity_checked_at, last_backup_at, startup_warnings) = run(&db, |conn| {
        Ok((
            finsight_core::settings::get::<String>(conn, "data.integrity_status")?
                .unwrap_or_else(|| "unknown".into()),
            finsight_core::settings::get::<String>(conn, "data.integrity_checked_at")?,
            finsight_core::settings::get::<String>(conn, "data.last_backup_at")?,
            finsight_core::settings::get::<Vec<String>>(conn, "data.startup_warnings")?
                .unwrap_or_default(),
        ))
    })
    .await
    .map_err(AppError::from)?;

    Ok(DataHealth {
        integrity_status,
        integrity_checked_at,
        last_backup_at,
        db_bytes: file_len(&dir.join("data.sqlcipher")),
        wal_bytes: file_len(&dir.join("data.sqlcipher-wal")),
        startup_warnings,
        backups: list_backup_files(&dir.join("backups")),
        pending_restore: dir.join("data.pending-restore.sqlcipher").exists(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn create_manual_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<BackupInfo> {
    let dir = app_data_dir(&app)?;
    let db = (*state.db).clone();
    let backups_dir = dir.join("backups");
    let path = tokio::task::spawn_blocking(move || db.backup(&backups_dir, "manual", 10))
        .await
        .map_err(|e| AppError::new("data.join", e.to_string()))?
        .map_err(AppError::from)?;
    let db2 = (*state.db).clone();
    run(&db2, {
        let p = path.to_string_lossy().to_string();
        move |conn| {
            finsight_core::settings::set(conn, "data.last_backup_path", &p)?;
            finsight_core::settings::set(
                conn,
                "data.last_backup_at",
                &chrono::Utc::now().to_rfc3339(),
            )
        }
    })
    .await
    .map_err(AppError::from)?;
    Ok(BackupInfo {
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string(),
        bytes: file_len(&path),
        created_at: mtime_rfc3339(&path),
        path: path.to_string_lossy().to_string(),
    })
}

/// Stage a restore: copy the chosen backup to `data.pending-restore.sqlcipher`.
/// The swap into `data.sqlcipher` happens on the NEXT startup, before the DB is
/// opened, so we never replace a database that has live connections. The
/// current database is itself backed up first, so a restore is reversible.
#[tauri::command]
#[specta::specta]
pub async fn stage_restore_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> AppResult<()> {
    let dir = app_data_dir(&app)?;
    let backups_dir = dir.join("backups");
    let chosen = PathBuf::from(&path);
    // Only allow restoring from our own backups directory (no arbitrary paths).
    let canonical_ok = chosen
        .canonicalize()
        .ok()
        .zip(backups_dir.canonicalize().ok())
        .map(|(c, b)| c.starts_with(&b))
        .unwrap_or(false);
    if !canonical_ok || !chosen.exists() {
        return Err(AppError::new(
            "data.restore",
            "Backup file not found in the backups folder.",
        ));
    }
    // Safety net: snapshot the current DB before staging a restore over it.
    let db = (*state.db).clone();
    let bdir = backups_dir.clone();
    let _ = tokio::task::spawn_blocking(move || db.backup(&bdir, "pre-restore", 10)).await;

    let staged = dir.join("data.pending-restore.sqlcipher");
    std::fs::copy(&chosen, &staged)
        .map_err(|e| AppError::new("data.restore", format!("stage restore: {e}")))?;
    Ok(())
}

/// Cancel a staged restore (delete the pending file) before the next restart.
#[tauri::command]
#[specta::specta]
pub async fn cancel_staged_restore(app: tauri::AppHandle) -> AppResult<()> {
    let dir = app_data_dir(&app)?;
    let staged = dir.join("data.pending-restore.sqlcipher");
    if staged.exists() {
        std::fs::remove_file(&staged)
            .map_err(|e| AppError::new("data.restore", format!("cancel restore: {e}")))?;
    }
    Ok(())
}

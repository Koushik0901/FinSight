use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::{imports as imports_repo, run};
use finsight_providers::{CsvImportMapping, CsvPreview, CsvProvider, ImportSummary};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize, Type)]
pub struct ProgressPayload {
    pub import_id: String,
    pub rows_done: u32,
    pub rows_total: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn preview_csv_columns(
    path: String,
    skip_header_rows: u32,
) -> AppResult<CsvPreview> {
    let path_buf = PathBuf::from(path);
    tokio::task::spawn_blocking(move || CsvProvider::preview(&path_buf, skip_header_rows))
        .await
        .map_err(|e| AppError::new("internal", format!("join: {e}")))?
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn import_csv(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<ImportSummary> {
    let db = (*state.db).clone();
    let path = PathBuf::from(path);
    let app_emit = app.clone();

    let summary = tokio::task::spawn_blocking(move || {
        CsvProvider::import(&path, &account_id, &mapping, &db, |p| {
            let _ = app_emit.emit(
                "import.progress",
                ProgressPayload {
                    import_id: String::new(), // unknown until summary returned
                    rows_done: p.rows_done,
                    rows_total: p.rows_total,
                },
            );
        })
        .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::new("internal", format!("join: {e}")))?;

    let summary = summary?;
    app.emit("import.complete", &summary.import_id).ok();
    Ok(summary)
}

#[tauri::command]
#[specta::specta]
pub async fn list_unfinished_imports(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<imports_repo::Import>> {
    let db = (*state.db).clone();
    run(&db, |conn| imports_repo::list_unfinished(conn))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn discard_unfinished_import(
    state: tauri::State<'_, AppState>,
    import_id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        imports_repo::finish(conn, &import_id, 0, 0, Some("discarded"))
    })
    .await
    .map_err(AppError::from)
}

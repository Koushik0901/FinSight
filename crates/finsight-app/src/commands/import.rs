use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::{imports as imports_repo, run};
use finsight_providers::{CsvImportMapping, CsvPreview, CsvProvider, ImportSummary};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use tauri::Emitter;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Type)]
pub struct ProgressPayload {
    pub import_id: String,
    pub rows_done: u32,
    pub rows_total: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn preview_csv_columns(path: String, skip_header_rows: u32) -> AppResult<CsvPreview> {
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
    // Pre-generate the import_id so progress events carry it before the summary is returned.
    let import_id = Uuid::new_v4().to_string();
    let import_id_for_progress = import_id.clone();

    let summary = tokio::task::spawn_blocking(move || {
        CsvProvider::import(&path, &account_id, &import_id, &mapping, &db, |p| {
            let _ = app_emit.emit(
                "import-progress",
                ProgressPayload {
                    import_id: import_id_for_progress.clone(),
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

    // Deterministic, provider-free baseline categorization. Runs on every import
    // so common merchants get a stable category even with no LLM provider
    // configured; the AI categorizer (if a provider is set) still refines the
    // rest. Best-effort — a failure here must not fail the import itself.
    {
        let cat_db = (*state.db).clone();
        let _ = run(&cat_db, finsight_core::categorize::apply_builtin_categorization).await;
    }
    // Recompute statistical anomaly flags from the (now larger) history.
    // Best-effort — must not fail the import.
    {
        let anom_db = (*state.db).clone();
        let _ = run(&anom_db, finsight_core::anomaly::recompute_anomalies).await;
    }
    // Refresh the derived balance + net-worth trend from the new activity so the
    // Today/Accounts numbers and the net-worth chart populate immediately.
    {
        let nw_db = (*state.db).clone();
        let _ = run(&nw_db, |conn| {
            finsight_core::repos::net_worth::record_today(conn)?;
            finsight_core::repos::net_worth::backfill_history_from_transactions(conn)
        })
        .await;
    }

    // Auto-categorize with the configured AI provider when the setting is on
    // (default). The deterministic builtin pass above only covers well-known
    // merchants; this enqueues the LLM categorizer for the long tail so import
    // actually honours the "categorize after each import" promise. Best-effort:
    // a missing provider or full queue must not fail the import (the user can
    // still re-run the scan manually from Settings / Insights).
    {
        let cfg_db = (*state.db).clone();
        let auto = run(&cfg_db, |conn| {
            let v: Option<bool> =
                finsight_core::settings::get(conn, crate::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY)?;
            Ok(v.unwrap_or(true))
        })
        .await
        .unwrap_or(true);
        if auto {
            let _ = state
                .agent
                .tx
                .try_send(finsight_agent::agent::AgentJob::CategorizeAll);
        }
    }

    app.emit("import-complete", &summary).ok();

    let notify_app = app.clone();
    let notify_db = (*state.db).clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::notifications::check_and_fire(&notify_app, &notify_db).await;
    });

    Ok(summary)
}

/// The CSV import mapping (columns, date format, amount handling) last used for
/// this account, so a recurring import from the same bank can pre-fill and the
/// user never re-picks the same settings. `None` when the account has never been
/// imported into.
#[tauri::command]
#[specta::specta]
pub async fn get_saved_csv_mapping(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<Option<CsvImportMapping>> {
    let db = (*state.db).clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.get().map_err(AppError::from)?;
        finsight_providers::csv::mapping::load(&conn, &account_id).map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::new("internal", format!("join: {e}")))?
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

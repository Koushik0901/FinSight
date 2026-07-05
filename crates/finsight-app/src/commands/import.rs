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

/// A lightweight, bounded preview of what an import WOULD do — counts + a
/// capped error list + a staleness signature — so the UI can show
/// "N new · D duplicates · R to review" before the user commits to importing.
/// Deliberately excludes per-row decisions: those can number in the
/// thousands and must never cross the Tauri IPC boundary wholesale.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreparedImportPreview {
    pub signature: String,
    pub rows_total: u32,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub rows_queued_for_review: u32,
    pub errors: Vec<finsight_providers::csv::RowError>,
}

/// Build a bounded preview of the import outcome. Testable without a Tauri
/// handle; the `prepare_csv_import` command below is a thin async wrapper.
pub fn build_preview(
    db: &finsight_core::Db,
    path: &std::path::Path,
    account_id: &str,
    mapping: &CsvImportMapping,
) -> AppResult<PreparedImportPreview> {
    let conn = db.get().map_err(AppError::from)?;
    let p = finsight_providers::csv::CsvProvider::prepare(path, account_id, mapping, &conn)
        .map_err(AppError::from)?;
    // rows_total must reflect the TRUE data-row count (decisions + all errors),
    // computed before truncating the error payload for IPC.
    let total_errors = p.errors.len() as u32;
    let rows_total = p.rows.len() as u32 + total_errors;
    let mut errors = p.errors;
    errors.truncate(50); // never ship an unbounded per-row payload over IPC
    Ok(PreparedImportPreview {
        signature: p.signature,
        rows_total,
        rows_imported: p.rows_imported,
        rows_skipped_duplicates: p.rows_skipped_duplicates,
        rows_queued_for_review: p.rows_queued_for_review,
        errors,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_csv_import(
    state: tauri::State<'_, AppState>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<PreparedImportPreview> {
    let db = (*state.db).clone();
    let path = PathBuf::from(path);
    tokio::task::spawn_blocking(move || build_preview(&db, &path, &account_id, &mapping))
        .await
        .map_err(|e| AppError::new("internal", format!("join: {e}")))?
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
    // Snapshot the reset generation before we touch the ledger. If a Delete-All
    // lands while this import runs, the post-commit derived-state cascade below
    // is skipped — otherwise `ensure_default_categories` (inside the builtin
    // categorization step) would re-seed categories, and net-worth/anomaly
    // recompute would repopulate derived state, into a ledger the user just
    // wiped.
    let reset_epoch_at_start = state.agent.reset_generation();
    // Keep the target account id for the post-commit cascade (the original is
    // moved into the blocking import closure below).
    let cascade_account_id = account_id.clone();
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

    // Post-commit derived-state cascade. Skip the whole thing if a Delete-All
    // landed during the import — re-seeding categories / recomputing net worth
    // into a freshly-wiped ledger is exactly the stale-write the reset must
    // prevent. Each step re-checks so a wipe mid-cascade stops the remainder.
    let wiped = || state.agent.reset_generation() != reset_epoch_at_start;
    if !wiped() {
        // Deterministic, provider-free baseline categorization. Runs on every
        // import so common merchants get a stable category even with no LLM
        // provider configured; the AI categorizer (if a provider is set) still
        // refines the rest. Best-effort — a failure here must not fail the
        // import itself.
        {
            let cat_db = (*state.db).clone();
            let _ = run(&cat_db, finsight_core::categorize::apply_builtin_categorization).await;
        }
        // Pair cross-account transfer legs (withdrawal ↔ matching deposit) now
        // that both sides may exist. Runs after the keyword pass, which supplies
        // the flagged anchors. Best-effort — must not fail the import.
        if !wiped() {
            let pair_db = (*state.db).clone();
            let _ = run(&pair_db, finsight_core::categorize::pair_transfers).await;
        }
        // Recompute statistical anomaly flags from the (now larger) history.
        // Scoped to the imported account's merchants: only those groups can
        // have shifted, and this leaves every other merchant's flags untouched
        // (proven equivalent to the full recompute in anomaly.rs tests).
        // Best-effort — must not fail the import.
        if !wiped() {
            let anom_db = (*state.db).clone();
            let acct = cascade_account_id.clone();
            let _ = run(&anom_db, move |conn| {
                finsight_core::anomaly::recompute_anomalies_for_account(conn, &acct)
            })
            .await;
        }
        // Refresh the derived balance + net-worth trend from the new activity so
        // the Today/Accounts numbers and the net-worth chart populate
        // immediately.
        if !wiped() {
            let nw_db = (*state.db).clone();
            let _ = run(&nw_db, |conn| {
                finsight_core::repos::net_worth::record_today(conn)?;
                finsight_core::repos::net_worth::backfill_history_from_transactions(conn)
            })
            .await;
        }
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

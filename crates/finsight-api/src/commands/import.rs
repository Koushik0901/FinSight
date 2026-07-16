use crate::error::{AppError, AppResult};
use crate::sink::FrameSink;
use crate::ApiState;
use finsight_core::repos::{imports as imports_repo, run};
use finsight_providers::{CsvImportMapping, CsvPreview, CsvProvider, ImportSummary};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
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

pub async fn prepare_csv_import(
    state: &ApiState,
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

pub async fn preview_csv_columns(path: String, skip_header_rows: u32) -> AppResult<CsvPreview> {
    let path_buf = PathBuf::from(path);
    tokio::task::spawn_blocking(move || CsvProvider::preview(&path_buf, skip_header_rows))
        .await
        .map_err(|e| AppError::new("internal", format!("join: {e}")))?
        .map_err(AppError::from)
}

/// The import outcome plus what still needs a category. Surfacing the
/// uncategorized count (and whether the AI pass was auto-started) makes the
/// cloud LLM categorization a visible, informed choice rather than a silent
/// background enqueue.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub summary: ImportSummary,
    /// Rows the deterministic builtin pass categorized in this import's
    /// cascade — shown post-import so the derived work is visible, not silent.
    pub builtin_categorized: u32,
    /// Cross-account transfer pairs linked in this import's cascade.
    pub transfers_paired: u32,
    /// Uncategorized non-transfer EXPENSE rows in this account after the builtin
    /// pass — exactly what an AI categorization run would work on.
    pub uncategorized_after: i64,
    /// True when the background AI categorizer was auto-enqueued (the
    /// auto-categorize setting is on). When false, the UI offers an explicit
    /// "run AI categorization" action.
    pub ai_categorization_started: bool,
}

/// Import a CSV file, running the deterministic post-import cascade
/// (categorization, transfer pairing, anomaly refresh, net-worth refresh) and
/// enqueuing the AI categorizer. Progress and completion are pushed through the
/// `sink` (`"import-progress"` / `"import-complete"`, unchanged event names and
/// payload shapes) — the Tauri wrapper feeds a `TauriFrameSink` that emits real
/// window events, and ALSO fires the desktop "check_and_fire" notification
/// after this returns (that notification is native-only and stays in the
/// wrapper, not here — see `crates/finsight-app/src/commands/import.rs`).
pub async fn import_csv(
    state: &ApiState,
    sink: Arc<dyn FrameSink>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<ImportResult> {
    let db = (*state.db).clone();
    let path = PathBuf::from(path);
    // Coordinate with Delete-All. Snapshot the ledger epoch and hold a writer
    // lease across the entire import + derived-state cascade. A concurrent
    // Delete-All cannot complete its wipe until this lease drains, and if a
    // reset already happened (or lands mid-import) `superseded()` reports it —
    // so nothing this import writes can survive a Delete-All that reports
    // success: either our writes commit before the wipe (and it removes them),
    // or we observe the advanced epoch and stop.
    let start_epoch = db.reset_barrier().epoch();
    let reset_lease = db.reset_barrier().writer_lease(start_epoch).await;
    if reset_lease.superseded() {
        return Err(AppError::new(
            "reset",
            "Import cancelled: all data was cleared as this import began.",
        ));
    }
    // Keep the target account id for the post-commit cascade (the original is
    // moved into the blocking import closure below).
    let cascade_account_id = account_id.clone();
    // Pre-generate the import_id so progress events carry it before the summary is returned.
    let import_id = Uuid::new_v4().to_string();
    let import_id_for_progress = import_id.clone();

    let import_db = db.clone();
    let progress_sink = Arc::clone(&sink);
    let summary = tokio::task::spawn_blocking(move || {
        CsvProvider::import(&path, &account_id, &import_id, &mapping, &import_db, |p| {
            progress_sink.emit(
                "import-progress",
                serde_json::to_value(ProgressPayload {
                    import_id: import_id_for_progress.clone(),
                    rows_done: p.rows_done,
                    rows_total: p.rows_total,
                })
                .expect("ProgressPayload serializes"),
            );
        })
        .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::new("internal", format!("join: {e}")))?;

    let summary = summary?;

    // Post-commit derived-state cascade. Skip it if a Delete-All has landed
    // while we held the lease — re-seeding categories / recomputing net worth
    // into a ledger the user just wiped is exactly the stale write the reset
    // must prevent. `superseded()` also lets us drop the doomed work promptly so
    // the pending reset drains faster; the lease guarantees correctness even if
    // this raced.
    let wiped = || reset_lease.superseded();
    let mut builtin_categorized: u32 = 0;
    let mut transfers_paired: u32 = 0;
    if !wiped() {
        // Deterministic, provider-free baseline categorization. Runs on every
        // import so common merchants get a stable category even with no LLM
        // provider configured; the AI categorizer (if a provider is set) still
        // refines the rest. Best-effort — a failure here must not fail the
        // import itself. The count is surfaced in the result so the cascade's
        // work is visible instead of silent.
        {
            let cat_db = (*state.db).clone();
            builtin_categorized = run(
                &cat_db,
                finsight_core::categorize::apply_builtin_categorization,
            )
            .await
            .unwrap_or(0);
        }
        // Pair cross-account transfer legs (withdrawal ↔ matching deposit) now
        // that both sides may exist. Runs after the keyword pass, which supplies
        // the flagged anchors. Best-effort — must not fail the import.
        if !wiped() {
            let pair_db = (*state.db).clone();
            transfers_paired = run(&pair_db, finsight_core::categorize::pair_transfers)
                .await
                .unwrap_or(0);
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
    // Release the lease now that all import + cascade writes are done. The AI
    // categorizer enqueued below takes its own lease when it runs, so it stays
    // coordinated with Delete-All without us holding this one across the queue.
    drop(reset_lease);

    // Auto-categorize with the configured AI provider when the setting is on
    // (default). The deterministic builtin pass above only covers well-known
    // merchants; this enqueues the LLM categorizer for the long tail so import
    // actually honours the "categorize after each import" promise. Best-effort:
    // a missing provider or full queue must not fail the import (the user can
    // still re-run the scan manually from Settings / Insights).
    let ai_categorization_started = {
        let cfg_db = (*state.db).clone();
        let auto = run(&cfg_db, |conn| {
            let v: Option<bool> = finsight_core::settings::get(
                conn,
                crate::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY,
            )?;
            Ok(v.unwrap_or(true))
        })
        .await
        .unwrap_or(true);
        auto && state
            .agent
            .tx
            .try_send(finsight_agent::agent::AgentJob::CategorizeAll)
            .is_ok()
    };

    // How many rows still have no category after the builtin pass — what an AI
    // run would work on, and what the UI surfaces so the LLM pass is an informed
    // choice, not a silent enqueue.
    let count_db = (*state.db).clone();
    let count_acct = cascade_account_id.clone();
    let uncategorized_after = run(&count_db, move |conn| {
        conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM transactions t \
                 WHERE account_id = ?1 AND category_id IS NULL AND amount_cents < 0 \
                   AND is_transfer = 0 AND {}",
                finsight_core::metrics::non_investment_txn_predicate("t")
            ),
            rusqlite::params![count_acct],
            |r| r.get::<_, i64>(0),
        )
        .map_err(finsight_core::CoreError::from)
    })
    .await
    .unwrap_or(0);

    let result = ImportResult {
        summary,
        builtin_categorized,
        transfers_paired,
        uncategorized_after,
        ai_categorization_started,
    };
    sink.emit(
        "import-complete",
        serde_json::to_value(&result).expect("ImportResult serializes"),
    );

    Ok(result)
}

/// The CSV import mapping (columns, date format, amount handling) last used for
/// this account, so a recurring import from the same bank can pre-fill and the
/// user never re-picks the same settings. `None` when the account has never been
/// imported into.
pub async fn get_saved_csv_mapping(
    state: &ApiState,
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

pub async fn list_unfinished_imports(state: &ApiState) -> AppResult<Vec<imports_repo::Import>> {
    let db = (*state.db).clone();
    run(&db, |conn| imports_repo::list_unfinished(conn))
        .await
        .map_err(AppError::from)
}

pub async fn discard_unfinished_import(state: &ApiState, import_id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        imports_repo::finish(conn, &import_id, 0, 0, Some("discarded"))
    })
    .await
    .map_err(AppError::from)
}

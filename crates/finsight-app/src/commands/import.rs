use crate::commands::TauriFrameSink;
use crate::error::AppResult;
use crate::AppState;
use finsight_providers::{CsvImportMapping, CsvPreview};
use std::sync::Arc;

// Types + the tauri-free preview helper live in finsight-api now; re-exported
// so existing imports of `finsight_app::commands::import::*` (lib.rs, tests —
// e.g. `prepare_csv_cmd.rs`'s direct call to `build_preview`) keep resolving.
pub use finsight_api::commands::import::{
    build_preview, ImportResult, PreparedImportPreview, ProgressPayload,
};

#[tauri::command]
#[specta::specta]
pub async fn prepare_csv_import(
    state: tauri::State<'_, AppState>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<PreparedImportPreview> {
    finsight_api::commands::import::prepare_csv_import(&state.api, path, account_id, mapping).await
}

#[tauri::command]
#[specta::specta]
pub async fn preview_csv_columns(path: String, skip_header_rows: u32) -> AppResult<CsvPreview> {
    finsight_api::commands::import::preview_csv_columns(path, skip_header_rows).await
}

// Imports the CSV, then best-effort fires the desktop "import complete"
// notification. Progress/completion events flow through a `TauriFrameSink`
// into real Tauri window events ("import-progress" / "import-complete",
// unchanged names + payload shapes). The notification uses `tauri::AppHandle`
// directly (native notification plugin) and so stays here in the wrapper —
// the finsight-api body has no tauri dependency.
// (Plain `//` on purpose: `///` doc comments flow into the generated
// bindings.ts and would break the Phase 1 bindings zero-diff invariant.)
#[tauri::command]
#[specta::specta]
pub async fn import_csv(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<ImportResult> {
    let sink: Arc<dyn finsight_api::sink::FrameSink> =
        Arc::new(TauriFrameSink(app.clone()));
    let result =
        finsight_api::commands::import::import_csv(&state.api, sink, path, account_id, mapping)
            .await?;

    let notify_db = (*state.api.db).clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::notifications::check_and_fire(&app, &notify_db).await;
    });

    Ok(result)
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
    finsight_api::commands::import::get_saved_csv_mapping(&state.api, account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_unfinished_imports(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<finsight_core::repos::imports::Import>> {
    finsight_api::commands::import::list_unfinished_imports(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn discard_unfinished_import(
    state: tauri::State<'_, AppState>,
    import_id: String,
) -> AppResult<()> {
    finsight_api::commands::import::discard_unfinished_import(&state.api, import_id).await
}

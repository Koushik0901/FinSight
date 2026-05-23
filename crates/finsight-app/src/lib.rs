//! FinSight Tauri app — command surface + lifecycle.

pub mod commands;
pub mod error;

use finsight_core::db::run_migrations;
use finsight_core::Db;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Db>,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { db: Arc::new(db) }
    }
}

/// Build the tauri-specta builder with all commands registered.
/// Shared between the Tauri app and the `export_bindings` binary so the
/// generated TS bindings stay in sync with what Tauri actually exposes.
pub fn build_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::accounts::list_accounts,
        commands::accounts::create_account,
        commands::transactions::list_transactions,
        commands::transactions::create_transaction,
        commands::onboarding::get_onboarding_state,
        commands::onboarding::seed_sample_household,
        commands::onboarding::mark_onboarding_complete,
        commands::onboarding::reset_onboarding_completion,
        commands::onboarding::clear_sample_data,
        commands::onboarding::commit_starter_categories,
        commands::onboarding::probe_ollama,
        commands::onboarding::save_llm_provider,
        commands::meta::app_ready,
        commands::import::preview_csv_columns,
        commands::import::import_csv,
        commands::import::list_unfinished_imports,
        commands::import::discard_unfinished_import,
    ])
}

const SERVICE: &str = "com.finsight.app";
const USER: &str = "default";

/// Configure a `tauri::Builder` with our plugins, invoke handler, and lifecycle
/// setup. The caller (the `src-tauri` binary) is responsible for the final
/// `.run(generate_context!())` because `generate_context!` must be expanded in
/// the crate that owns `tauri.conf.json`.
pub fn configure_app(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let specta = build_specta_builder();

    builder
        // Single-instance: two windows on the same encrypted DB would deadlock on WAL locks.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("data.sqlcipher");

            let key = finsight_core::keychain::load_or_create_key(SERVICE, USER)
                .map_err(|e| -> Box<dyn std::error::Error> {
                    format!("keychain error: {e}").into()
                })?;

            let db = Db::open(&db_path, &key).map_err(|e| -> Box<dyn std::error::Error> {
                format!("db open error: {e}").into()
            })?;
            run_migrations(&db).map_err(|e| -> Box<dyn std::error::Error> {
                format!("migrations: {e}").into()
            })?;

            app.manage(AppState { db: Arc::new(db) });
            Ok(())
        })
}

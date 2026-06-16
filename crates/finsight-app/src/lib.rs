//! FinSight Tauri app — command surface + lifecycle.

pub mod commands;
pub mod error;

use finsight_agent::{
    agent::{AgentEvent, AgentHandle, EventCallback},
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider, openai_compat::OpenAiCompatProvider,
    },
    CompletionProvider,
};
use finsight_core::{db::run_migrations, settings, Db};
use std::sync::{Arc, RwLock};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Arc<Db>,
    pub agent: AgentHandle,
    pub agent_provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
}

impl AppState {
    pub fn new(db: Db, on_event: EventCallback) -> Self {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let agent = AgentHandle::spawn(db.clone(), Arc::clone(&provider), on_event);
        Self {
            db: Arc::new(db),
            agent,
            agent_provider: provider,
        }
    }
}

/// Migrate legacy `llm_provider` key → `completion_provider`.
/// Called from `configure_app` setup before managing AppState.
/// Exported for integration tests.
pub fn migrate_provider_settings(db: &Db) -> Result<(), finsight_core::CoreError> {
    let conn = db.get()?;
    // Only migrate if completion_provider is absent
    let new_cfg: Option<serde_json::Value> = settings::get(&conn, "completion_provider")?;
    if new_cfg.is_some() {
        return Ok(());
    }
    let old_cfg: Option<serde_json::Value> = settings::get(&conn, "llm_provider")?;
    let Some(old) = old_cfg else { return Ok(()) };
    let migrated = match old.get("kind").and_then(|k| k.as_str()) {
        Some("ollama") => serde_json::json!({
            "kind": "ollama",
            "base_url": old["base_url"],
            "model": old["completion_model"]
        }),
        _ => serde_json::json!({ "kind": "unconfigured" }),
    };
    settings::set(&conn, "completion_provider", &migrated)?;
    Ok(())
}

/// Load the saved CompletionProviderConfig from settings and instantiate the provider.
/// Returns None if unconfigured or key absent.
pub fn load_provider_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    let conn = db.get().ok()?;
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").ok()??;
    build_provider_from_config(&cfg)
}

pub(crate) fn build_provider_from_config(
    cfg: &serde_json::Value,
) -> Option<Arc<dyn CompletionProvider>> {
    match cfg.get("kind")?.as_str()? {
        "ollama" => {
            let base_url = cfg["base_url"].as_str()?.to_string();
            let model = cfg["model"].as_str()?.to_string();
            Some(Arc::new(OllamaProvider::new(base_url, model)))
        }
        "openai_compat" => {
            let base_url = cfg["base_url"].as_str()?.to_string();
            let model = cfg["model"].as_str()?.to_string();
            let preset = cfg["preset"].as_str().unwrap_or("custom").to_string();
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", &preset)
                .ok()??
                .to_string();
            Some(Arc::new(OpenAiCompatProvider::new(
                base_url, api_key, model, preset,
            )))
        }
        "anthropic" => {
            let model = cfg["model"].as_str()?.to_string();
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", "anthropic")
                .ok()??
                .to_string();
            Some(Arc::new(AnthropicProvider::new(api_key, model)))
        }
        _ => None,
    }
}

/// Build the tauri-specta builder with all commands registered.
/// Shared between the Tauri app and the `export_bindings` binary so the
/// generated TS bindings stay in sync with what Tauri actually exposes.
pub fn build_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::accounts::list_accounts,
        commands::accounts::create_account,
        commands::accounts::update_account,
        commands::accounts::archive_account,
        commands::transactions::list_transactions,
        commands::transactions::create_transaction,
        commands::transactions::update_transaction,
        commands::transactions::delete_transaction,
        commands::transactions::create_rule,
        commands::transactions::list_categories,
        commands::onboarding::get_onboarding_state,
        commands::onboarding::seed_sample_household,
        commands::onboarding::seed_dev_demo,
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
        commands::agent::set_completion_provider,
        commands::agent::save_provider_api_key,
        commands::agent::list_provider_models,
        commands::agent::test_completion_provider,
        commands::agent::get_needs_review_count,
        commands::agent::trigger_categorize,
        commands::transactions::list_categories_with_spending,
        commands::transactions::list_rules_with_categories,
        commands::transactions::toggle_rule,
        commands::budget::list_budget_envelopes,
        commands::budget::set_budget,
        commands::budget::list_goals,
        commands::budget::create_goal,
        commands::budget::update_goal_balance,
        commands::budget::archive_goal,
        commands::recurring::list_recurring,
        commands::reports::get_report_data,
        commands::reports::get_month_totals,
        commands::scenarios::run_scenario,
        commands::scenarios::save_scenario,
        commands::scenarios::list_scenario_history,
        commands::scenarios::delete_scenario,
        commands::transactions::get_transaction_count,
        commands::assets::list_manual_assets,
        commands::assets::create_manual_asset,
        commands::assets::update_manual_asset,
        commands::assets::delete_manual_asset,
        commands::assets::list_liabilities,
        commands::assets::create_liability,
        commands::assets::update_liability,
        commands::assets::delete_liability,
        commands::assets::record_net_worth_snapshot,
        commands::assets::list_net_worth_history,
        commands::insights::list_agent_memory,
        commands::insights::forget_agent_memory,
        commands::agent::list_rule_proposals,
        commands::agent::accept_rule_proposal,
        commands::agent::decline_rule_proposal,
        commands::transactions::set_transaction_flags,
        commands::budget::update_goal_monthly,
        commands::settings::get_currency,
        commands::settings::set_currency,
        commands::settings::export_all_data_json,
        commands::settings::export_all_data_csv,
        commands::budget::get_plan_next_month_data,
        commands::budget::apply_next_month_plan,
        commands::budget::list_budget_history,
        commands::agent::list_recent_agent_activity,
        commands::transactions::export_transactions_csv,
        commands::accounts::export_account_csv,
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
            migrate_provider_settings(&db).map_err(|e| -> Box<dyn std::error::Error> {
                format!("provider migration: {e}").into()
            })?;

            // Best-effort: record today's net-worth snapshot on startup.
            if let Ok(mut conn) = db.get() {
                let _ = finsight_core::repos::net_worth::record_today(&mut conn);
            }

            let window = app.get_webview_window("main").expect("main window");
            let on_event: EventCallback = Arc::new(move |event| {
                let (event_name, payload) = match &event {
                    AgentEvent::CategorizationProgress { .. } =>
                        ("categorization.progress", serde_json::to_value(&event).unwrap()),
                    AgentEvent::CategorizationComplete { .. } =>
                        ("categorization.complete", serde_json::to_value(&event).unwrap()),
                    AgentEvent::Error { .. } =>
                        ("agent.error", serde_json::to_value(&event).unwrap()),
                };
                let _ = window.emit(event_name, payload);
            });

            let state = AppState::new(db.clone(), on_event);
            // Load saved provider configuration and wire it into the agent
            if let Some(provider) = load_provider_from_settings(&db) {
                state.agent.set_provider(provider);
            }
            app.manage(state);
            Ok(())
        })
}

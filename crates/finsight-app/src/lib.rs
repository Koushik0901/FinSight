//! FinSight Tauri app — command surface + lifecycle.

pub mod commands;
pub mod error;
pub mod notifications;
pub mod sync_scheduler;

use finsight_agent::{
    agent::{AgentEvent, AgentHandle, EventCallback},
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider, openai_compat::OpenAiCompatProvider,
    },
    CompletionProvider,
};
use finsight_core::{db::run_migrations, settings, Db};
use std::sync::{Arc, RwLock};
use sync_scheduler::SyncScheduler;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Arc<Db>,
    pub agent: AgentHandle,
    pub agent_provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    pub sync_scheduler: SyncScheduler,
}

impl AppState {
    pub fn new(db: Db, on_event: EventCallback) -> Self {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let agent = AgentHandle::spawn(db.clone(), Arc::clone(&provider), on_event);
        let sync_scheduler = SyncScheduler::new(db.clone());
        Self {
            db: Arc::new(db),
            agent,
            agent_provider: provider,
            sync_scheduler,
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

/// Default OpenRouter base URL used when seeding a provider from `.env`.
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
/// Default model used when seeding a provider from `.env`.
pub const DEFAULT_OPENROUTER_MODEL: &str = "google/gemma-4-31b-it";
/// Keychain preset id under which the OpenRouter key is stored.
pub const OPENROUTER_PRESET: &str = "openrouter";

/// Read a single key from a `.env` file without pulling in a dotenv crate or
/// polluting the process environment with every entry. Walks up from `start`
/// looking for `.env`, then returns the value of `wanted` (unquoted, trimmed).
///
/// SECURITY: the returned value is a secret. Callers must never log, print, or
/// serialize it into settings/error output.
fn read_env_key_from_file(start: &std::path::Path, wanted: &str) -> Option<String> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(".env");
        if let Ok(contents) = std::fs::read_to_string(&candidate) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let line = line.strip_prefix("export ").unwrap_or(line);
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                if k.trim() != wanted {
                    continue;
                }
                let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
                if v.is_empty() {
                    return None;
                }
                return Some(v.to_string());
            }
        }
        dir = d.parent();
    }
    None
}

/// Resolve the OpenRouter API key from the process environment first, then a
/// `.env` file discovered by walking up from the current working directory.
fn resolve_openrouter_key() -> Option<String> {
    if let Ok(k) = std::env::var("OPENROUTER_API_KEY") {
        let k = k.trim();
        if !k.is_empty() {
            return Some(k.to_string());
        }
    }
    let cwd = std::env::current_dir().ok()?;
    read_env_key_from_file(&cwd, "OPENROUTER_API_KEY")
}

/// True when `completion_provider` is present and not `unconfigured`.
fn provider_already_configured(db: &Db) -> Result<bool, finsight_core::CoreError> {
    let conn = db.get()?;
    let existing: Option<serde_json::Value> = settings::get(&conn, "completion_provider")?;
    Ok(existing
        .as_ref()
        .and_then(|v| v.get("kind"))
        .and_then(|k| k.as_str())
        .map(|kind| kind != "unconfigured")
        .unwrap_or(false))
}

/// Seed an OpenRouter/Gemma provider using `key`, but only when no provider is
/// configured yet (preserving any user override). Stores the secret in the OS
/// keychain and writes an `openai_compat`/`openrouter` config pointing at the
/// default Gemma model. The key never touches the settings row or logs.
///
/// Returns `Ok(true)` when a new provider was seeded.
pub fn seed_openrouter_provider_if_unconfigured(
    db: &Db,
    key: &str,
) -> Result<bool, finsight_core::CoreError> {
    if key.trim().is_empty() || provider_already_configured(db)? {
        return Ok(false);
    }
    // Store the secret in the OS keychain (never in the settings row).
    finsight_core::keychain::set_key("com.finsight.llm", OPENROUTER_PRESET, key.trim())?;

    let cfg = serde_json::json!({
        "kind": "openai_compat",
        "preset": OPENROUTER_PRESET,
        "base_url": OPENROUTER_BASE_URL,
        "model": DEFAULT_OPENROUTER_MODEL,
    });
    let conn = db.get()?;
    settings::set(&conn, "completion_provider", &cfg)?;
    Ok(true)
}

/// Bootstrap an OpenRouter/Gemma provider from `.env` on startup.
///
/// Only seeds when `completion_provider` is missing or `unconfigured`. Resolves
/// the key from the process environment, then a `.env` file discovered by
/// walking up from the current working directory.
///
/// Returns `Ok(true)` when a new provider was seeded.
pub fn bootstrap_env_provider(db: &Db) -> Result<bool, finsight_core::CoreError> {
    if provider_already_configured(db)? {
        return Ok(false);
    }
    let Some(key) = resolve_openrouter_key() else {
        return Ok(false);
    };
    seed_openrouter_provider_if_unconfigured(db, &key)
}

/// Load the saved CompletionProviderConfig from settings and instantiate the provider.
/// Returns None if unconfigured or key absent.
pub fn load_provider_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    let conn = db.get().ok()?;
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").ok()??;
    build_provider_from_config(&cfg)
}

/// Load the raw CompletionProviderConfig from settings.
/// Returns Unconfigured when the setting is missing.
pub fn load_completion_provider_config(
    db: &Db,
) -> Result<commands::agent::CompletionProviderConfig, finsight_core::CoreError> {
    let conn = db.get()?;
    let cfg: Option<serde_json::Value> = settings::get(&conn, "completion_provider")?;
    match cfg {
        Some(v) => serde_json::from_value(v).map_err(|e| {
            finsight_core::CoreError::InvalidState(format!("completion_provider parse: {e}"))
        }),
        None => Ok(commands::agent::CompletionProviderConfig::Unconfigured),
    }
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
        commands::accounts::set_account_balance,
        commands::categories::update_category_color,
        commands::categories::create_category,
        commands::categories::rename_category,
        commands::categories::archive_category,
        commands::categories::set_category_guidance,
        commands::transactions::list_transactions,
        commands::transactions::create_transaction,
        commands::transactions::update_transaction,
        commands::transactions::delete_transaction,
        commands::transactions::create_rule,
        commands::transactions::list_categories,
        commands::transactions::set_category_spending_type,
        commands::transactions::get_spending_breakdown,
        commands::onboarding::get_onboarding_state,
        commands::onboarding::mark_onboarding_complete,
        commands::onboarding::reset_onboarding_completion,
        commands::onboarding::commit_starter_categories,
        commands::onboarding::probe_ollama,
        commands::onboarding::save_llm_provider,
        commands::meta::app_ready,
        commands::import::preview_csv_columns,
        commands::import::import_csv,
        commands::import::get_saved_csv_mapping,
        commands::import::list_unfinished_imports,
        commands::import::discard_unfinished_import,
        commands::agent::set_completion_provider,
        commands::agent::get_completion_provider,
        commands::agent::save_provider_api_key,
        commands::agent::list_provider_models,
        commands::agent::test_completion_provider,
        commands::agent::get_needs_review_count,
        commands::agent::trigger_categorize,
        commands::agent::recompute_anomalies,
        commands::agent::trigger_recategorize_low_confidence,
        commands::agent::get_agent_status,
        commands::agent::ask_agent,
        commands::transactions::list_categories_with_spending,
        commands::transactions::list_rules_with_categories,
        commands::transactions::toggle_rule,
        commands::budget::list_budget_envelopes,
        commands::budget::set_budget,
        commands::budget::list_goals,
        commands::budget::create_goal,
        commands::budget::update_goal_balance,
        commands::budget::archive_goal,
        commands::budget::project_goal_growth,
        commands::recurring::list_recurring,
        commands::reports::get_report_data,
        commands::reports::get_month_totals,
        commands::reports::get_savings_rate_history,
        commands::reports::create_monthly_review,
        commands::reports::list_monthly_reviews,
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
        commands::assets::compute_debt_payoff,
        commands::assets::get_uncelebrated_milestones,
        commands::insights::list_agent_memory,
        commands::insights::forget_agent_memory,
        commands::insights::get_financial_health_score,
        commands::agent::list_rule_proposals,
        commands::agent::accept_rule_proposal,
        commands::agent::decline_rule_proposal,
        commands::copilot::list_agent_sessions,
        commands::copilot::create_agent_session,
        commands::copilot::close_agent_session,
        commands::copilot::list_action_bundles,
        commands::copilot::get_action_bundle,
        commands::copilot::approve_action_item,
        commands::copilot::reject_action_item,
        commands::copilot::list_execution_log,
        commands::copilot::execute_action_bundle,
        commands::recipes::list_recipes,
        commands::recipes::create_recipe,
        commands::recipes::update_recipe,
        commands::recipes::pause_recipe,
        commands::recipes::resume_recipe,
        commands::recipes::delete_recipe,
        commands::recipes::trigger_recipe,
        commands::recipes::list_recipe_runs,
        commands::transactions::set_transaction_flags,
        commands::transactions::get_transaction_splits,
        commands::transactions::set_transaction_splits,
        commands::budget::update_goal_monthly,
        commands::budget::update_goal_purpose,
        commands::settings::get_currency,
        commands::settings::set_currency,
        commands::settings::delete_all_data,
        commands::settings::export_all_data_json,
        commands::settings::export_all_data_csv,
        commands::settings::get_notifications_enabled,
        commands::settings::set_notifications_enabled,
        commands::settings::get_auto_categorize_enabled,
        commands::settings::set_auto_categorize_enabled,
        commands::budget::get_plan_next_month_data,
        commands::budget::apply_next_month_plan,
        commands::budget::list_budget_history,
        commands::agent::list_recent_agent_activity,
        commands::planned_transactions::list_planned_transactions,
        commands::planned_transactions::get_planned_transaction,
        commands::planned_transactions::create_planned_transaction,
        commands::planned_transactions::update_planned_transaction,
        commands::planned_transactions::delete_planned_transaction,
        commands::transactions::export_transactions_csv,
        commands::accounts::export_account_csv,
        commands::accounts::list_account_balance_history,
        commands::accounts::list_account_balance_sparklines,
        commands::journey::get_journey_status,
        commands::inbox::get_action_items,
        commands::simplefin::save_simplefin_setup_token,
        commands::simplefin::get_simplefin_status,
        commands::simplefin::list_simplefin_connections,
        commands::simplefin::list_simplefin_accounts,
        commands::simplefin::import_simplefin_accounts,
        commands::simplefin::sync_simplefin_account,
        commands::simplefin::disconnect_simplefin,
        commands::simplefin::purge_simplefin_data,
        commands::simplefin::delete_simplefin_connection,
        commands::simplefin::sync_all_simplefin_accounts,
        commands::simplefin::get_simplefin_sync_settings,
        commands::simplefin::set_simplefin_sync_settings,
        commands::simplefin::list_simplefin_alerts,
        commands::simplefin::acknowledge_simplefin_alert,
        commands::simplefin::list_simplefin_transfer_suggestions,
        commands::simplefin::confirm_simplefin_transfer,
        commands::simplefin::reject_simplefin_transfer,
        commands::simplefin::list_import_review_candidates,
        commands::simplefin::accept_import_candidate_match,
        commands::simplefin::create_import_candidate_transaction,
        commands::simplefin::dismiss_import_candidate,
        commands::copilot_chat::stream_copilot_message,
        commands::copilot_chat::list_conversations,
        commands::copilot_chat::get_conversation_messages,
        commands::copilot_chat::delete_conversation,
        commands::copilot_chat::create_conversation,
        commands::copilot_chat::edit_conversation_user_message,
        commands::copilot_chat::delete_conversation_messages_after,
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
        .plugin(tauri_plugin_notification::init())
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
            // Best-effort: seed an OpenRouter/Gemma provider from `.env` when the
            // user has not configured one. Never fatal — a missing key just means
            // the user configures a provider via Settings. Errors are logged
            // without the key value.
            match bootstrap_env_provider(&db) {
                Ok(true) => tracing::info!("Seeded OpenRouter provider from .env (key hidden)"),
                Ok(false) => {}
                Err(e) => tracing::warn!("OpenRouter .env bootstrap skipped: {e}"),
            }

            // Best-effort: derive balances for existing imported accounts (so the
            // "$0 after import" state resolves without a re-import), record today's
            // net-worth snapshot, and recompute statistical anomaly flags so
            // existing imported data populates without waiting for a re-import.
            if let Ok(mut conn) = db.get() {
                if let Ok(ids) = conn
                    .prepare("SELECT id FROM accounts WHERE archived_at IS NULL")
                    .and_then(|mut s| {
                        s.query_map([], |r| r.get::<_, String>(0))
                            .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
                    })
                {
                    for id in ids {
                        let _ = finsight_core::repos::accounts::recompute_balance_if_linked(&mut conn, &id);
                    }
                }
                let _ = finsight_core::repos::net_worth::record_today(&mut conn);
                let _ = finsight_core::repos::net_worth::backfill_history_from_transactions(&mut conn);
                let _ = finsight_core::anomaly::recompute_anomalies(&mut conn);
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

            let _scheduler = app.state::<AppState>().sync_scheduler.start();

            let check_agent = app.state::<AppState>().agent.tx.clone();
            tauri::async_runtime::spawn(async move {
                let _ = check_agent
                    .send(finsight_agent::agent::AgentJob::CheckDueRecipes)
                    .await;
            });

            // Resume auto-categorization if a previous run was interrupted — e.g.
            // the app was imported into and then closed before the LLM pass
            // finished, which otherwise leaves those rows uncategorized forever.
            // Best-effort: gated on the setting and on there being real work.
            let cat_agent = app.state::<AppState>().agent.tx.clone();
            let cat_db = db.clone();
            tauri::async_runtime::spawn(async move {
                let should = finsight_core::repos::run(&cat_db, |conn| {
                    let auto: Option<bool> = finsight_core::settings::get(
                        conn,
                        crate::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY,
                    )?;
                    if !auto.unwrap_or(true) {
                        return Ok(false);
                    }
                    let pending: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND is_transfer = 0",
                        [],
                        |r| r.get(0),
                    )?;
                    Ok(pending > 0)
                })
                .await
                .unwrap_or(false);
                if should {
                    let _ = cat_agent
                        .send(finsight_agent::agent::AgentJob::CategorizeAll)
                        .await;
                }
            });

            let notify_app = app.handle().clone();
            let notify_db = db.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::notifications::check_and_fire(&notify_app, &notify_db).await;
            });

            Ok(())
        })
}

#[cfg(test)]
mod env_bootstrap_tests {
    use super::read_env_key_from_file;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn reads_key_from_dotenv_file_walking_up_from_a_subdir() {
        let root = TempDir::new().unwrap();
        fs::write(
            root.path().join(".env"),
            "# comment\nexport OPENROUTER_API_KEY=\"sk-or-file-value\"\nOTHER=1\n",
        )
        .unwrap();
        let sub = root.path().join("a").join("b");
        fs::create_dir_all(&sub).unwrap();

        // Walks up from the nested subdir to find the .env at the root.
        let got = read_env_key_from_file(&sub, "OPENROUTER_API_KEY");
        assert_eq!(got.as_deref(), Some("sk-or-file-value"));
    }

    #[test]
    fn returns_none_when_key_absent_or_empty() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join(".env"), "OPENROUTER_API_KEY=\nFOO=bar\n").unwrap();
        assert_eq!(read_env_key_from_file(root.path(), "OPENROUTER_API_KEY"), None);
        assert_eq!(read_env_key_from_file(root.path(), "MISSING"), None);
    }
}

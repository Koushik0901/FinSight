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
            // Trim defensively: a key stored with stray whitespace (e.g. a
            // paste with a trailing newline) must not corrupt the auth header.
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", &preset)
                .ok()??
                .trim()
                .to_string();
            if api_key.is_empty() {
                return None;
            }
            Some(Arc::new(OpenAiCompatProvider::new(
                base_url, api_key, model, preset,
            )))
        }
        "anthropic" => {
            let model = cfg["model"].as_str()?.to_string();
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", "anthropic")
                .ok()??
                .trim()
                .to_string();
            if api_key.is_empty() {
                return None;
            }
            Some(Arc::new(AnthropicProvider::new(api_key, model)))
        }
        _ => None,
    }
}

/// Optional fast "router" model for the Copilot tool loop. When the user sets a
/// `copilot.router_model` in settings AND the main provider is OpenAI-compatible
/// (OpenRouter etc.), build a cheap, small-budget router that drives the many
/// tool-selection turns while the configured (strong) model writes the final
/// answer. Returns None when unset or not applicable → single-model loop.
pub(crate) fn build_copilot_router_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    let conn = db.get().ok()?;
    let router_model: String = settings::get(&conn, "copilot.router_model").ok()??;
    let router_model = router_model.trim().to_string();
    if router_model.is_empty() {
        return None;
    }
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").ok()??;
    // Only OpenAI-compatible endpoints can serve a cheap sibling model on the
    // same base URL + key.
    if cfg.get("kind")?.as_str()? != "openai_compat" {
        return None;
    }
    let base_url = cfg["base_url"].as_str()?.to_string();
    let preset = cfg["preset"].as_str().unwrap_or("custom").to_string();
    let api_key = finsight_core::keychain::get_key("com.finsight.llm", &preset)
        .ok()??
        .trim()
        .to_string();
    if api_key.is_empty() {
        return None;
    }
    // Small completion budget: a routing turn only emits a short tool call.
    Some(Arc::new(
        OpenAiCompatProvider::new(base_url, api_key, router_model, preset).with_max_tokens(1024),
    ))
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
        commands::import::prepare_csv_import,
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
        commands::agent::set_anomaly_dismissed,
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
        commands::budget::contribute_to_goal,
        commands::budget::list_goal_contributions,
        commands::budget::archive_goal,
        commands::budget::project_goal_growth,
        commands::recurring::list_recurring,
        commands::reports::get_report_data,
        commands::reports::get_month_totals,
        commands::reports::get_savings_rate_history,
        commands::reports::create_monthly_review,
        commands::reports::list_monthly_reviews,
        commands::metrics::get_financial_metrics,
        commands::metrics::set_financial_assumptions,
        commands::scenarios::run_scenario,
        commands::scenarios::save_scenario,
        commands::scenarios::list_scenario_history,
        commands::scenarios::delete_scenario,
        commands::transactions::get_transaction_count,
        commands::assets::list_manual_assets,
        commands::assets::create_manual_asset,
        commands::assets::update_manual_asset,
        commands::assets::delete_manual_asset,
        commands::assets::record_net_worth_snapshot,
        commands::assets::list_net_worth_history,
        commands::assets::compute_debt_payoff,
        commands::assets::get_uncelebrated_milestones,
        commands::household::list_household_members,
        commands::household::create_household_member,
        commands::household::set_self_member,
        commands::household::delete_household_member,
        commands::household::list_account_owners,
        commands::household::set_account_owners,
        commands::household::set_account_owner_shares,
        commands::household::list_asset_owners,
        commands::household::set_asset_owners,
        commands::data_health::get_data_health,
        commands::data_health::create_manual_backup,
        commands::data_health::stage_restore_backup,
        commands::data_health::cancel_staged_restore,
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
        commands::transactions::export_search_transactions_csv,
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

            // Apply a staged restore (P0-4) BEFORE opening the DB, so we never
            // swap a database that has live connections. Move the pending file
            // over data.sqlcipher and drop the stale WAL/SHM so the restored
            // snapshot is authoritative.
            let pending_restore = app_data_dir.join("data.pending-restore.sqlcipher");
            if pending_restore.exists() {
                let _ = std::fs::remove_file(app_data_dir.join("data.sqlcipher-wal"));
                let _ = std::fs::remove_file(app_data_dir.join("data.sqlcipher-shm"));
                if let Err(e) = std::fs::rename(&pending_restore, &db_path) {
                    eprintln!("⚠ failed to apply staged restore: {e}");
                }
            }

            let key = finsight_core::keychain::load_or_create_key(SERVICE, USER)
                .map_err(|e| -> Box<dyn std::error::Error> {
                    format!("keychain error: {e}").into()
                })?;

            let db = Db::open(&db_path, &key).map_err(|e| -> Box<dyn std::error::Error> {
                format!("db open error: {e}").into()
            })?;

            // ── Durability guards (P0-4) ──────────────────────────────────────
            // 1. Verify the database is not corrupt. Record the result so the
            //    Settings → Data & backups panel can show it; a failure does NOT
            //    block startup (the user needs the app open to restore a backup).
            let backups_dir = app_data_dir.join("backups");
            let integrity = db.integrity_check().unwrap_or_else(|e| format!("check failed: {e}"));
            if integrity.trim() != "ok" {
                eprintln!("⚠ database integrity check: {integrity}");
            }
            // 2. Take a consistent encrypted backup BEFORE applying any pending
            //    migration, so a failed/again-corrupting migration is always
            //    recoverable. Only when migrations are actually pending (keeps
            //    the backup set meaningful and avoids a copy on every launch).
            let pending = db.pending_migration_count().unwrap_or(0);
            let mut startup_warnings: Vec<String> = Vec::new();
            let mut last_backup: Option<String> = None;
            if pending > 0 {
                match db.backup(&backups_dir, "pre-migration", 10) {
                    Ok(p) => last_backup = Some(p.to_string_lossy().to_string()),
                    Err(e) => startup_warnings.push(format!("pre-migration backup failed: {e}")),
                }
            }
            run_migrations(&db).map_err(|e| -> Box<dyn std::error::Error> {
                format!("migrations: {e}").into()
            })?;
            migrate_provider_settings(&db).map_err(|e| -> Box<dyn std::error::Error> {
                format!("provider migration: {e}").into()
            })?;
            if let Ok(conn) = db.get() {
                let _ = finsight_core::settings::set(&conn, "data.integrity_status", &integrity);
                let _ = finsight_core::settings::set(
                    &conn,
                    "data.integrity_checked_at",
                    &chrono::Utc::now().to_rfc3339(),
                );
                if let Some(p) = &last_backup {
                    let _ = finsight_core::settings::set(&conn, "data.last_backup_path", p);
                    let _ = finsight_core::settings::set(
                        &conn,
                        "data.last_backup_at",
                        &chrono::Utc::now().to_rfc3339(),
                    );
                }
            }
            // Best-effort: derive balances for existing imported accounts (so the
            // "$0 after import" state resolves without a re-import), record today's
            // net-worth snapshot, and recompute statistical anomaly flags so
            // existing imported data populates without waiting for a re-import.
            // Each cascade step is best-effort, but a FAILURE is recorded (not
            // silently swallowed) so the user can see that derived data may be
            // stale, instead of the old `let _ =` that hid real problems.
            if let Ok(mut conn) = db.get() {
                macro_rules! step {
                    ($label:expr, $e:expr) => {
                        if let Err(err) = $e {
                            startup_warnings.push(format!("{}: {err}", $label));
                        }
                    };
                }
                // Re-run the deterministic builtin pass so transfer flags reflect
                // the current keyword list (idempotent; fixes stale is_transfer),
                // then pair cross-account transfer legs so existing imports gain
                // pairing without a re-import.
                step!(
                    "startup categorization",
                    finsight_core::categorize::apply_builtin_categorization(&mut conn)
                );
                step!(
                    "startup transfer pairing",
                    finsight_core::categorize::pair_transfers(&mut conn)
                );
                if let Ok(ids) = conn
                    .prepare("SELECT id FROM accounts WHERE archived_at IS NULL")
                    .and_then(|mut s| {
                        s.query_map([], |r| r.get::<_, String>(0))
                            .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
                    })
                {
                    for id in ids {
                        step!(
                            "startup balance recompute",
                            finsight_core::repos::accounts::recompute_balance_if_linked(&mut conn, &id)
                        );
                    }
                }
                step!(
                    "startup net-worth snapshot",
                    finsight_core::repos::net_worth::record_today(&mut conn)
                );
                step!(
                    "startup net-worth backfill",
                    finsight_core::repos::net_worth::backfill_history_from_transactions(&mut conn)
                );
                step!(
                    "startup anomaly recompute",
                    finsight_core::anomaly::recompute_anomalies(&mut conn)
                );
                let _ = finsight_core::settings::set(
                    &conn,
                    "data.startup_warnings",
                    &startup_warnings,
                );
            }
            // Truncate the WAL now that the startup write burst is done, so it
            // doesn't linger at the size of the whole database between sessions.
            let _ = db.checkpoint();

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


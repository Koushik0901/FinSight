//! FinSight Tauri app — command surface + lifecycle.

pub mod commands;
pub mod error;
pub mod notifications;

/// Re-exported from finsight-api: the background + batch SimpleFin sync
/// scheduler now lives on `ApiState`, not `AppState`. Kept as `crate::sync_scheduler`
/// so existing call sites (e.g. `commands::simplefin`) don't churn.
pub use finsight_api::sync_scheduler;

/// Provider-construction helpers, moved to finsight-api (tauri-free already).
/// Re-exported here because integration tests and command modules import them
/// from `finsight_app`/`crate::` today.
pub use finsight_api::provider::{
    build_copilot_router_from_settings, build_provider_from_config, load_completion_provider_config,
    load_provider_from_settings, migrate_provider_settings,
};

use finsight_agent::agent::{AgentEvent, EventCallback};
use finsight_core::Db;
use std::sync::Arc;
use tauri::{Emitter, Manager};

pub struct AppState {
    /// Shared behind an `Arc` so the future finsight-server can hand the same
    /// `ApiState` to many concurrent request handlers; the single-window Tauri
    /// app doesn't strictly need the sharing, but keeps one construction path.
    pub api: Arc<finsight_api::ApiState>,
}

impl AppState {
    pub fn new(db: Db, data_dir: std::path::PathBuf, on_event: EventCallback) -> Self {
        let api = finsight_api::ApiState::new(db, data_dir, on_event);
        Self { api: Arc::new(api) }
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
        commands::categories::list_category_groups,
        commands::categories::create_category_group,
        commands::categories::set_category_group,
        commands::transactions::list_transactions,
        commands::transactions::create_transaction,
        commands::transactions::update_transaction,
        commands::transactions::delete_transaction,
        commands::transactions::create_rule,
        commands::transactions::set_transaction_owner,
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
        commands::investments::list_account_positions,
        commands::investments::get_investment_summary,
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
        commands::spending::get_spending_path_back,
        commands::spending::set_spending_annotation,
        commands::metrics::get_financial_metrics,
        commands::metrics::household_net_worth_breakdown,
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
        commands::transactions::set_transaction_transfer,
        commands::transactions::apply_transfer_verdict_to_similar,
        commands::transactions::set_counterparty_verdict,
        commands::transactions::apply_counterparty_verdict_to_similar,
        commands::transactions::list_unresolved_counterparties,
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
            // Dev/prod DB isolation (root-cause fix for recurring corruption): the
            // Tauri identifier is shared with the installed app, so a debug
            // `tauri dev` build would otherwise open/migrate/corrupt the REAL
            // production database. Debug builds use a sibling ".dev" data dir.
            let app_data_dir = resolve_app_data_dir(app.path().app_data_dir()?, cfg!(debug_assertions));
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

            // Refresh derived state (integrity check, pre-migration backup,
            // migrations, provider-settings migration, categorization/transfer
            // pairing/balances/net-worth/anomaly recompute). Shared with the
            // server's per-user login catch-up — see finsight_api::startup.
            let backups_dir = app_data_dir.join("backups");
            let _startup_report = finsight_api::startup::run_startup_cascade(&db, &backups_dir);

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

            let state = AppState::new(db.clone(), app_data_dir.clone(), on_event);
            // Load saved provider configuration and wire it into the agent
            if let Some(provider) = load_provider_from_settings(&db) {
                state.api.agent.set_provider(provider);
            }
            app.manage(state);

            // Pass Tauri's Tokio runtime handle explicitly: `.setup()` runs with no
            // ambient runtime entered, so `SyncScheduler::start` must spawn via a
            // `Handle` rather than the bare `tokio::spawn` (which would panic here).
            let rt = tauri::async_runtime::handle();
            let _scheduler = app.state::<AppState>().api.sync_scheduler.start(rt.inner());

            let check_agent = app.state::<AppState>().api.agent.tx.clone();
            tauri::async_runtime::spawn(async move {
                let _ = check_agent
                    .send(finsight_agent::agent::AgentJob::CheckDueRecipes)
                    .await;
            });

            // Resume auto-categorization if a previous run was interrupted — e.g.
            // the app was imported into and then closed before the LLM pass
            // finished, which otherwise leaves those rows uncategorized forever.
            // Best-effort: gated on the setting and on there being real work.
            let cat_agent = app.state::<AppState>().api.agent.tx.clone();
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

/// Choose the app data directory. Debug builds get a sibling `<identifier>.dev`
/// directory so development NEVER opens the installed app's production database
/// — the Tauri identifier is shared, so without this a `tauri dev` build would
/// migrate/corrupt the real DB (the root cause of the recurring corruption).
fn resolve_app_data_dir(prod: std::path::PathBuf, is_debug: bool) -> std::path::PathBuf {
    if !is_debug {
        return prod;
    }
    let dev_name = prod
        .file_name()
        .map(|n| format!("{}.dev", n.to_string_lossy()))
        .unwrap_or_else(|| "finsight.dev".to_string());
    prod.with_file_name(dev_name)
}

#[cfg(test)]
mod data_dir_tests {
    use super::resolve_app_data_dir;
    use std::path::PathBuf;

    #[test]
    fn release_uses_the_production_dir() {
        let prod = PathBuf::from(r"C:\Users\k\AppData\Roaming\com.finsight.app");
        assert_eq!(resolve_app_data_dir(prod.clone(), false), prod);
    }

    #[test]
    fn debug_uses_a_sibling_dev_dir_beside_prod() {
        let prod = PathBuf::from(r"C:\Users\k\AppData\Roaming\com.finsight.app");
        let dev = resolve_app_data_dir(prod.clone(), true);
        assert_eq!(
            dev.file_name().unwrap(),
            std::ffi::OsStr::new("com.finsight.app.dev")
        );
        // Same parent → fully separate from prod, never nested inside it.
        assert_eq!(dev.parent(), prod.parent());
        assert_ne!(dev, prod);
    }
}


//! FinSight TypeScript-bindings codegen crate.
//!
//! **This crate exists purely to generate `ui/src/api/bindings.ts`.** Its
//! ~199 `#[tauri::command]` wrappers are thin delegations to
//! `finsight_api::commands::*`; their only job is to carry the
//! `#[specta::specta]` type signatures that `build_specta_builder()` feeds to
//! the co-located `export_bindings` binary (`cargo run -p finsight-bindings
//! --bin export_bindings`, aka `pnpm bindings`).
//!
//! Nothing here is linked into the SHIPPED desktop binary — that's
//! `src-tauri/src/main.rs`, a thin webview shell with no local command surface
//! and no local database (see
//! docs/superpowers/plans/2026-07-17-server-phase4-thin-desktop-shell.md). The
//! shell's crate does NOT depend on this one; the pre-Phase-4 app-lifecycle
//! setup (`configure_app`, data-dir resolution, the single-instance/dialog/
//! opener plugins) was dead after the pivot and has been removed so the crate's
//! codegen-only purpose is clear at a glance.

pub mod commands;
pub mod error;
pub mod notifications;

/// Re-exported from finsight-api: the background + batch SimpleFin sync
/// scheduler now lives on `ApiState`, not `AppState`. Kept as `crate::sync_scheduler`
/// so existing call sites (e.g. `commands::simplefin`) don't churn.
pub use finsight_api::sync_scheduler;

/// Provider-construction helpers, moved to finsight-api (tauri-free already).
/// Re-exported here because integration tests and command modules import them
/// from `finsight_bindings`/`crate::` today.
pub use finsight_api::provider::{
    build_copilot_router_from_settings, build_provider_from_config, load_completion_provider_config,
    load_provider_from_settings, migrate_provider_settings,
};

use finsight_agent::agent::EventCallback;
use finsight_core::Db;
use std::sync::Arc;

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

/// Build the tauri-specta builder with all commands registered. Consumed by
/// the `export_bindings` binary; the registered command set is the contract the
/// generated `bindings.ts` — and the finsight-server dispatcher (see
/// `crates/finsight-server/tests/parity.rs`) — must stay in sync with.
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
        commands::budget::list_member_budget_envelopes,
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
        commands::restoration::list_restoration_envelopes,
        commands::restoration::get_restoration_status,
        commands::restoration::create_restoration_envelope,
        commands::restoration::close_restoration_envelope,
        commands::restoration::delete_restoration_envelope,
        commands::restoration::add_restoration_leg,
        commands::restoration::remove_restoration_leg,
        commands::metrics::get_financial_philosophy,
        commands::metrics::set_financial_philosophy,
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
        commands::budget::update_goal_priority,
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
        commands::accounts::get_account_balance_timeline,
        commands::accounts::list_account_balance_sparklines,
        commands::journey::get_journey_status,
        commands::inbox::get_action_items,
        commands::inbox::get_inbox_badge_count,
        commands::push::get_push_status,
        commands::push::save_push_subscription,
        commands::push::delete_push_subscription,
        commands::push::list_push_devices,
        commands::push::send_test_push,
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


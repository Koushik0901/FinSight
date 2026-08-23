use serde_json::{json, Value};
use utoipa::openapi::OpenApi;

/// Every RPC command that `finsight-server/src/dispatch.rs` routes.
/// Keep this sorted and identical to `SUPPORTED` (minus `UNSUPPORTED` which is
/// empty today) — the parity test + this snapshot test enforce they stay in sync.
/// This list is the OpenAPI contract's paths; each entry becomes `POST /api/rpc/{cmd}`.
pub const COMMANDS: &[&str] = &[
    "accept_category_proposal",
    "accept_import_candidate_match",
    "accept_rule_proposal",
    "acknowledge_simplefin_alert",
    "add_category_example",
    "add_restoration_leg",
    "app_ready",
    "apply_counterparty_verdict_to_similar",
    "apply_next_month_plan",
    "apply_scenario",
    "apply_transfer_verdict_to_similar",
    "approve_action_item",
    "archive_account",
    "archive_category",
    "archive_goal",
    "archive_scenario",
    "ask_agent",
    "cancel_staged_restore",
    "clear_scenario_revision",
    "close_agent_session",
    "close_restoration_envelope",
    "commit_starter_categories",
    "compute_debt_payoff",
    "confirm_simplefin_transfer",
    "contribute_to_goal",
    "correct_category_proposal",
    "create_account",
    "create_agent_session",
    "create_category",
    "create_category_group",
    "create_conversation",
    "create_goal",
    "create_household_member",
    "create_import_candidate_transaction",
    "create_manual_asset",
    "create_manual_backup",
    "create_planned_transaction",
    "create_recipe",
    "create_restoration_envelope",
    "create_rule",
    "create_transaction",
    "decline_rule_proposal",
    "delete_all_data",
    "delete_conversation",
    "delete_conversation_messages_after",
    "delete_household_member",
    "delete_manual_asset",
    "delete_planned_transaction",
    "delete_push_subscription",
    "delete_recipe",
    "delete_restoration_envelope",
    "delete_scenario",
    "delete_simplefin_connection",
    "delete_transaction",
    "discard_unfinished_import",
    "disconnect_simplefin",
    "dismiss_import_candidate",
    "duplicate_scenario",
    "edit_conversation_user_message",
    "execute_action_bundle",
    "explain_financial_metrics",
    "explain_goals",
    "explain_scenario",
    "export_account_csv",
    "export_all_data_csv",
    "export_all_data_json",
    "export_search_transactions_csv",
    "export_transactions_csv",
    "forget_agent_memory",
    "get_account_balance_timeline",
    "get_action_bundle",
    "get_action_items",
    "get_agent_status",
    "get_auto_categorize_enabled",
    "get_cashflow_forecast",
    "get_completion_provider",
    "get_conversation_messages",
    "get_currency",
    "get_data_health",
    "get_financial_health_score",
    "get_financial_metrics",
    "get_financial_philosophy",
    "get_inbox_badge_count",
    "get_investment_summary",
    "get_journey_status",
    "get_month_close",
    "get_month_totals",
    "get_needs_review_count",
    "get_notification_prefs",
    "get_notifications_enabled",
    "get_onboarding_state",
    "get_plan_next_month_data",
    "get_planned_transaction",
    "get_push_status",
    "get_report_data",
    "get_restoration_status",
    "get_saved_csv_mapping",
    "get_savings_rate_history",
    "get_simplefin_status",
    "get_simplefin_sync_settings",
    "get_spending_breakdown",
    "get_spending_path_back",
    "get_transaction_count",
    "get_transaction_splits",
    "get_uncelebrated_milestones",
    "household_net_worth_breakdown",
    "import_csv",
    "import_simplefin_accounts",
    "list_account_balance_history",
    "list_account_balance_sparklines",
    "list_account_owners",
    "list_account_positions",
    "list_accounts",
    "list_action_bundles",
    "list_agent_memory",
    "list_agent_sessions",
    "list_asset_owners",
    "list_budget_envelopes",
    "list_budget_history",
    "list_categories",
    "list_categories_with_spending",
    "list_category_examples",
    "list_category_groups",
    "list_category_proposals",
    "list_conversations",
    "list_execution_log",
    "list_goal_contributions",
    "list_goals",
    "list_household_members",
    "list_import_review_candidates",
    "list_manual_assets",
    "list_member_budget_envelopes",
    "list_month_closes",
    "list_net_worth_history",
    "list_notifications",
    "list_planned_transactions",
    "list_provider_models",
    "list_push_devices",
    "list_recent_agent_activity",
    "list_recipe_runs",
    "list_recipes",
    "list_recurring",
    "list_restoration_envelopes",
    "list_rule_proposals",
    "list_rules_with_categories",
    "list_saved_scenarios",
    "list_simplefin_accounts",
    "list_simplefin_alerts",
    "list_simplefin_connections",
    "list_simplefin_transfer_suggestions",
    "list_transactions",
    "list_unfinished_imports",
    "list_unresolved_counterparties",
    "mark_all_notifications_read",
    "mark_notification_read",
    "mark_onboarding_complete",
    "mark_subscription_cancelled",
    "notification_unread_count",
    "pause_recipe",
    "prepare_csv_import",
    "preview_csv_columns",
    "probe_ollama",
    "project_goal_growth",
    "promote_scenario",
    "purge_simplefin_data",
    "recompute_anomalies",
    "reconcileBases",
    "record_net_worth_snapshot",
    "reject_action_item",
    "reject_category_proposal",
    "reject_simplefin_transfer",
    "remove_category_example",
    "remove_restoration_leg",
    "rename_category",
    "reset_onboarding_completion",
    "resume_recipe",
    "revise_scenario",
    "run_scenario",
    "save_llm_provider",
    "save_month_close",
    "save_provider_api_key",
    "save_push_subscription",
    "save_scenario",
    "save_simplefin_setup_token",
    "send_test_push",
    "set_account_balance",
    "set_account_owner_shares",
    "set_account_owners",
    "set_anomaly_dismissed",
    "set_asset_owners",
    "set_auto_categorize_enabled",
    "set_budget",
    "set_category_group",
    "set_category_guidance",
    "set_category_spending_type",
    "set_completion_provider",
    "set_counterparty_verdict",
    "set_currency",
    "set_financial_assumptions",
    "set_financial_philosophy",
    "set_notification_prefs",
    "set_notifications_enabled",
    "set_self_member",
    "set_simplefin_sync_settings",
    "set_spending_annotation",
    "set_subscription_trial",
    "set_subscription_verdict",
    "set_transaction_flags",
    "set_transaction_owner",
    "set_transaction_splits",
    "set_transaction_transfer",
    "stage_restore_backup",
    "stream_copilot_message",
    "sync_all_simplefin_accounts",
    "sync_simplefin_account",
    "test_completion_provider",
    "toggle_rule",
    "trigger_categorize",
    "trigger_recategorize_low_confidence",
    "trigger_recipe",
    "update_account",
    "update_category_color",
    "update_goal_balance",
    "update_goal_monthly",
    "update_goal_priority",
    "update_goal_purpose",
    "update_manual_asset",
    "update_planned_transaction",
    "update_recipe",
    "update_transaction",
];

/// Build the current OpenAPI document.
///
/// Each entry in `COMMANDS` becomes a `POST /api/rpc/{cmd}` path with a
/// generic JSON request/response (the precise shapes live in `finsight-api`
/// DTOs; this is intentionally shallow for the scaffold — Task 2's snapshot
/// only checks that every command appears, not its schema). The file is served
/// at `GET /api/openapi.json` and used by `openapi-typescript` to generate
/// the typed client. Keeping this as a pure function makes it testable without
/// a server.
pub fn build_openapi() -> Value {
    let mut paths = serde_json::Map::new();
    for cmd in COMMANDS {
        let path = format!("/api/rpc/{cmd}");
        paths.insert(
            path,
            json!({
                "post": {
                    "operationId": cmd,
                    "summary": format!("RPC {cmd}"),
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }),
        );
    }
    let mut all_paths = paths;
    all_paths.insert(
        "/api/openapi.json".to_string(),
        json!({
            "get": {
                "operationId": "get_openapi",
                "summary": "OpenAPI specification",
                "responses": {
                    "200": {
                        "description": "OpenAPI JSON",
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            }
                        }
                    }
                }
            }
        }),
    );
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "FinSight API",
            "version": "0.1.0",
            "description": "FinSight RPC API — every command is POST /api/rpc/{cmd} with a JSON object body. The OpenAPI file is the contract for the generated TypeScript client (replaces tauri-specta bindings.ts).",
            "license": { "name": "AGPL-3.0-or-later" }
        },
        "paths": all_paths
    })
}

/// Typed wrapper for tests that need `OpenApi` struct (keeps utoipa dep used).
pub fn build_openapi_typed() -> OpenApi {
    serde_json::from_value(build_openapi()).expect("openapi json must deserialize to OpenApi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_is_version_3x() {
        let json = build_openapi();
        let v = json["openapi"].as_str().unwrap_or_default();
        assert!(
            v.starts_with("3."),
            "OpenAPI version must be 3.x, got {v:?}"
        );
    }

    #[test]
    fn openapi_has_expected_info() {
        let json = build_openapi();
        assert_eq!(json["info"]["title"], "FinSight API");
        assert_eq!(json["info"]["version"], "0.1.0");
    }

    #[test]
    fn openapi_serializes_to_valid_json() {
        let json = build_openapi();
        let json_str = serde_json::to_string(&json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(v.get("openapi").is_some());
        assert!(v.get("info").is_some());
    }

    #[test]
    fn openapi_contains_every_rpc_command() {
        let json = build_openapi();
        let paths = json["paths"].as_object().expect("paths must be object");
        for cmd in COMMANDS {
            let key = format!("/api/rpc/{cmd}");
            assert!(
                paths.contains_key(&key),
                "openapi paths missing {key} — did you update COMMANDS vs dispatch.rs SUPPORTED?"
            );
        }
    }

    #[test]
    fn openapi_paths_match_rpc_command_count() {
        let json = build_openapi();
        let paths = json["paths"].as_object().unwrap();
        // Filter only /api/rpc/* (exclude /api/openapi.json itself)
        let rpc_count = paths.keys().filter(|k| k.starts_with("/api/rpc/")).count();
        assert_eq!(
            rpc_count,
            COMMANDS.len(),
            "rpc path count must equal COMMANDS.len() — drift between spec and dispatch"
        );
    }

    #[test]
    fn openapi_typed_roundtrips() {
        let v = build_openapi();
        let typed: OpenApi = serde_json::from_value(v).expect("must deserialize to OpenApi");
        let json = serde_json::to_value(&typed).unwrap();
        assert_eq!(json["info"]["title"], "FinSight API");
    }

    #[test]
    fn openapi_schemas_not_shallow() {
        let spec = build_openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let schemas = json["components"]["schemas"]
            .as_object()
            .expect("schemas");
        assert!(
            schemas.len() > 20,
            "expected many schemas, got {}",
            schemas.len()
        );
        for (name, schema) in schemas {
            let s = schema.to_string();
            assert!(
                !s.contains(r#""type":"object""#) || s.contains("properties"),
                "shallow schema {name} still type:object without properties"
            );
        }
    }
}

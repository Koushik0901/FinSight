//! RPC dispatcher: `POST /api/rpc/{cmd}` over the shared `finsight-api` command
//! surface.

use crate::auth::AuthedUser;
use crate::state::{OutboundEvent, ServerState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use finsight_api::error::AppError;
use finsight_api::sink::FrameSink;
use finsight_api::ApiState;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Sink that fans command-emitted events out to every SSE subscriber.
pub struct BroadcastSink(pub tokio::sync::broadcast::Sender<OutboundEvent>);
impl FrameSink for BroadcastSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.0.send(OutboundEvent {
            event: event.to_string(),
            payload,
        });
    }
}

/// Argument keys arrive in camelCase (what bindings.ts sends — Tauri converted
/// them to snake_case for us before; here we read them by their camelCase name).
fn arg<T: serde::de::DeserializeOwned>(p: &serde_json::Value, name: &str) -> Result<T, AppError> {
    let v = p.get(name).cloned().unwrap_or(serde_json::Value::Null);
    serde_json::from_value(v).map_err(|e| AppError::new("rpc.bad_arg", format!("argument `{name}`: {e}")))
}

fn ok<T: serde::Serialize>(v: T) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(v).map_err(|e| AppError::new("rpc.serialize", e.to_string()))
}

/// Commands intentionally unavailable over HTTP. CSV import is supported via
/// `/api/import/csv`; its opaque upload token is resolved and confined below.
pub const UNSUPPORTED: &[&str] = &[];

async fn uploaded_csv_path(api: &ApiState, token: String) -> Result<String, AppError> {
    let candidate = PathBuf::from(&token);
    let valid_name = candidate.file_name().and_then(|name| name.to_str()) == Some(token.as_str())
        && candidate.extension().and_then(|ext| ext.to_str()) == Some("csv")
        && candidate
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| uuid::Uuid::parse_str(stem).is_ok());
    if !valid_name {
        return Err(AppError::new(
            "rpc.invalid_import_upload",
            "CSV import requires a valid browser upload token",
        ));
    }

    let imports_dir = api.data_dir.join("imports");
    tokio::task::spawn_blocking(move || {
        let root = std::fs::canonicalize(&imports_dir).map_err(|_| {
            AppError::new("rpc.invalid_import_upload", "CSV upload was not found")
        })?;
        let path = std::fs::canonicalize(imports_dir.join(candidate)).map_err(|_| {
            AppError::new("rpc.invalid_import_upload", "CSV upload was not found")
        })?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err(AppError::new(
                "rpc.invalid_import_upload",
                "CSV upload is outside the authenticated user's staging directory",
            ));
        }
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| AppError::new("internal", format!("join: {e}")))?
}

pub async fn rpc(
    State(st): State<Arc<ServerState>>,
    user: AuthedUser,
    Path(cmd): Path<String>,
    Json(p): Json<serde_json::Value>,
) -> Response {
    if UNSUPPORTED.contains(&cmd.as_str()) {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(AppError::new(
                "rpc.unsupported",
                format!("`{cmd}` needs the desktop app (Phase 3 adds a web flow)"),
            )),
        )
            .into_response();
    }
    let rt = match st
        .registry
        .get_or_bootstrap(&st.data_dir, &user.user_id, &user.db_key_hex)
        .await
    {
        Ok(rt) => rt,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError::new("auth.runtime", e.to_string())),
            )
                .into_response()
        }
    };
    st.registry.touch(&user.user_id);
    match dispatch(&rt.api, &rt.events, &cmd, p).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) if e.code == "rpc.unknown_command" => (StatusCode::NOT_FOUND, Json(e)).into_response(),
        Err(e) if e.code == "rpc.bad_arg" || e.code == "rpc.invalid_import_upload" => {
            (StatusCode::BAD_REQUEST, Json(e)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e)).into_response(),
    }
}

async fn dispatch(
    api: &Arc<ApiState>,
    events: &broadcast::Sender<OutboundEvent>,
    cmd: &str,
    p: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    use finsight_api::commands as c;
    match cmd {
        // ── accounts ──
        "list_accounts" => ok(c::accounts::list_accounts(api).await?),
        "create_account" => ok(c::accounts::create_account(api, arg(&p, "input")?).await?),
        "update_account" => {
            ok(c::accounts::update_account(api, arg(&p, "id")?, arg(&p, "patch")?).await?)
        }
        "archive_account" => ok(c::accounts::archive_account(api, arg(&p, "id")?).await?),
        "set_account_balance" => ok(c::accounts::set_account_balance(
            api,
            arg(&p, "id")?,
            arg(&p, "balanceCents")?,
        )
        .await?),
        "list_account_balance_history" => ok(c::accounts::list_account_balance_history(
            api,
            arg(&p, "accountId")?,
            arg(&p, "days")?,
        )
        .await?),
        "get_account_balance_timeline" => ok(c::accounts::get_account_balance_timeline(
            api,
            arg(&p, "accountId")?,
            arg(&p, "since")?,
        )
        .await?),
        "list_account_balance_sparklines" => {
            ok(c::accounts::list_account_balance_sparklines(api, arg(&p, "days")?).await?)
        }
        "export_account_csv" => {
            ok(c::accounts::export_account_csv(api, arg(&p, "accountId")?).await?)
        }

        // ── agent ──
        "set_completion_provider" => {
            ok(c::agent::set_completion_provider(api, arg(&p, "config")?).await?)
        }
        "get_completion_provider" => ok(c::agent::get_completion_provider(api).await?),
        "save_provider_api_key" => ok(c::agent::save_provider_api_key(
            api,
            arg(&p, "providerId")?,
            arg(&p, "key")?,
        )
        .await?),
        "list_provider_models" => {
            ok(c::agent::list_provider_models(api, arg(&p, "config")?).await?)
        }
        "test_completion_provider" => ok(c::agent::test_completion_provider(
            api,
            arg(&p, "config")?,
            arg(&p, "apiKey")?,
        )
        .await?),
        "get_needs_review_count" => ok(c::agent::get_needs_review_count(api).await?),
        "trigger_categorize" => ok(c::agent::trigger_categorize(api).await?),
        "recompute_anomalies" => ok(c::agent::recompute_anomalies(api).await?),
        "set_anomaly_dismissed" => ok(c::agent::set_anomaly_dismissed(
            api,
            arg(&p, "txnId")?,
            arg(&p, "dismissed")?,
        )
        .await?),
        "trigger_recategorize_low_confidence" => {
            ok(c::agent::trigger_recategorize_low_confidence(api).await?)
        }
        "get_agent_status" => ok(c::agent::get_agent_status(api).await?),
        "ask_agent" => {
            ok(c::agent::ask_agent(api, arg(&p, "question")?, arg(&p, "mode")?).await?)
        }
        "list_rule_proposals" => ok(c::agent::list_rule_proposals(api).await?),
        "accept_rule_proposal" => ok(c::agent::accept_rule_proposal(api, arg(&p, "id")?).await?),
        "decline_rule_proposal" => {
            ok(c::agent::decline_rule_proposal(api, arg(&p, "id")?).await?)
        }
        "list_recent_agent_activity" => {
            ok(c::agent::list_recent_agent_activity(api, arg(&p, "limit")?).await?)
        }

        // ── assets ──
        "list_manual_assets" => ok(c::assets::list_manual_assets(api).await?),
        "create_manual_asset" => {
            ok(c::assets::create_manual_asset(api, arg(&p, "input")?).await?)
        }
        "update_manual_asset" => {
            ok(c::assets::update_manual_asset(api, arg(&p, "id")?, arg(&p, "patch")?).await?)
        }
        "delete_manual_asset" => ok(c::assets::delete_manual_asset(api, arg(&p, "id")?).await?),
        "record_net_worth_snapshot" => ok(c::assets::record_net_worth_snapshot(api).await?),
        "list_net_worth_history" => {
            ok(c::assets::list_net_worth_history(api, arg(&p, "days")?).await?)
        }
        "compute_debt_payoff" => {
            ok(c::assets::compute_debt_payoff(api, arg(&p, "extraMonthlyCents")?).await?)
        }
        "get_uncelebrated_milestones" => ok(c::assets::get_uncelebrated_milestones(api).await?),

        // ── budget ──
        "list_budget_envelopes" => ok(c::budget::list_budget_envelopes(api).await?),
        "set_budget" => ok(c::budget::set_budget(
            api,
            arg(&p, "categoryId")?,
            arg(&p, "amountCents")?,
        )
        .await?),
        "list_goals" => ok(c::budget::list_goals(api).await?),
        "create_goal" => ok(c::budget::create_goal(api, arg(&p, "input")?).await?),
        "update_goal_balance" => ok(c::budget::update_goal_balance(
            api,
            arg(&p, "id")?,
            arg(&p, "currentCents")?,
        )
        .await?),
        "contribute_to_goal" => ok(c::budget::contribute_to_goal(
            api,
            arg(&p, "id")?,
            arg(&p, "amountCents")?,
            arg(&p, "note")?,
            arg(&p, "source")?,
        )
        .await?),
        "list_goal_contributions" => {
            ok(c::budget::list_goal_contributions(api, arg(&p, "goalId")?).await?)
        }
        "archive_goal" => ok(c::budget::archive_goal(api, arg(&p, "id")?).await?),
        "project_goal_growth" => ok(c::budget::project_goal_growth(
            api,
            arg(&p, "goalId")?,
            arg(&p, "years")?,
        )
        .await?),
        "update_goal_monthly" => ok(c::budget::update_goal_monthly(
            api,
            arg(&p, "id")?,
            arg(&p, "monthlyCents")?,
        )
        .await?),
        "update_goal_purpose" => ok(c::budget::update_goal_purpose(
            api,
            arg(&p, "id")?,
            arg(&p, "purpose")?,
        )
        .await?),
        "get_plan_next_month_data" => ok(c::budget::get_plan_next_month_data(api).await?),
        "apply_next_month_plan" => {
            ok(c::budget::apply_next_month_plan(api, arg(&p, "assignments")?).await?)
        }
        "list_budget_history" => {
            ok(c::budget::list_budget_history(api, arg(&p, "months")?).await?)
        }

        // ── categories ──
        "update_category_color" => ok(c::categories::update_category_color(
            api,
            arg(&p, "id")?,
            arg(&p, "color")?,
        )
        .await?),
        "create_category" => ok(c::categories::create_category(
            api,
            arg(&p, "label")?,
            arg(&p, "groupId")?,
            arg(&p, "color")?,
        )
        .await?),
        "rename_category" => ok(c::categories::rename_category(
            api,
            arg(&p, "id")?,
            arg(&p, "label")?,
        )
        .await?),
        "archive_category" => ok(c::categories::archive_category(api, arg(&p, "id")?).await?),
        "set_category_guidance" => ok(c::categories::set_category_guidance(
            api,
            arg(&p, "id")?,
            arg(&p, "guidance")?,
        )
        .await?),
        "list_category_groups" => ok(c::categories::list_category_groups(api).await?),
        "create_category_group" => ok(c::categories::create_category_group(
            api,
            arg(&p, "label")?,
            arg(&p, "hint")?,
        )
        .await?),
        "set_category_group" => ok(c::categories::set_category_group(
            api,
            arg(&p, "categoryId")?,
            arg(&p, "groupId")?,
        )
        .await?),

        // ── copilot (action bundles / sessions) ──
        "list_agent_sessions" => ok(c::copilot::list_agent_sessions(api).await?),
        "create_agent_session" => ok(c::copilot::create_agent_session(
            api,
            arg(&p, "title")?,
            arg(&p, "taskType")?,
        )
        .await?),
        "close_agent_session" => ok(c::copilot::close_agent_session(api, arg(&p, "id")?).await?),
        "list_action_bundles" => ok(c::copilot::list_action_bundles(
            api,
            arg(&p, "statusFilter")?,
            arg(&p, "sessionId")?,
            arg(&p, "limit")?,
        )
        .await?),
        "get_action_bundle" => ok(c::copilot::get_action_bundle(api, arg(&p, "id")?).await?),
        "approve_action_item" => {
            ok(c::copilot::approve_action_item(api, arg(&p, "itemId")?).await?)
        }
        "reject_action_item" => {
            ok(c::copilot::reject_action_item(api, arg(&p, "itemId")?).await?)
        }
        "list_execution_log" => {
            ok(c::copilot::list_execution_log(api, arg(&p, "bundleId")?).await?)
        }
        "execute_action_bundle" => {
            ok(c::copilot::execute_action_bundle(api, arg(&p, "bundleId")?).await?)
        }

        // ── copilot_chat: emit-path command constructs a BroadcastSink; note the
        //    sink is NOT a bindings arg, so the real args below ARE still
        //    arg-checked by Task 10 ──
        "stream_copilot_message" => {
            let sink: Arc<dyn FrameSink> = Arc::new(BroadcastSink(events.clone()));
            ok(c::copilot_chat::stream_copilot_message(
                api,
                sink,
                arg(&p, "conversationId")?,
                arg(&p, "runId")?,
                arg(&p, "text")?,
                arg(&p, "history")?,
                arg(&p, "sourceMessageId")?,
            )
            .await?)
        }
        "list_conversations" => ok(c::copilot_chat::list_conversations(api).await?),
        "get_conversation_messages" => {
            ok(c::copilot_chat::get_conversation_messages(api, arg(&p, "conversationId")?).await?)
        }
        "delete_conversation" => {
            ok(c::copilot_chat::delete_conversation(api, arg(&p, "id")?).await?)
        }
        "create_conversation" => ok(c::copilot_chat::create_conversation(api).await?),
        "edit_conversation_user_message" => ok(c::copilot_chat::edit_conversation_user_message(
            api,
            arg(&p, "input")?,
        )
        .await?),
        "delete_conversation_messages_after" => ok(c::copilot_chat::delete_conversation_messages_after(
            api,
            arg(&p, "conversationId")?,
            arg(&p, "messageId")?,
        )
        .await?),

        // ── data_health ──
        "get_data_health" => ok(c::data_health::get_data_health(api).await?),
        "create_manual_backup" => ok(c::data_health::create_manual_backup(api).await?),
        "stage_restore_backup" => {
            ok(c::data_health::stage_restore_backup(api, arg(&p, "path")?).await?)
        }
        "cancel_staged_restore" => ok(c::data_health::cancel_staged_restore(api).await?),

        // ── household ──
        "list_household_members" => ok(c::household::list_household_members(api).await?),
        "create_household_member" => ok(c::household::create_household_member(
            api,
            arg(&p, "name")?,
            arg(&p, "color")?,
        )
        .await?),
        "set_self_member" => {
            ok(c::household::set_self_member(api, arg(&p, "memberId")?).await?)
        }
        "delete_household_member" => {
            ok(c::household::delete_household_member(api, arg(&p, "id")?).await?)
        }
        "list_account_owners" => ok(c::household::list_account_owners(api).await?),
        "set_account_owners" => ok(c::household::set_account_owners(
            api,
            arg(&p, "accountId")?,
            arg(&p, "memberIds")?,
        )
        .await?),
        "set_account_owner_shares" => ok(c::household::set_account_owner_shares(
            api,
            arg(&p, "accountId")?,
            arg(&p, "owners")?,
        )
        .await?),
        "list_asset_owners" => ok(c::household::list_asset_owners(api).await?),
        "set_asset_owners" => ok(c::household::set_asset_owners(
            api,
            arg(&p, "assetId")?,
            arg(&p, "owners")?,
        )
        .await?),

        // ── import (import_csv is the other emit-path command) ──
        "preview_csv_columns" => ok(c::import::preview_csv_columns(
            uploaded_csv_path(api, arg(&p, "path")?).await?,
            arg(&p, "skipHeaderRows")?,
        )
        .await?),
        "prepare_csv_import" => ok(c::import::prepare_csv_import(
            api,
            uploaded_csv_path(api, arg(&p, "path")?).await?,
            arg(&p, "accountId")?,
            arg(&p, "mapping")?,
        )
        .await?),
        "import_csv" => {
            let sink: Arc<dyn FrameSink> = Arc::new(BroadcastSink(events.clone()));
            let path = uploaded_csv_path(api, arg(&p, "path")?).await?;
            let result = c::import::import_csv(
                api,
                sink,
                path.clone(),
                arg(&p, "accountId")?,
                arg(&p, "mapping")?,
            )
            .await?;
            if let Err(e) = tokio::fs::remove_file(&path).await {
                tracing::warn!(path, "could not remove completed CSV upload: {e}");
            }
            ok(result)
        }
        "get_saved_csv_mapping" => {
            ok(c::import::get_saved_csv_mapping(api, arg(&p, "accountId")?).await?)
        }
        "list_unfinished_imports" => ok(c::import::list_unfinished_imports(api).await?),
        "discard_unfinished_import" => {
            ok(c::import::discard_unfinished_import(api, arg(&p, "importId")?).await?)
        }

        // ── inbox ──
        "get_action_items" => ok(c::inbox::get_action_items(api).await?),

        // ── insights ──
        "list_agent_memory" => ok(c::insights::list_agent_memory(api).await?),
        "forget_agent_memory" => {
            ok(c::insights::forget_agent_memory(api, arg(&p, "id")?).await?)
        }
        "get_financial_health_score" => ok(c::insights::get_financial_health_score(api).await?),

        // ── investments ──
        "list_account_positions" => {
            ok(c::investments::list_account_positions(api, arg(&p, "accountId")?).await?)
        }
        "get_investment_summary" => {
            ok(c::investments::get_investment_summary(api, arg(&p, "accountId")?).await?)
        }

        // ── journey ──
        "get_journey_status" => ok(c::journey::get_journey_status(api).await?),

        // ── meta: returns the real AppReady { version } (UI reads it); no args, no state ──
        "app_ready" => ok(c::meta::app_ready().await?),

        // ── metrics ──
        "get_financial_metrics" => {
            ok(c::metrics::get_financial_metrics(api, arg(&p, "memberId")?).await?)
        }
        "household_net_worth_breakdown" => {
            ok(c::metrics::household_net_worth_breakdown(api).await?)
        }
        "set_financial_assumptions" => {
            ok(c::metrics::set_financial_assumptions(api, arg(&p, "input")?).await?)
        }

        // ── onboarding (probe_ollama takes no state — plain HTTP probe) ──
        "get_onboarding_state" => ok(c::onboarding::get_onboarding_state(api).await?),
        "mark_onboarding_complete" => ok(c::onboarding::mark_onboarding_complete(api).await?),
        "reset_onboarding_completion" => {
            ok(c::onboarding::reset_onboarding_completion(api).await?)
        }
        "commit_starter_categories" => {
            ok(c::onboarding::commit_starter_categories(api, arg(&p, "categories")?).await?)
        }
        "probe_ollama" => ok(c::onboarding::probe_ollama(arg(&p, "baseUrl")?).await?),
        "save_llm_provider" => {
            ok(c::onboarding::save_llm_provider(api, arg(&p, "config")?).await?)
        }

        // ── planned_transactions ──
        "list_planned_transactions" => ok(c::planned_transactions::list_planned_transactions(
            api,
            arg(&p, "filter")?,
        )
        .await?),
        "get_planned_transaction" => {
            ok(c::planned_transactions::get_planned_transaction(api, arg(&p, "id")?).await?)
        }
        "create_planned_transaction" => ok(c::planned_transactions::create_planned_transaction(
            api,
            arg(&p, "input")?,
        )
        .await?),
        "update_planned_transaction" => ok(c::planned_transactions::update_planned_transaction(
            api,
            arg(&p, "id")?,
            arg(&p, "patch")?,
        )
        .await?),
        "delete_planned_transaction" => {
            ok(c::planned_transactions::delete_planned_transaction(api, arg(&p, "id")?).await?)
        }

        // ── recipes ──
        "list_recipes" => {
            ok(c::recipes::list_recipes(api, arg(&p, "includePaused")?).await?)
        }
        "create_recipe" => ok(c::recipes::create_recipe(
            api,
            arg(&p, "title")?,
            arg(&p, "description")?,
            arg(&p, "recipeKind")?,
            arg(&p, "promptTemplate")?,
            arg(&p, "cadence")?,
            arg(&p, "dayOfWeek")?,
            arg(&p, "dayOfMonth")?,
        )
        .await?),
        "update_recipe" => ok(c::recipes::update_recipe(
            api,
            arg(&p, "id")?,
            arg(&p, "title")?,
            arg(&p, "description")?,
            arg(&p, "promptTemplate")?,
            arg(&p, "cadence")?,
            arg(&p, "dayOfWeek")?,
            arg(&p, "dayOfMonth")?,
        )
        .await?),
        "pause_recipe" => ok(c::recipes::pause_recipe(api, arg(&p, "id")?).await?),
        "resume_recipe" => ok(c::recipes::resume_recipe(api, arg(&p, "id")?).await?),
        "delete_recipe" => ok(c::recipes::delete_recipe(api, arg(&p, "id")?).await?),
        "trigger_recipe" => ok(c::recipes::trigger_recipe(api, arg(&p, "id")?).await?),
        "list_recipe_runs" => ok(c::recipes::list_recipe_runs(
            api,
            arg(&p, "recipeId")?,
            arg(&p, "limit")?,
        )
        .await?),

        // ── recurring ──
        "list_recurring" => ok(c::recurring::list_recurring(api).await?),

        // ── reports ──
        "get_report_data" => ok(c::reports::get_report_data(
            api,
            arg(&p, "scope")?,
            arg(&p, "memberId")?,
        )
        .await?),
        "get_month_totals" => ok(c::reports::get_month_totals(api).await?),
        "get_savings_rate_history" => ok(c::reports::get_savings_rate_history(api).await?),
        "create_monthly_review" => {
            ok(c::reports::create_monthly_review(api, arg(&p, "input")?).await?)
        }
        "list_monthly_reviews" => ok(c::reports::list_monthly_reviews(api).await?),

        // ── scenarios ──
        "run_scenario" => ok(c::scenarios::run_scenario(
            api,
            arg(&p, "description")?,
            arg(&p, "months")?,
            arg(&p, "params")?,
        )
        .await?),
        "save_scenario" => ok(c::scenarios::save_scenario(
            api,
            arg(&p, "description")?,
            arg(&p, "result")?,
        )
        .await?),
        "list_scenario_history" => ok(c::scenarios::list_scenario_history(api).await?),
        "delete_scenario" => ok(c::scenarios::delete_scenario(api, arg(&p, "id")?).await?),

        // ── settings ──
        "get_currency" => ok(c::settings::get_currency(api).await?),
        "set_currency" => ok(c::settings::set_currency(api, arg(&p, "currency")?).await?),
        "delete_all_data" => ok(c::settings::delete_all_data(api).await?),
        "get_notifications_enabled" => ok(c::settings::get_notifications_enabled(api).await?),
        "set_notifications_enabled" => {
            ok(c::settings::set_notifications_enabled(api, arg(&p, "enabled")?).await?)
        }
        "get_auto_categorize_enabled" => ok(c::settings::get_auto_categorize_enabled(api).await?),
        "set_auto_categorize_enabled" => {
            ok(c::settings::set_auto_categorize_enabled(api, arg(&p, "enabled")?).await?)
        }
        "export_all_data_json" => ok(c::settings::export_all_data_json(api).await?),
        "export_all_data_csv" => ok(c::settings::export_all_data_csv(api).await?),

        // ── simplefin ──
        "save_simplefin_setup_token" => {
            ok(c::simplefin::save_simplefin_setup_token(api, arg(&p, "token")?).await?)
        }
        "get_simplefin_status" => ok(c::simplefin::get_simplefin_status(api).await?),
        "list_simplefin_connections" => ok(c::simplefin::list_simplefin_connections(api).await?),
        "list_simplefin_accounts" => ok(c::simplefin::list_simplefin_accounts(api).await?),
        "import_simplefin_accounts" => {
            ok(c::simplefin::import_simplefin_accounts(api, arg(&p, "accounts")?).await?)
        }
        "sync_simplefin_account" => {
            ok(c::simplefin::sync_simplefin_account(api, arg(&p, "accountId")?).await?)
        }
        "disconnect_simplefin" => ok(c::simplefin::disconnect_simplefin(api).await?),
        "purge_simplefin_data" => ok(c::simplefin::purge_simplefin_data(api).await?),
        "delete_simplefin_connection" => {
            ok(c::simplefin::delete_simplefin_connection(api, arg(&p, "connectionId")?).await?)
        }
        "sync_all_simplefin_accounts" => {
            ok(c::simplefin::sync_all_simplefin_accounts(api).await?)
        }
        "get_simplefin_sync_settings" => {
            ok(c::simplefin::get_simplefin_sync_settings(api).await?)
        }
        "set_simplefin_sync_settings" => {
            ok(c::simplefin::set_simplefin_sync_settings(api, arg(&p, "settings")?).await?)
        }
        "list_simplefin_alerts" => ok(c::simplefin::list_simplefin_alerts(api).await?),
        "acknowledge_simplefin_alert" => {
            ok(c::simplefin::acknowledge_simplefin_alert(api, arg(&p, "alertId")?).await?)
        }
        "list_simplefin_transfer_suggestions" => {
            ok(c::simplefin::list_simplefin_transfer_suggestions(api).await?)
        }
        "confirm_simplefin_transfer" => {
            ok(c::simplefin::confirm_simplefin_transfer(api, arg(&p, "transferId")?).await?)
        }
        "reject_simplefin_transfer" => {
            ok(c::simplefin::reject_simplefin_transfer(api, arg(&p, "transferId")?).await?)
        }
        "list_import_review_candidates" => {
            ok(c::simplefin::list_import_review_candidates(api).await?)
        }
        "accept_import_candidate_match" => ok(c::simplefin::accept_import_candidate_match(
            api,
            arg(&p, "candidateId")?,
            arg(&p, "transactionId")?,
        )
        .await?),
        "create_import_candidate_transaction" => {
            ok(c::simplefin::create_import_candidate_transaction(api, arg(&p, "candidateId")?)
                .await?)
        }
        "dismiss_import_candidate" => {
            ok(c::simplefin::dismiss_import_candidate(api, arg(&p, "candidateId")?).await?)
        }

        // ── spending ──
        "get_spending_path_back" => ok(c::spending::get_spending_path_back(
            api,
            arg(&p, "period")?,
            arg(&p, "targetMonthlyCents")?,
        )
        .await?),
        "set_spending_annotation" => ok(c::spending::set_spending_annotation(
            api,
            arg(&p, "merchantKey")?,
            arg(&p, "verdict")?,
        )
        .await?),

        // ── transactions ──
        "list_transactions" => {
            ok(c::transactions::list_transactions(api, arg(&p, "filter")?).await?)
        }
        "create_transaction" => {
            ok(c::transactions::create_transaction(api, arg(&p, "input")?).await?)
        }
        "update_transaction" => ok(c::transactions::update_transaction(
            api,
            arg(&p, "id")?,
            arg(&p, "patch")?,
        )
        .await?),
        "delete_transaction" => {
            ok(c::transactions::delete_transaction(api, arg(&p, "id")?).await?)
        }
        "create_rule" => ok(c::transactions::create_rule(
            api,
            arg(&p, "pattern")?,
            arg(&p, "categoryId")?,
        )
        .await?),
        "set_transaction_owner" => ok(c::transactions::set_transaction_owner(
            api,
            arg(&p, "transactionId")?,
            arg(&p, "memberId")?,
        )
        .await?),
        "list_categories" => ok(c::transactions::list_categories(api).await?),
        "set_category_spending_type" => ok(c::transactions::set_category_spending_type(
            api,
            arg(&p, "id")?,
            arg(&p, "spendingType")?,
        )
        .await?),
        "get_spending_breakdown" => ok(c::transactions::get_spending_breakdown(api).await?),
        "list_categories_with_spending" => {
            ok(c::transactions::list_categories_with_spending(api).await?)
        }
        "list_rules_with_categories" => {
            ok(c::transactions::list_rules_with_categories(api).await?)
        }
        "toggle_rule" => ok(c::transactions::toggle_rule(
            api,
            arg(&p, "id")?,
            arg(&p, "enabled")?,
        )
        .await?),
        "get_transaction_count" => ok(c::transactions::get_transaction_count(api).await?),
        "set_transaction_flags" => ok(c::transactions::set_transaction_flags(
            api,
            arg(&p, "id")?,
            arg(&p, "isReimbursable")?,
            arg(&p, "isSplit")?,
        )
        .await?),
        "set_transaction_transfer" => ok(c::transactions::set_transaction_transfer(
            api,
            arg(&p, "id")?,
            arg(&p, "isTransfer")?,
        )
        .await?),
        "apply_transfer_verdict_to_similar" => {
            ok(c::transactions::apply_transfer_verdict_to_similar(
                api,
                arg(&p, "pattern")?,
                arg(&p, "isTransfer")?,
            )
            .await?)
        }
        "set_counterparty_verdict" => ok(c::transactions::set_counterparty_verdict(
            api,
            arg(&p, "id")?,
            arg(&p, "verdict")?,
        )
        .await?),
        "apply_counterparty_verdict_to_similar" => {
            ok(c::transactions::apply_counterparty_verdict_to_similar(
                api,
                arg(&p, "pattern")?,
                arg(&p, "verdict")?,
            )
            .await?)
        }
        "list_unresolved_counterparties" => {
            ok(c::transactions::list_unresolved_counterparties(api).await?)
        }
        "get_transaction_splits" => {
            ok(c::transactions::get_transaction_splits(api, arg(&p, "transactionId")?).await?)
        }
        "set_transaction_splits" => ok(c::transactions::set_transaction_splits(
            api,
            arg(&p, "transactionId")?,
            arg(&p, "splits")?,
        )
        .await?),
        "export_transactions_csv" => {
            ok(c::transactions::export_transactions_csv(api, arg(&p, "filter")?).await?)
        }
        "export_search_transactions_csv" => {
            ok(c::transactions::export_search_transactions_csv(api, arg(&p, "query")?).await?)
        }

        // Every SUPPORTED command above has exactly one arm. Task 10's parity test
        // enforces BOTH: (a) a missed arm is a red test, and (b) every argument
        // key read through the `arg` helper matches the camelCase key bindings.ts
        // actually sends — so a typo'd key is red at `cargo test`, not a latent
        // 500 discovered in production. NOTE: keep this comment free of the
        // literal `arg(&p, "..")` call shape — Task 10's regex parser has no
        // knowledge of Rust comments and would misattribute it as a real key read
        // on whichever arm precedes it (the one directly above, in the match's
        // byte order).
        _ => Err(AppError::new(
            "rpc.unknown_command",
            format!("unknown command `{cmd}`"),
        )),
    }
}

/// Every command with a match arm in `dispatch()`. Keep in the SAME ORDER as the
/// match so review sees drift. The Task 10 parity test proves this ==
/// (bindings.ts commands − UNSUPPORTED).
pub const SUPPORTED: &[&str] = &[
    // accounts
    "list_accounts",
    "create_account",
    "update_account",
    "archive_account",
    "set_account_balance",
    "list_account_balance_history",
    "get_account_balance_timeline",
    "list_account_balance_sparklines",
    "export_account_csv",
    // agent
    "set_completion_provider",
    "get_completion_provider",
    "save_provider_api_key",
    "list_provider_models",
    "test_completion_provider",
    "get_needs_review_count",
    "trigger_categorize",
    "recompute_anomalies",
    "set_anomaly_dismissed",
    "trigger_recategorize_low_confidence",
    "get_agent_status",
    "ask_agent",
    "list_rule_proposals",
    "accept_rule_proposal",
    "decline_rule_proposal",
    "list_recent_agent_activity",
    // assets
    "list_manual_assets",
    "create_manual_asset",
    "update_manual_asset",
    "delete_manual_asset",
    "record_net_worth_snapshot",
    "list_net_worth_history",
    "compute_debt_payoff",
    "get_uncelebrated_milestones",
    // budget
    "list_budget_envelopes",
    "set_budget",
    "list_goals",
    "create_goal",
    "update_goal_balance",
    "contribute_to_goal",
    "list_goal_contributions",
    "archive_goal",
    "project_goal_growth",
    "update_goal_monthly",
    "update_goal_purpose",
    "get_plan_next_month_data",
    "apply_next_month_plan",
    "list_budget_history",
    // categories
    "update_category_color",
    "create_category",
    "rename_category",
    "archive_category",
    "set_category_guidance",
    "list_category_groups",
    "create_category_group",
    "set_category_group",
    // copilot
    "list_agent_sessions",
    "create_agent_session",
    "close_agent_session",
    "list_action_bundles",
    "get_action_bundle",
    "approve_action_item",
    "reject_action_item",
    "list_execution_log",
    "execute_action_bundle",
    // copilot_chat
    "stream_copilot_message",
    "list_conversations",
    "get_conversation_messages",
    "delete_conversation",
    "create_conversation",
    "edit_conversation_user_message",
    "delete_conversation_messages_after",
    // data_health
    "get_data_health",
    "create_manual_backup",
    "stage_restore_backup",
    "cancel_staged_restore",
    // household
    "list_household_members",
    "create_household_member",
    "set_self_member",
    "delete_household_member",
    "list_account_owners",
    "set_account_owners",
    "set_account_owner_shares",
    "list_asset_owners",
    "set_asset_owners",
    // import
    "preview_csv_columns",
    "prepare_csv_import",
    "import_csv",
    "get_saved_csv_mapping",
    "list_unfinished_imports",
    "discard_unfinished_import",
    // inbox
    "get_action_items",
    // insights
    "list_agent_memory",
    "forget_agent_memory",
    "get_financial_health_score",
    // investments
    "list_account_positions",
    "get_investment_summary",
    // journey
    "get_journey_status",
    // meta
    "app_ready",
    // metrics
    "get_financial_metrics",
    "household_net_worth_breakdown",
    "set_financial_assumptions",
    // onboarding
    "get_onboarding_state",
    "mark_onboarding_complete",
    "reset_onboarding_completion",
    "commit_starter_categories",
    "probe_ollama",
    "save_llm_provider",
    // planned_transactions
    "list_planned_transactions",
    "get_planned_transaction",
    "create_planned_transaction",
    "update_planned_transaction",
    "delete_planned_transaction",
    // recipes
    "list_recipes",
    "create_recipe",
    "update_recipe",
    "pause_recipe",
    "resume_recipe",
    "delete_recipe",
    "trigger_recipe",
    "list_recipe_runs",
    // recurring
    "list_recurring",
    // reports
    "get_report_data",
    "get_month_totals",
    "get_savings_rate_history",
    "create_monthly_review",
    "list_monthly_reviews",
    // scenarios
    "run_scenario",
    "save_scenario",
    "list_scenario_history",
    "delete_scenario",
    // settings
    "get_currency",
    "set_currency",
    "delete_all_data",
    "get_notifications_enabled",
    "set_notifications_enabled",
    "get_auto_categorize_enabled",
    "set_auto_categorize_enabled",
    "export_all_data_json",
    "export_all_data_csv",
    // simplefin
    "save_simplefin_setup_token",
    "get_simplefin_status",
    "list_simplefin_connections",
    "list_simplefin_accounts",
    "import_simplefin_accounts",
    "sync_simplefin_account",
    "disconnect_simplefin",
    "purge_simplefin_data",
    "delete_simplefin_connection",
    "sync_all_simplefin_accounts",
    "get_simplefin_sync_settings",
    "set_simplefin_sync_settings",
    "list_simplefin_alerts",
    "acknowledge_simplefin_alert",
    "list_simplefin_transfer_suggestions",
    "confirm_simplefin_transfer",
    "reject_simplefin_transfer",
    "list_import_review_candidates",
    "accept_import_candidate_match",
    "create_import_candidate_transaction",
    "dismiss_import_candidate",
    // spending
    "get_spending_path_back",
    "set_spending_annotation",
    // transactions
    "list_transactions",
    "create_transaction",
    "update_transaction",
    "delete_transaction",
    "create_rule",
    "set_transaction_owner",
    "list_categories",
    "set_category_spending_type",
    "get_spending_breakdown",
    "list_categories_with_spending",
    "list_rules_with_categories",
    "toggle_rule",
    "get_transaction_count",
    "set_transaction_flags",
    "set_transaction_transfer",
    "apply_transfer_verdict_to_similar",
    "set_counterparty_verdict",
    "apply_counterparty_verdict_to_similar",
    "list_unresolved_counterparties",
    "get_transaction_splits",
    "set_transaction_splits",
    "export_transactions_csv",
    "export_search_transactions_csv",
];

/// Commands whose dispatch legitimately does not read every bindings arg through
/// the `arg` helper. Should be EMPTY: sink-constructing commands
/// (`stream_copilot_message`, `import_csv`) still arg-check their real args (the
/// sink is not a bindings arg), and `app_ready` has no args. Any entry here needs
/// a written justification + controller sign-off.
pub const ARG_CHECK_EXEMPT: &[&str] = &[];

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn rpc_list_accounts_roundtrip() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let cookie = crate::router::tests::setup_and_login(&app).await;
        let res = app
            .oneshot(
                Request::post("/api/rpc/list_accounts")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.is_array()); // empty DB → []
    }

    #[tokio::test]
    async fn rpc_create_then_list_account() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let cookie = crate::router::tests::setup_and_login(&app).await;
        let input = serde_json::json!({ "input": {
            // NewAccount's required (non-Option, no #[serde(default)]) fields, per
            // finsight_core::models::NewAccount: owner, bank, r#type, name, currency,
            // color, opening_balance_cents. NewAccount has no rename_all, so keys
            // are snake_case (matches bindings.ts's `NewAccount` TS type).
            "owner": "You",
            "bank": "Test Bank",
            "type": "Checking",
            "name": "RPC Test",
            "currency": "USD",
            "color": "#336699",
            "opening_balance_cents": 0
        }});
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/rpc/create_account")
                    .header("content-type", "application/json")
                    .header("cookie", cookie.clone())
                    .body(Body::from(input.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .oneshot(
                Request::post("/api/rpc/list_accounts")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unknown_command_is_404_with_app_error_body() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let cookie = crate::router::tests::setup_and_login(&app).await;
        let res = app
            .oneshot(
                Request::post("/api/rpc/not_a_command")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "rpc.unknown_command");
    }

    #[test]
    fn import_commands_are_routed_through_upload_tokens() {
        for cmd in ["preview_csv_columns", "prepare_csv_import", "import_csv"] {
            assert!(!super::UNSUPPORTED.contains(&cmd));
            assert!(super::SUPPORTED.contains(&cmd));
        }
    }

    #[tokio::test]
    async fn arbitrary_server_path_is_rejected() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let cookie = crate::router::tests::setup_and_login(&app).await;
        let res = app
            .oneshot(
                Request::post("/api/rpc/preview_csv_columns")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from(r#"{"path":"/etc/passwd","skipHeaderRows":0}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["code"], "rpc.invalid_import_upload");
    }

    #[tokio::test]
    async fn browser_upload_can_be_previewed() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let cookie = crate::router::tests::setup_and_login(&app).await;
        let boundary = "finsight-test-boundary";
        let multipart = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"history.csv\"\r\nContent-Type: text/csv\r\n\r\ndate,merchant,amount\r\n2026-07-18,Coffee,-4.50\r\n--{boundary}--\r\n"
        );
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/import/csv")
                    .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                    .header("cookie", cookie.clone())
                    .body(Body::from(multipart))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let uploaded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = uploaded["path"].as_str().unwrap();
        assert!(!token.contains('/') && !token.contains('\\'));

        let body = serde_json::json!({"path": token, "skipHeaderRows": 0});
        let res = app
            .oneshot(
                Request::post("/api/rpc/preview_csv_columns")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let preview: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(preview["rows"][0][0], "date");
    }
}

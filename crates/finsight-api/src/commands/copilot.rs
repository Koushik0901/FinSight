use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::cashflow::{self, CashflowForecast, WhatIf};
use finsight_core::metrics;
use finsight_core::models::{
    AgentActionBundle, AgentExecutionEntry, AgentNavigationTarget, AgentSession,
};
use finsight_core::repos::{copilot_actions, copilot_sessions, run};
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileResult {
    pub delta_cents: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSummary {
    pub bundle_id: String,
    pub succeeded: u32,
    pub failed: u32,
    pub results: Vec<ExecutionItemResult>,
    /// Screens the user can open to see the applied changes in context.
    /// Derived from the payloads of actions that succeeded, so these are
    /// always real screens holding real entities. Rendered as an offer.
    pub navigation: Vec<AgentNavigationTarget>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionItemResult {
    pub item_id: String,
    pub action_kind: String,
    pub status: String,
    pub summary: Option<String>,
    pub error: Option<String>,
}

pub async fn list_agent_sessions(state: &ApiState) -> AppResult<Vec<AgentSession>> {
    let db = (*state.db).clone();
    run(&db, |conn| copilot_sessions::list(conn, 50))
        .await
        .map_err(AppError::from)
}

pub async fn create_agent_session(
    state: &ApiState,
    title: String,
    task_type: String,
) -> AppResult<AgentSession> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_sessions::insert(conn, &title, &task_type)
    })
    .await
    .map_err(AppError::from)
}

pub async fn close_agent_session(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_sessions::set_status(conn, &id, "closed")
    })
    .await
    .map_err(AppError::from)
}

pub async fn list_action_bundles(
    state: &ApiState,
    status_filter: Option<String>,
    session_id: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<AgentActionBundle>> {
    let db = (*state.db).clone();
    let limit = limit.unwrap_or(25);
    run(&db, move |conn| {
        copilot_actions::list_bundles(conn, status_filter.as_deref(), session_id.as_deref(), limit)
    })
    .await
    .map_err(AppError::from)
}

pub async fn get_action_bundle(
    state: &ApiState,
    id: String,
) -> AppResult<Option<AgentActionBundle>> {
    let db = (*state.db).clone();
    run(&db, move |conn| copilot_actions::get_bundle(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn approve_action_item(state: &ApiState, item_id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_actions::set_item_status(conn, &item_id, "approved")
    })
    .await
    .map_err(AppError::from)
}

pub async fn reject_action_item(state: &ApiState, item_id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_actions::set_item_status(conn, &item_id, "rejected")
    })
    .await
    .map_err(AppError::from)
}

pub async fn list_execution_log(
    state: &ApiState,
    bundle_id: String,
) -> AppResult<Vec<AgentExecutionEntry>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        copilot_actions::list_execution_log(conn, &bundle_id)
    })
    .await
    .map_err(AppError::from)
}

pub async fn execute_action_bundle(
    state: &ApiState,
    bundle_id: String,
) -> AppResult<ExecutionSummary> {
    let db = (*state.db).clone();
    let result = run(&db, move |conn| {
        finsight_agent::executor::execute_bundle(conn, &bundle_id)
    })
    .await
    .map_err(AppError::from)?;

    Ok(ExecutionSummary {
        bundle_id: result.bundle_id,
        succeeded: result.succeeded as u32,
        failed: result.failed as u32,
        navigation: result.navigation,
        results: result
            .executed
            .into_iter()
            .map(|item| ExecutionItemResult {
                item_id: item.item_id,
                action_kind: item.action_kind,
                status: item.status,
                summary: item.result_summary,
                error: item.error,
            })
            .collect(),
    })
}

pub async fn get_safe_to_spend(
    state: &ApiState,
    horizon_days: Option<i64>,
    buffer_cents: Option<i64>,
    extra_expense_cents: Option<i64>,
) -> AppResult<CashflowForecast> {
    let db = (*state.db).clone();
    let horizon = horizon_days.unwrap_or(cashflow::DEFAULT_HORIZON_DAYS);
    run(&db, move |conn| {
        let whatif = WhatIf {
            buffer_cents: buffer_cents.unwrap_or(0).max(0),
            extra_expense_cents: extra_expense_cents.unwrap_or(0).max(0),
            extra_expense_date: None,
            extra_expense_label: None,
        };
        cashflow::build_forecast(conn, horizon, &whatif)
    })
    .await
    .map_err(AppError::from)
}

pub async fn explain_basis(_state: &ApiState, basis: metrics::ExpenseBasis) -> AppResult<String> {
    Ok(metrics::explain(basis).to_string())
}

pub async fn reconcile_bases(
    state: &ApiState,
    basis_a: metrics::ExpenseBasis,
    basis_b: metrics::ExpenseBasis,
    scope: Option<String>,
) -> AppResult<ReconcileResult> {
    let db = (*state.db).clone();
    let r = run(&db, move |conn| {
        metrics::reconcile(conn, basis_a, basis_b, scope.as_deref())
    })
    .await
    .map_err(AppError::from)?;
    Ok(ReconcileResult {
        delta_cents: r.delta_cents,
        reason: r.reason,
    })
}

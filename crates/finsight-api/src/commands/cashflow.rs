//! Near-term daily cash-flow forecast + safe-to-spend, bridging the frontend to
//! `finsight-core::cashflow`. The what-if parameters (safety buffer, a
//! hypothetical outflow) are evaluated purely — nothing is persisted, so a
//! user can explore "what if I spend $X" without touching real records.

use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::cashflow::{self, CashflowForecast, WhatIf};
use finsight_core::repos::run;

/// Project the liquid balance forward `horizon_days` (default 30, clamped
/// 7–180), optionally against a safety `buffer_cents` and a hypothetical one-off
/// outflow. `include_merchant_keys` force-includes overlooked bills (low-confidence
/// bills/subs) as dated events — the Cashflow screen's checkbox list feeds this.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct GetCashflowForecastRequest {
    pub horizon_days: Option<i64>,
    pub buffer_cents: Option<i64>,
    pub extra_expense_cents: Option<i64>,
    pub extra_expense_date: Option<String>,
    pub include_merchant_keys: Option<Vec<String>>,
}
#[utoipa::path(post, path = "/api/rpc/get_cashflow_forecast", request_body(content = GetCashflowForecastRequest), responses((status = 200, body = CashflowForecast)))]
pub async fn get_cashflow_forecast(
    state: &ApiState,
    horizon_days: Option<i64>,
    buffer_cents: Option<i64>,
    extra_expense_cents: Option<i64>,
    extra_expense_date: Option<String>,
    include_merchant_keys: Option<Vec<String>>,
) -> AppResult<CashflowForecast> {
    let db = (*state.db).clone();
    let horizon = horizon_days.unwrap_or(cashflow::DEFAULT_HORIZON_DAYS);
    let include_keys = include_merchant_keys.unwrap_or_default();
    run(&db, move |conn| {
        let whatif = WhatIf {
            buffer_cents: buffer_cents.unwrap_or(0).max(0),
            extra_expense_cents: extra_expense_cents.unwrap_or(0).max(0),
            extra_expense_date: extra_expense_date
                .as_deref()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
            extra_expense_label: None,
        };
        cashflow::build_forecast(conn, horizon, &whatif, &include_keys)
    })
    .await
    .map_err(AppError::from)
}

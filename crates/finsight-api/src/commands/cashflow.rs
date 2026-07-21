//! Near-term daily cash-flow forecast + safe-to-spend, bridging the frontend to
//! `finsight-core::cashflow`. The what-if parameters (safety buffer, a
//! hypothetical outflow) are evaluated purely — nothing is persisted, so a
//! user can explore "what if I spend $X" without touching real records.

use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::cashflow::{self, CashflowForecast, WhatIf};
use finsight_core::repos::run;

/// Project the liquid balance forward `horizon_days` (default 30, clamped
/// 7–90), optionally against a safety `buffer_cents` and a hypothetical one-off
/// outflow. Returns the daily trajectory, the lowest point, the first day it
/// breaches the buffer, the conservative safe-to-spend, upcoming dated events,
/// and data-quality warnings.
pub async fn get_cashflow_forecast(
    state: &ApiState,
    horizon_days: Option<i64>,
    buffer_cents: Option<i64>,
    extra_expense_cents: Option<i64>,
    extra_expense_date: Option<String>,
) -> AppResult<CashflowForecast> {
    let db = (*state.db).clone();
    let horizon = horizon_days.unwrap_or(cashflow::DEFAULT_HORIZON_DAYS);
    run(&db, move |conn| {
        let whatif = WhatIf {
            buffer_cents: buffer_cents.unwrap_or(0).max(0),
            extra_expense_cents: extra_expense_cents.unwrap_or(0).max(0),
            extra_expense_date: extra_expense_date
                .as_deref()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
            extra_expense_label: None,
        };
        cashflow::build_forecast(conn, horizon, &whatif)
    })
    .await
    .map_err(AppError::from)
}

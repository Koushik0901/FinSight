//! Codegen wrapper for the near-term cash-flow forecast + safe-to-spend.

use crate::error::AppResult;
use crate::AppState;
use finsight_core::cashflow::CashflowForecast;

/// Project the liquid balance forward `horizonDays` (default 30, clamped 7–90),
/// optionally against a safety buffer and a hypothetical one-off outflow — all
/// evaluated purely, nothing persisted.
#[tauri::command]
#[specta::specta]
pub async fn get_cashflow_forecast(
    state: tauri::State<'_, AppState>,
    horizon_days: Option<i64>,
    buffer_cents: Option<i64>,
    extra_expense_cents: Option<i64>,
    extra_expense_date: Option<String>,
) -> AppResult<CashflowForecast> {
    finsight_api::commands::cashflow::get_cashflow_forecast(
        &state.api,
        horizon_days,
        buffer_cents,
        extra_expense_cents,
        extra_expense_date,
    )
    .await
}

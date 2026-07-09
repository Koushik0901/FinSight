//! Frontend bridge to the shared `finsight-core::metrics` layer. Screens read
//! canonical balances, averages, runway, and targets from here rather than
//! recomputing them client-side, so the UI and the Copilot never disagree.

use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::Utc;
use finsight_core::{metrics, repos::run};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FinancialMetrics {
    // Balances (known-balance accounts only), classified by account type.
    pub liquid_cents: i64,
    pub invested_cents: i64,
    pub debt_cents: i64,
    pub emergency_fund_cents: i64,
    pub net_worth_cents: i64,
    pub accounts_with_unknown_balance: i64,
    // Trailing 90-day averages.
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub net_monthly_cents: i64,
    pub rolling_savings_rate_pct: i64,
    // Current calendar month.
    pub this_month_income_cents: i64,
    pub this_month_expense_cents: i64,
    pub this_month_net_cents: i64,
    pub this_month_savings_rate_pct: i64,
    // Derived.
    pub emergency_fund_months: f64,
    pub runway_days: i64,
    // User-configurable targets (settings-backed, framework defaults).
    pub target_savings_rate_pct: i64,
    pub emergency_fund_target_months: f64,
    pub expected_annual_return_pct: f64,
}

#[tauri::command]
#[specta::specta]
pub async fn get_financial_metrics(
    state: tauri::State<'_, AppState>,
) -> AppResult<FinancialMetrics> {
    let db = (*state.db).clone();
    let month_start = Utc::now().format("%Y-%m-01").to_string();
    run(&db, move |conn| {
        let balances = metrics::balance_breakdown(conn)?;
        let rolling = metrics::rolling_averages(conn, 90)?;
        let this_month = metrics::cashflow_since(conn, &month_start)?;
        let emergency_fund_months = metrics::emergency_fund_months(
            balances.emergency_fund_cents,
            rolling.avg_monthly_expense_cents,
        );
        let runway_days =
            metrics::runway_days(balances.liquid_cents, rolling.avg_monthly_expense_cents);
        let assumptions = metrics::assumptions(conn);
        Ok(FinancialMetrics {
            liquid_cents: balances.liquid_cents,
            invested_cents: balances.invested_cents,
            debt_cents: balances.debt_cents,
            emergency_fund_cents: balances.emergency_fund_cents,
            net_worth_cents: balances.net_worth_cents,
            accounts_with_unknown_balance: balances.accounts_with_unknown_balance,
            avg_monthly_income_cents: rolling.avg_monthly_income_cents,
            avg_monthly_expense_cents: rolling.avg_monthly_expense_cents,
            net_monthly_cents: rolling.net_monthly_cents,
            rolling_savings_rate_pct: rolling.savings_rate_pct,
            this_month_income_cents: this_month.income_cents,
            this_month_expense_cents: this_month.expense_cents,
            this_month_net_cents: this_month.net_cents,
            this_month_savings_rate_pct: this_month.savings_rate_pct,
            emergency_fund_months,
            runway_days,
            target_savings_rate_pct: assumptions.target_savings_rate_pct,
            emergency_fund_target_months: assumptions.emergency_fund_target_months,
            expected_annual_return_pct: assumptions.expected_annual_return_pct,
        })
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FinancialAssumptionsInput {
    pub target_savings_rate_pct: i64,
    pub emergency_fund_target_months: f64,
    pub expected_annual_return_pct: f64,
}

#[tauri::command]
#[specta::specta]
pub async fn set_financial_assumptions(
    state: tauri::State<'_, AppState>,
    input: FinancialAssumptionsInput,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        metrics::set_assumptions(
            conn,
            &metrics::Assumptions {
                // Clamp to sane ranges so a stray value can't poison every
                // downstream calculation.
                target_savings_rate_pct: input.target_savings_rate_pct.clamp(0, 100),
                emergency_fund_target_months: input.emergency_fund_target_months.clamp(0.0, 24.0),
                expected_annual_return_pct: input.expected_annual_return_pct.clamp(0.0, 30.0),
            },
        )
    })
    .await
    .map_err(AppError::from)
}

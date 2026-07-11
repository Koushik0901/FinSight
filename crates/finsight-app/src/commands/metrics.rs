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

/// `get_financial_metrics`, optionally scoped to one household member. A `None`
/// member returns the whole-household numbers (unchanged); `Some(id)` weights
/// every figure by the member's ownership share (explicit `share_bps`, else an
/// equal split), so the per-person view reconciles to the household total plus
/// the unassigned residual.
#[tauri::command]
#[specta::specta]
pub async fn get_financial_metrics(
    state: tauri::State<'_, AppState>,
    member_id: Option<String>,
) -> AppResult<FinancialMetrics> {
    let db = (*state.db).clone();
    let month_start = Utc::now().format("%Y-%m-01").to_string();
    run(&db, move |conn| {
        let member = member_id.as_deref();
        let balances = metrics::balance_breakdown_for(conn, member)?;
        let rolling = metrics::rolling_averages_for(conn, 90, member)?;
        let this_month = metrics::cashflow_since_for(conn, &month_start, member)?;
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

/// One row of the "who owns what" household net-worth split. `member_id` None is
/// the unassigned residual — value owned by no recorded member, i.e. by people
/// running their OWN separate FinSight app (the cross-user share). Member slices
/// plus the residual reconcile to the household total.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MemberNetWorth {
    pub member_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
    pub net_worth_cents: i64,
    pub liquid_cents: i64,
    pub invested_cents: i64,
    pub debt_cents: i64,
}

/// Each household member's share of net worth (share-weighted across accounts AND
/// jointly-owned assets, via the metrics layer — NOT a client-side equal split),
/// plus an "unassigned" residual so the rows sum to the household total.
#[tauri::command]
#[specta::specta]
pub async fn household_net_worth_breakdown(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<MemberNetWorth>> {
    use finsight_core::repos::household;
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let members = household::list_members(conn)?;
        let household_bd = metrics::balance_breakdown_for(conn, None)?;
        let mut out = Vec::new();
        let (mut nw, mut liq, mut inv, mut debt) = (0i64, 0i64, 0i64, 0i64);
        for m in &members {
            let bd = metrics::balance_breakdown_for(conn, Some(&m.id))?;
            nw += bd.net_worth_cents;
            liq += bd.liquid_cents;
            inv += bd.invested_cents;
            debt += bd.debt_cents;
            out.push(MemberNetWorth {
                member_id: Some(m.id.clone()),
                name: m.name.clone(),
                color: m.color.clone(),
                net_worth_cents: bd.net_worth_cents,
                liquid_cents: bd.liquid_cents,
                invested_cents: bd.invested_cents,
                debt_cents: bd.debt_cents,
            });
        }
        // The unattributed remainder: ownerless accounts/assets and the shares of
        // jointly-owned items owned by people in their own separate apps.
        let residual = household_bd.net_worth_cents - nw;
        if residual != 0 || (out.is_empty() && household_bd.net_worth_cents != 0) {
            out.push(MemberNetWorth {
                member_id: None,
                name: "Unassigned / shared".to_string(),
                color: None,
                net_worth_cents: residual,
                liquid_cents: household_bd.liquid_cents - liq,
                invested_cents: household_bd.invested_cents - inv,
                debt_cents: household_bd.debt_cents - debt,
            });
        }
        Ok(out)
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

//! Frontend bridge to the shared `finsight-core::metrics` layer. Screens read
//! canonical balances, averages, runway, and targets from here rather than
//! recomputing them client-side, so the UI and the Copilot never disagree.

use crate::error::{AppError, AppResult};
use crate::ApiState;
use chrono::Utc;
use finsight_core::{metrics, repos::run};
use serde::{Deserialize, Serialize};
use specta::Type;

/// A currency the user holds that these metrics are NOT denominated in.
/// Reported so the UI can say "also holding US$3,200, not converted" instead of
/// either inventing an exchange rate or silently omitting real money.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnconvertedHolding {
    pub code: String,
    pub account_count: i64,
    pub balance_cents: i64,
}

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
    // Derived. `None` means the app declines to state a figure rather than
    // guessing — too little history, or a member-scoped query where a personal
    // share of household survival time is not a meaningful quantity. Consumers
    // must render an explicit "not yet" instead of substituting a number.
    pub emergency_fund_months: Option<f64>,
    pub runway_days: Option<i64>,
    /// Days of history behind the safety basis, so the UI can say WHY a figure
    /// is withheld instead of showing a bare dash.
    pub safety_basis_span_days: i64,
    // User-configurable targets (settings-backed, framework defaults).
    pub target_savings_rate_pct: i64,
    pub emergency_fund_target_months: f64,
    pub expected_annual_return_pct: f64,
    /// The currency every `_cents` field above is denominated in, derived from
    /// the user's accounts rather than from a display preference — a preference
    /// goes stale the moment they open an account in another currency. `None`
    /// only when there are no accounts yet.
    pub currency: Option<String>,
    /// Money held in other currencies, never converted and never folded into
    /// the totals above. Non-empty means every figure here is a partial view,
    /// and the UI must label it as such rather than render a bare number.
    pub unconverted_holdings: Vec<UnconvertedHolding>,
}

pub async fn get_financial_metrics(
    state: &ApiState,
    member_id: Option<String>,
) -> AppResult<FinancialMetrics> {
    let db = (*state.db).clone();
    let month_start = Utc::now().format("%Y-%m-01").to_string();
    run(&db, move |conn| {
        let member = member_id.as_deref();
        let balances = metrics::balance_breakdown_for(conn, member)?;
        let rolling = metrics::rolling_averages_for(conn, 90, member)?;
        let this_month = metrics::cashflow_since_for(conn, &month_start, member)?;
        // Safety metrics use the conservative basis, NOT the 90-day mean the
        // descriptive figures above use: a median or short-window mean hides
        // annual obligations, and measuring survival time against a number that
        // excludes your insurance bill overstates how safe you are.
        //
        // Household-scoped by definition — nobody survives on their share of a
        // joint runway — so a member-filtered query withholds rather than
        // inventing a personal figure. (No screen shows per-member runway.)
        let safety = metrics::safety_expense_basis(conn)?;
        let (emergency_fund_months, runway_days) = if member.is_some() || !safety.sufficient {
            (None, None)
        } else {
            (
                Some(metrics::emergency_fund_months(
                    balances.emergency_fund_cents,
                    safety.monthly_expense_cents,
                )),
                Some(metrics::runway_days(
                    balances.liquid_cents,
                    safety.monthly_expense_cents,
                )),
            )
        };
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
            safety_basis_span_days: safety.data_span_days,
            target_savings_rate_pct: assumptions.target_savings_rate_pct,
            emergency_fund_target_months: assumptions.emergency_fund_target_months,
            expected_annual_return_pct: assumptions.expected_annual_return_pct,
            currency: balances.currency.clone(),
            unconverted_holdings: balances
                .unconverted
                .iter()
                .map(|h| UnconvertedHolding {
                    code: h.code.clone(),
                    account_count: h.account_count,
                    balance_cents: h.balance_cents,
                })
                .collect(),
        })
    })
    .await
    .map_err(AppError::from)
}

/// Structured "explain this number" provenance for the decision-driving
/// dashboard metrics, optionally scoped to one household member. Every value is
/// pulled from the same `finsight-core::metrics` layer `get_financial_metrics`
/// reads, so an explanation can never disagree with the number shown elsewhere.
/// The single source of truth is shared verbatim with the Copilot's
/// `explain_metric` tool.
pub async fn explain_financial_metrics(
    state: &ApiState,
    member_id: Option<String>,
) -> AppResult<Vec<finsight_core::provenance::MetricExplanation>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        finsight_core::provenance::explain_financial_metrics(conn, member_id.as_deref())
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

pub async fn household_net_worth_breakdown(state: &ApiState) -> AppResult<Vec<MemberNetWorth>> {
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

pub async fn set_financial_assumptions(
    state: &ApiState,
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

// ── Financial philosophy ────────────────────────────────────────────────────

/// The user's stated philosophy, as the UI reads and writes it.
///
/// Strings rather than enums on the wire so an older client that sends an
/// unrecognised value degrades to the default instead of failing the request —
/// a preference is never worth erroring over.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FinancialPhilosophyDto {
    /// "avalanche" | "snowball"
    pub debt_strategy: String,
    /// "cautious" | "balanced" | "aggressive"
    pub risk_tolerance: String,
    /// Derived, read-only: the APR at or above which debt is treated as
    /// high-interest. Surfaced so the Settings screen can show the
    /// consequence of the choice rather than just its name.
    pub high_interest_apr_pct: f64,
}

fn philosophy_to_dto(p: metrics::FinancialPhilosophy) -> FinancialPhilosophyDto {
    FinancialPhilosophyDto {
        debt_strategy: p.debt_strategy.as_method().to_string(),
        risk_tolerance: match p.risk_tolerance {
            metrics::RiskTolerance::Cautious => "cautious",
            metrics::RiskTolerance::Balanced => "balanced",
            metrics::RiskTolerance::Aggressive => "aggressive",
        }
        .to_string(),
        high_interest_apr_pct: p.risk_tolerance.high_interest_apr_pct(),
    }
}

/// Parse a risk-tolerance name, falling back to the default for anything
/// unrecognised. Never errors: an unknown string must behave as "untouched".
fn parse_risk_tolerance(raw: &str) -> metrics::RiskTolerance {
    match raw.trim().to_ascii_lowercase().as_str() {
        "cautious" => metrics::RiskTolerance::Cautious,
        "aggressive" => metrics::RiskTolerance::Aggressive,
        _ => metrics::RiskTolerance::Balanced,
    }
}

pub async fn get_financial_philosophy(state: &ApiState) -> AppResult<FinancialPhilosophyDto> {
    let db = (*state.db).clone();
    run(&db, move |conn| Ok(philosophy_to_dto(metrics::philosophy(conn))))
        .await
        .map_err(AppError::from)
}

pub async fn set_financial_philosophy(
    state: &ApiState,
    input: FinancialPhilosophyDto,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        metrics::set_philosophy(
            conn,
            &metrics::FinancialPhilosophy {
                debt_strategy: metrics::DebtStrategy::from_method(&input.debt_strategy),
                risk_tolerance: parse_risk_tolerance(&input.risk_tolerance),
            },
        )
    })
    .await
    .map_err(AppError::from)
}

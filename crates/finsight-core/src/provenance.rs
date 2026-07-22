//! "Explain this number" — structured, self-consistent provenance for the
//! decision-driving financial metrics.
//!
//! Every value here is pulled from the shared [`crate::metrics`] layer — the
//! same functions the dashboard and the Copilot already read — so an
//! explanation can never disagree with the number it explains. This module adds
//! NO arithmetic of its own and issues NO parallel SQL: it fetches the exact
//! intermediates [`crate::metrics`] produces and *describes* them.
//!
//! Prose is deliberately currency- and value-neutral. Amounts live only in the
//! structured [`MetricInput::amount_cents`] / [`MetricValue`] fields so the UI
//! formats them in the user's own currency; warnings and exclusions never
//! embed a hard-coded symbol or a figure keyed to any particular user's data.

use crate::metrics::{self, Assumptions, BalanceBreakdown, RollingAverages, SafetyExpenseBasis};
use crate::CoreResult;
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

/// The trailing window (days) the descriptive averages are computed over. Kept
/// in lockstep with what [`get_financial_metrics`] requests (90) so the
/// explanations describe the very same figures the dashboard shows.
const ROLLING_WINDOW_DAYS: i64 = 90;

/// Severity of a data-quality caveat attached to a metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MetricWarningLevel {
    /// Neutral context — e.g. which window was used. Does not undermine the figure.
    Info,
    /// The figure stands, but this caveat materially affects how far to trust it.
    Caution,
    /// The figure is withheld entirely; [`MetricExplanation::value`] is
    /// [`MetricValue::Withheld`]. Better to say "not yet" than a confident wrong number.
    Withheld,
}

/// One data-quality caveat: missing, stale, low-confidence, or unsupported data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MetricWarning {
    pub level: MetricWarningLevel,
    pub message: String,
}

/// A material input that fed the metric. `amount_cents` is present when the
/// input is a money figure (formatted by the UI in the user's currency);
/// `detail` carries non-money context such as "3 months of history".
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MetricInput {
    pub label: String,
    pub amount_cents: Option<i64>,
    pub detail: Option<String>,
}

/// A tunable assumption that shaped the metric (e.g. the user's target rate).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MetricAssumption {
    pub label: String,
    pub value: String,
}

/// The metric's value, tagged so the UI knows how to format it and so a
/// withheld figure is a first-class state rather than a fabricated zero.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MetricValue {
    Money { cents: i64 },
    Percent { pct: i64 },
    Months { months: f64 },
    Days { days: i64 },
    /// The app declines to state a figure; see `warnings` for why.
    Withheld,
}

/// A complete, self-consistent explanation of one financial metric: what it
/// means, what produced it, what it leaves out, what it assumes, over what
/// period, and how far to trust it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MetricExplanation {
    /// Stable identifier, e.g. `"net_worth"`. Safe to switch on.
    pub key: String,
    pub label: String,
    /// The value — identical to what the app shows elsewhere, by construction.
    pub value: MetricValue,
    /// One-sentence definition of what the number represents.
    pub definition: String,
    /// The material inputs that produced it, with their amounts where money.
    pub inputs: Vec<MetricInput>,
    /// What was deliberately left out.
    pub exclusions: Vec<String>,
    /// Tunable assumptions that shaped it.
    pub assumptions: Vec<MetricAssumption>,
    /// Material tradeoffs between this result and the competing option(s) — the
    /// "why this and not the alternative" reasoning. Empty for a descriptive
    /// metric that has no alternative (e.g. net worth); populated for a
    /// recommendation such as a debt-payoff order or a contribution level.
    #[serde(default)]
    pub tradeoffs: Vec<String>,
    /// The time window the figure covers.
    pub period: String,
    /// Data-quality caveats: missing / stale / withheld / low-confidence disclosures.
    pub warnings: Vec<MetricWarning>,
}

/// Explanations for the decision-driving dashboard metrics, optionally scoped to
/// one household member. Fetches the identical intermediates
/// [`get_financial_metrics`] uses, then hands them to [`assemble`] — so the
/// explained value and the displayed value are the same computation.
///
/// [`get_financial_metrics`]: (finsight-api) crates/finsight-api/src/commands/metrics.rs
pub fn explain_financial_metrics(
    conn: &mut Connection,
    member: Option<&str>,
) -> CoreResult<Vec<MetricExplanation>> {
    let balances = metrics::balance_breakdown_for(conn, member)?;
    let rolling = metrics::rolling_averages_for(conn, ROLLING_WINDOW_DAYS, member)?;
    // Safety metrics are household-scoped by definition (nobody survives on
    // their share of a joint runway), exactly as get_financial_metrics treats
    // them — so this basis is intentionally NOT member-filtered.
    let safety = metrics::safety_expense_basis(conn)?;
    let assumptions = metrics::assumptions(conn);
    Ok(assemble(&balances, &rolling, &safety, &assumptions, member))
}

/// Pure assembly: turn the metric intermediates into explanations. Split out
/// from the DB fetch so every branch (fresh user, unknown balances, mixed
/// currency, full history) is unit-testable without seeding a database.
fn assemble(
    balances: &BalanceBreakdown,
    rolling: &RollingAverages,
    safety: &SafetyExpenseBasis,
    assumptions: &Assumptions,
    member: Option<&str>,
) -> Vec<MetricExplanation> {
    vec![
        net_worth(balances),
        avg_monthly_income(balances, rolling),
        avg_monthly_expense(balances, rolling),
        monthly_surplus(rolling),
        savings_rate(rolling, assumptions),
        emergency_fund_months(balances, safety, assumptions, member),
        runway_days(balances, safety, member),
    ]
}

// ── Shared caveat builders ──────────────────────────────────────────────────

/// Exclusion + warning for accounts whose balance isn't confirmed. Balances
/// metrics exclude these rather than invent a $0, and say so.
fn unknown_balance_caveats(balances: &BalanceBreakdown, into_ex: &mut Vec<String>, into_warn: &mut Vec<MetricWarning>) {
    let n = balances.accounts_with_unknown_balance;
    if n > 0 {
        into_ex.push(format!(
            "{n} account{} with no confirmed balance {} excluded — counted as unknown, never as zero.",
            if n == 1 { "" } else { "s" },
            if n == 1 { "is" } else { "are" },
        ));
        into_warn.push(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: format!(
                "This figure omits {n} account{} whose balance hasn't been confirmed; it may be incomplete until you record or sync {} balance.",
                if n == 1 { "" } else { "s" },
                if n == 1 { "its" } else { "their" },
            ),
        });
    }
}

/// Exclusion + warning for money held in currencies the aggregate isn't
/// denominated in. Never converted, never folded in — so every total is a
/// partial view and must be labelled as such.
fn unconverted_currency_caveats(balances: &BalanceBreakdown, into_ex: &mut Vec<String>, into_warn: &mut Vec<MetricWarning>) {
    if balances.unconverted.is_empty() {
        return;
    }
    let codes: Vec<&str> = balances.unconverted.iter().map(|h| h.code.as_str()).collect();
    into_ex.push(format!(
        "Money held in {} is not converted and not included (no exchange rate is invented).",
        codes.join(", ")
    ));
    into_warn.push(MetricWarning {
        level: MetricWarningLevel::Caution,
        message: format!(
            "You also hold money in {}. It isn't converted into this currency, so this is a partial view.",
            codes.join(", ")
        ),
    });
}

/// The shared "these never count" exclusions for every cashflow-derived figure.
fn cashflow_exclusions(balances: &BalanceBreakdown) -> Vec<String> {
    let mut ex = vec![
        "Transfers between your own accounts (they aren't income or spending).".to_string(),
        "Activity inside investment accounts (buys/sells aren't cashflow).".to_string(),
    ];
    if !balances.unconverted.is_empty() {
        let codes: Vec<&str> = balances.unconverted.iter().map(|h| h.code.as_str()).collect();
        ex.push(format!(
            "Transactions in other currencies ({}) — only your primary currency is totalled.",
            codes.join(", ")
        ));
    }
    ex
}

/// Warning when the trailing window holds too little history for a monthly
/// average to be anything but an extrapolation from a partial month.
fn thin_history_warning(rolling: &RollingAverages) -> Option<MetricWarning> {
    if rolling.data_span_days <= 0 {
        Some(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: "No transaction history in this window yet, so this is $0 by default rather than a measured figure.".to_string(),
        })
    } else if rolling.data_span_days < metrics::SAFETY_BASIS_MIN_SPAN_DAYS {
        Some(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: format!(
                "Only {} day(s) of history so far — this monthly average is extrapolated from a partial month and will settle as more data arrives.",
                rolling.data_span_days
            ),
        })
    } else {
        None
    }
}

/// "Averaged over N month(s) of activity" — the honest divisor behind every
/// rolling average, so nobody mistakes a new user's one month for three.
fn averaging_input(rolling: &RollingAverages) -> MetricInput {
    MetricInput {
        label: "Averaged over".to_string(),
        amount_cents: None,
        detail: Some(format!(
            "{} month(s) of activity in the last {} days",
            rolling.months, rolling.window_days
        )),
    }
}

// ── Individual metric explanations ──────────────────────────────────────────

fn net_worth(balances: &BalanceBreakdown) -> MetricExplanation {
    let mut inputs = vec![
        MetricInput { label: "Cash & liquid accounts".into(), amount_cents: Some(balances.liquid_cents), detail: None },
        MetricInput { label: "Investments".into(), amount_cents: Some(balances.invested_cents), detail: None },
        MetricInput { label: "Debts".into(), amount_cents: Some(-balances.debt_cents), detail: Some("credit cards & loans, counted as negative".into()) },
    ];
    // Whatever net worth includes beyond these three (manual assets, etc.),
    // surfaced as a residual so the inputs always sum to the value shown.
    let residual = balances.net_worth_cents - (balances.liquid_cents + balances.invested_cents - balances.debt_cents);
    if residual != 0 {
        inputs.push(MetricInput { label: "Manual assets & other holdings".into(), amount_cents: Some(residual), detail: None });
    }
    let mut exclusions = Vec::new();
    let mut warnings = Vec::new();
    unknown_balance_caveats(balances, &mut exclusions, &mut warnings);
    unconverted_currency_caveats(balances, &mut exclusions, &mut warnings);
    MetricExplanation {
        key: "net_worth".into(),
        label: "Net worth".into(),
        value: MetricValue::Money { cents: balances.net_worth_cents },
        definition: "Everything you own minus everything you owe: confirmed account balances (debts negative) plus any manual assets.".into(),
        inputs,
        exclusions,
        assumptions: Vec::new(),
        tradeoffs: Vec::new(),
        period: "As of today".into(),
        warnings,
    }
}

fn avg_monthly_income(balances: &BalanceBreakdown, rolling: &RollingAverages) -> MetricExplanation {
    let mut warnings = Vec::new();
    if let Some(w) = thin_history_warning(rolling) { warnings.push(w); }
    MetricExplanation {
        key: "avg_monthly_income".into(),
        label: "Average monthly income".into(),
        value: MetricValue::Money { cents: rolling.avg_monthly_income_cents },
        definition: "Your typical income per month: money coming in, averaged over the months of history in the window.".into(),
        inputs: vec![averaging_input(rolling)],
        exclusions: {
            let mut ex = cashflow_exclusions(balances);
            ex.push("Reimbursements you were paid back (they net against the original expense, not counted as income).".into());
            ex
        },
        assumptions: Vec::new(),
        tradeoffs: Vec::new(),
        period: format!("Trailing {} days", rolling.window_days),
        warnings,
    }
}

fn avg_monthly_expense(balances: &BalanceBreakdown, rolling: &RollingAverages) -> MetricExplanation {
    let mut warnings = Vec::new();
    if let Some(w) = thin_history_warning(rolling) { warnings.push(w); }
    MetricExplanation {
        key: "avg_monthly_expense".into(),
        label: "Average monthly spending".into(),
        value: MetricValue::Money { cents: rolling.avg_monthly_expense_cents },
        definition: "Your typical spending per month: money going out, averaged over the months of history in the window.".into(),
        inputs: vec![averaging_input(rolling)],
        exclusions: cashflow_exclusions(balances),
        assumptions: Vec::new(),
        tradeoffs: Vec::new(),
        period: format!("Trailing {} days", rolling.window_days),
        warnings,
    }
}

fn monthly_surplus(rolling: &RollingAverages) -> MetricExplanation {
    let mut warnings = Vec::new();
    if let Some(w) = thin_history_warning(rolling) { warnings.push(w); }
    MetricExplanation {
        key: "monthly_surplus".into(),
        label: "Monthly surplus".into(),
        value: MetricValue::Money { cents: rolling.net_monthly_cents },
        definition: "What's left over in a typical month: average income minus average spending.".into(),
        inputs: vec![
            MetricInput { label: "Average monthly income".into(), amount_cents: Some(rolling.avg_monthly_income_cents), detail: None },
            MetricInput { label: "Average monthly spending".into(), amount_cents: Some(-rolling.avg_monthly_expense_cents), detail: None },
        ],
        exclusions: vec!["Transfers and investment-account activity (see income and spending).".into()],
        assumptions: Vec::new(),
        tradeoffs: Vec::new(),
        period: format!("Trailing {} days", rolling.window_days),
        warnings,
    }
}

fn savings_rate(rolling: &RollingAverages, assumptions: &Assumptions) -> MetricExplanation {
    let mut warnings = Vec::new();
    if let Some(w) = thin_history_warning(rolling) { warnings.push(w); }
    if rolling.avg_monthly_income_cents <= 0 {
        warnings.push(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: "No income recorded in this window, so the rate reads 0% by definition rather than being measured.".into(),
        });
    }
    MetricExplanation {
        key: "savings_rate".into(),
        label: "Savings rate".into(),
        value: MetricValue::Percent { pct: rolling.savings_rate_pct },
        definition: "The share of your income you keep: (income − spending) ÷ income, over the window.".into(),
        inputs: vec![
            MetricInput { label: "Average monthly income".into(), amount_cents: Some(rolling.avg_monthly_income_cents), detail: None },
            MetricInput { label: "Average monthly spending".into(), amount_cents: Some(rolling.avg_monthly_expense_cents), detail: None },
        ],
        exclusions: vec!["Transfers and investment-account activity.".into()],
        assumptions: vec![MetricAssumption {
            label: "Your target savings rate".into(),
            value: format!("{}%", assumptions.target_savings_rate_pct),
        }],
        tradeoffs: Vec::new(),
        period: format!("Trailing {} days", rolling.window_days),
        warnings,
    }
}

fn emergency_fund_months(
    balances: &BalanceBreakdown,
    safety: &SafetyExpenseBasis,
    assumptions: &Assumptions,
    member: Option<&str>,
) -> MetricExplanation {
    let inputs = vec![
        MetricInput { label: "Emergency-fund savings".into(), amount_cents: Some(balances.emergency_fund_cents), detail: None },
        MetricInput { label: "Conservative monthly spending".into(), amount_cents: Some(safety.monthly_expense_cents), detail: Some("the larger of your 12-month and 90-day average, so annual bills are counted".into()) },
    ];
    let assumption = MetricAssumption {
        label: "Your target".into(),
        value: format!("{} months of expenses", assumptions.emergency_fund_target_months),
    };
    let period = "As of today, at your conservative monthly spending".to_string();
    let definition = "How many months your emergency-fund savings would cover at your typical spending.".to_string();

    // Mirror get_financial_metrics EXACTLY: withhold for a member scope (a
    // personal share of household survival time isn't meaningful) and when
    // history is too thin for an honest monthly burn.
    if member.is_some() {
        return MetricExplanation {
            key: "emergency_fund_months".into(),
            label: "Emergency-fund coverage".into(),
            value: MetricValue::Withheld,
            definition,
            inputs,
            exclusions: Vec::new(),
            assumptions: vec![assumption],
            tradeoffs: Vec::new(),
            period,
            warnings: vec![MetricWarning {
                level: MetricWarningLevel::Withheld,
                message: "Not shown per-person: surviving on a personal share of a shared runway isn't a meaningful figure. It's reported for the whole household instead.".into(),
            }],
        };
    }
    if !safety.sufficient {
        return MetricExplanation {
            key: "emergency_fund_months".into(),
            label: "Emergency-fund coverage".into(),
            value: MetricValue::Withheld,
            definition,
            inputs,
            exclusions: Vec::new(),
            assumptions: vec![assumption],
            tradeoffs: Vec::new(),
            period,
            warnings: vec![MetricWarning {
                level: MetricWarningLevel::Withheld,
                message: format!(
                    "Withheld until there's about {} days of history — currently {}. A confident wrong number here would overstate how safe you are.",
                    metrics::SAFETY_BASIS_MIN_SPAN_DAYS, safety.data_span_days
                ),
            }],
        };
    }
    let months = metrics::emergency_fund_months(balances.emergency_fund_cents, safety.monthly_expense_cents);
    let mut warnings = vec![MetricWarning {
        level: MetricWarningLevel::Info,
        message: format!("Based on {} complete month(s) of spending history.", safety.months_observed),
    }];
    if safety.monthly_expense_cents <= 0 {
        warnings.push(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: "Your typical spending is about zero in this data, so months of coverage can't be expressed meaningfully.".into(),
        });
    }
    MetricExplanation {
        key: "emergency_fund_months".into(),
        label: "Emergency-fund coverage".into(),
        value: MetricValue::Months { months },
        definition,
        inputs,
        exclusions: Vec::new(),
        assumptions: vec![assumption],
        tradeoffs: Vec::new(),
        period,
        warnings,
    }
}

fn runway_days(balances: &BalanceBreakdown, safety: &SafetyExpenseBasis, member: Option<&str>) -> MetricExplanation {
    let inputs = vec![
        MetricInput { label: "Liquid cash".into(), amount_cents: Some(balances.liquid_cents), detail: None },
        MetricInput { label: "Conservative monthly spending".into(), amount_cents: Some(safety.monthly_expense_cents), detail: None },
    ];
    let period = "As of today, at your conservative monthly spending".to_string();
    let definition = "How long your liquid cash would last with no new income, at your typical spending.".to_string();

    if member.is_some() {
        return MetricExplanation {
            key: "runway_days".into(),
            label: "Cash runway".into(),
            value: MetricValue::Withheld,
            definition,
            inputs,
            exclusions: Vec::new(),
            assumptions: Vec::new(),
            tradeoffs: Vec::new(),
            period,
            warnings: vec![MetricWarning {
                level: MetricWarningLevel::Withheld,
                message: "Not shown per-person: a personal share of a shared runway isn't meaningful. Reported for the whole household instead.".into(),
            }],
        };
    }
    if !safety.sufficient {
        return MetricExplanation {
            key: "runway_days".into(),
            label: "Cash runway".into(),
            value: MetricValue::Withheld,
            definition,
            inputs,
            exclusions: Vec::new(),
            assumptions: Vec::new(),
            tradeoffs: Vec::new(),
            period,
            warnings: vec![MetricWarning {
                level: MetricWarningLevel::Withheld,
                message: format!(
                    "Withheld until there's about {} days of history — currently {}.",
                    metrics::SAFETY_BASIS_MIN_SPAN_DAYS, safety.data_span_days
                ),
            }],
        };
    }
    let days = metrics::runway_days(balances.liquid_cents, safety.monthly_expense_cents);
    MetricExplanation {
        key: "runway_days".into(),
        label: "Cash runway".into(),
        value: MetricValue::Days { days },
        definition,
        inputs,
        exclusions: Vec::new(),
        assumptions: Vec::new(),
        tradeoffs: Vec::new(),
        period,
        warnings: vec![MetricWarning {
            level: MetricWarningLevel::Info,
            message: format!("Based on {} complete month(s) of spending history.", safety.months_observed),
        }],
    }
}

// ── Planning-output explanations (issue #71) ────────────────────────────────
//
// Debt payoff, goals, and scenarios are *recommendations*, not descriptive
// dashboard figures. They reuse the same [`MetricExplanation`] shape — so one
// inspector renders them and the Copilot reads one contract — but they populate
// `tradeoffs` (the "why this and not the alternative" reasoning the dashboard
// metrics leave empty) and, like every metric here, describe numbers an engine
// already produced rather than computing anything new.

/// Explain a saved scenario's *current* projected result. Pure: the caller
/// (finsight-api) rebuilds the scenario's params, the current baseline
/// [`Snapshot`](crate::forecast::Snapshot), and the
/// [`Projection`](crate::forecast::Projection) via the same `forecast::project`
/// the Scenarios screen shows, then hands them here to be described. The
/// narrative `considerations` the engine already generated become the
/// tradeoffs, so this explanation can never disagree with the scenario card.
pub fn scenario_explanation(
    label: &str,
    params: &crate::forecast::ScenarioParams,
    baseline: &crate::forecast::Snapshot,
    projection: &crate::forecast::Projection,
    is_stale: bool,
    horizon_months: u32,
) -> MetricExplanation {
    let mut inputs = vec![
        MetricInput { label: "Starting balance".into(), amount_cents: Some(baseline.balance_cents), detail: None },
        MetricInput { label: "Baseline monthly income".into(), amount_cents: Some(baseline.avg_monthly_income_cents), detail: None },
        MetricInput { label: "Baseline monthly spending".into(), amount_cents: Some(baseline.avg_monthly_expense_cents), detail: None },
    ];
    // The scenario's own levers — only those actually set, so the explanation
    // lists what THIS scenario changes, not every possible knob.
    if params.income_delta_pct != 0 {
        inputs.push(MetricInput {
            label: "Income change".into(),
            amount_cents: None,
            detail: Some(format!("{:+}% vs baseline", params.income_delta_pct)),
        });
    }
    if params.monthly_expense_delta_cents != 0 {
        // Sign mirrors ScenarioParams: positive = more outflow.
        inputs.push(MetricInput {
            label: "Monthly spending change".into(),
            amount_cents: Some(params.monthly_expense_delta_cents),
            detail: None,
        });
    }
    if params.one_time_cents != 0 {
        inputs.push(MetricInput {
            label: "One-time amount".into(),
            amount_cents: Some(params.one_time_cents),
            detail: None,
        });
    }

    let starts = if params.start_month_offset == 0 {
        "immediately".to_string()
    } else {
        format!("in {} month(s)", params.start_month_offset)
    };
    let assumptions = vec![
        MetricAssumption { label: "Projection horizon".into(), value: format!("{} months", horizon_months.max(1)) },
        MetricAssumption { label: "Change starts".into(), value: starts },
        MetricAssumption { label: "Basis".into(), value: "Your trailing 90-day averages, projected flat".into() },
    ];

    let mut warnings = Vec::new();
    if is_stale {
        warnings.push(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: "The finances this scenario was saved against have moved materially since. This is it re-run on today's numbers; the saved result may differ.".into(),
        });
    }
    if !projection.verdict {
        warnings.push(MetricWarning {
            level: MetricWarningLevel::Caution,
            message: "On these assumptions your projected balance goes negative within the horizon — treat this as a plan that needs adjusting, not a safe path.".into(),
        });
    }

    MetricExplanation {
        key: "scenario".into(),
        label: label.to_string(),
        // Headline is the change to monthly cash flow; the fuller consequence
        // story lives in the tradeoffs.
        value: MetricValue::Money { cents: projection.monthly_impact_cents },
        definition: format!(
            "How \u{201c}{}\u{201d} changes your finances, projected over {} months against your current balance, income, and spending.",
            label.trim(),
            horizon_months.max(1),
        ),
        inputs,
        exclusions: Vec::new(),
        assumptions,
        // The engine's own narrative of consequences vs. your current plan — the
        // same lines the Scenarios screen shows — are exactly the tradeoffs.
        tradeoffs: projection.considerations.clone(),
        period: "Projected from today".into(),
        warnings,
    }
}

/// Explain a *legacy* saved scenario — one stored before params+baseline were
/// captured (pre-V055), so it can be shown but never recomputed. No projection
/// exists to describe, so the value is withheld and the reason stated rather
/// than fabricating a breakdown.
pub fn legacy_scenario_explanation(label: &str) -> MetricExplanation {
    MetricExplanation {
        key: "scenario".into(),
        label: label.to_string(),
        value: MetricValue::Withheld,
        definition: "A scenario you saved earlier. Its result was stored, but the assumptions behind it weren't, so it can't be broken down against today's finances.".into(),
        inputs: Vec::new(),
        exclusions: Vec::new(),
        assumptions: Vec::new(),
        tradeoffs: Vec::new(),
        period: "Saved earlier".into(),
        warnings: vec![MetricWarning {
            level: MetricWarningLevel::Withheld,
            message: "Legacy scenario: recorded before its assumptions were saved, so there's nothing to recompute or explain. Re-create it to get a full breakdown.".into(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::CurrencyHolding;

    fn fresh_db() -> (tempfile::TempDir, crate::Db) {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("provenance.sqlcipher"), &key).unwrap();
        crate::db::run_migrations(&db).unwrap();
        (dir, db)
    }

    /// The DB entry point runs end-to-end on a brand-new empty database, returns
    /// the full metric set, and withholds the safety figures instead of
    /// fabricating them from no history.
    #[test]
    fn empty_db_runs_and_withholds_safety() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let out = explain_financial_metrics(&mut conn, None).unwrap();
        assert_eq!(out.len(), 7);
        assert_eq!(find(&out, "runway_days").value, MetricValue::Withheld);
        assert_eq!(find(&out, "emergency_fund_months").value, MetricValue::Withheld);
    }

    /// The fetch path must read the SAME shared-metrics functions the dashboard
    /// does, so an explanation's value equals the number shown elsewhere. Seeds
    /// a realistic DB and asserts each explained value against a direct call to
    /// the metrics layer — the consistency contract, at the database level.
    #[test]
    fn db_fetch_matches_the_shared_metrics_layer() {
        let (_dir, db) = fresh_db();
        crate::sample::seed_dev_demo(&db).unwrap();
        let mut conn = db.get().unwrap();

        let balances = metrics::balance_breakdown_for(&mut conn, None).unwrap();
        let rolling = metrics::rolling_averages_for(&conn, ROLLING_WINDOW_DAYS, None).unwrap();
        let safety = metrics::safety_expense_basis(&conn).unwrap();

        let out = explain_financial_metrics(&mut conn, None).unwrap();
        assert_eq!(out.len(), 7);
        assert_eq!(find(&out, "net_worth").value, MetricValue::Money { cents: balances.net_worth_cents });
        assert_eq!(find(&out, "avg_monthly_income").value, MetricValue::Money { cents: rolling.avg_monthly_income_cents });
        assert_eq!(find(&out, "avg_monthly_expense").value, MetricValue::Money { cents: rolling.avg_monthly_expense_cents });
        assert_eq!(find(&out, "monthly_surplus").value, MetricValue::Money { cents: rolling.net_monthly_cents });
        assert_eq!(find(&out, "savings_rate").value, MetricValue::Percent { pct: rolling.savings_rate_pct });

        // The safety metrics must match get_financial_metrics' EXACT rule
        // (household scope: withhold iff the basis is insufficient; otherwise the
        // same emergency_fund_months / runway_days over the conservative basis).
        // Replicated here because get_financial_metrics lives in finsight-api;
        // this is the guard against the two withholding rules drifting apart.
        let (expected_ef, expected_runway) = if !safety.sufficient {
            (MetricValue::Withheld, MetricValue::Withheld)
        } else {
            (
                MetricValue::Months {
                    months: metrics::emergency_fund_months(balances.emergency_fund_cents, safety.monthly_expense_cents),
                },
                MetricValue::Days {
                    days: metrics::runway_days(balances.liquid_cents, safety.monthly_expense_cents),
                },
            )
        };
        assert_eq!(find(&out, "emergency_fund_months").value, expected_ef);
        assert_eq!(find(&out, "runway_days").value, expected_runway);
    }

    fn full_history() -> (BalanceBreakdown, RollingAverages, SafetyExpenseBasis, Assumptions) {
        let balances = BalanceBreakdown {
            liquid_cents: 800_000,
            invested_cents: 1_500_000,
            debt_cents: 300_000,
            emergency_fund_cents: 600_000,
            net_worth_cents: 2_000_000, // liquid + invested - debt exactly
            accounts_with_unknown_balance: 0,
            currency: Some("USD".into()),
            unconverted: Vec::new(),
        };
        let rolling = RollingAverages {
            window_days: 90,
            months: 3,
            avg_monthly_income_cents: 500_000,
            avg_monthly_expense_cents: 350_000,
            net_monthly_cents: 150_000,
            savings_rate_pct: 30,
            data_span_days: 88,
        };
        let safety = SafetyExpenseBasis {
            monthly_expense_cents: 380_000,
            sufficient: true,
            months_observed: 12,
            data_span_days: 88,
        };
        (balances, rolling, safety, Assumptions::default())
    }

    fn find<'a>(v: &'a [MetricExplanation], key: &str) -> &'a MetricExplanation {
        v.iter().find(|e| e.key == key).unwrap_or_else(|| panic!("missing metric {key}"))
    }

    /// Every explained value must equal what the intermediates carry — the
    /// consistency contract with get_financial_metrics.
    #[test]
    fn values_come_straight_from_the_intermediates() {
        let (b, r, s, a) = full_history();
        let out = assemble(&b, &r, &s, &a, None);
        assert_eq!(find(&out, "net_worth").value, MetricValue::Money { cents: 2_000_000 });
        assert_eq!(find(&out, "avg_monthly_income").value, MetricValue::Money { cents: 500_000 });
        assert_eq!(find(&out, "avg_monthly_expense").value, MetricValue::Money { cents: 350_000 });
        assert_eq!(find(&out, "monthly_surplus").value, MetricValue::Money { cents: 150_000 });
        assert_eq!(find(&out, "savings_rate").value, MetricValue::Percent { pct: 30 });
        assert_eq!(
            find(&out, "emergency_fund_months").value,
            MetricValue::Months { months: metrics::emergency_fund_months(600_000, 380_000) }
        );
        assert_eq!(
            find(&out, "runway_days").value,
            MetricValue::Days { days: metrics::runway_days(800_000, 380_000) }
        );
    }

    /// Net-worth inputs must always sum to the value shown, including a residual
    /// for manual assets not captured by liquid/invested/debt.
    #[test]
    fn net_worth_inputs_sum_to_value_with_residual() {
        let (mut b, r, s, a) = full_history();
        b.net_worth_cents = 2_250_000; // 250k of manual assets beyond the three buckets
        let out = assemble(&b, &r, &s, &a, None);
        let nw = find(&out, "net_worth");
        let sum: i64 = nw.inputs.iter().filter_map(|i| i.amount_cents).sum();
        assert_eq!(sum, 2_250_000, "inputs must reconcile to the displayed net worth");
        assert!(nw.inputs.iter().any(|i| i.label.contains("Manual assets")));
    }

    /// (a) Fresh user, no history: safety metrics withheld, thin-history
    /// warnings fire, nothing crashes, no fabricated numbers.
    #[test]
    fn fresh_user_withholds_safety_and_warns() {
        let balances = BalanceBreakdown::default();
        let rolling = RollingAverages { window_days: 90, months: 1, data_span_days: 0, ..Default::default() };
        let safety = SafetyExpenseBasis { monthly_expense_cents: 0, sufficient: false, months_observed: 0, data_span_days: 0 };
        let out = assemble(&balances, &rolling, &safety, &Assumptions::default(), None);

        assert_eq!(find(&out, "emergency_fund_months").value, MetricValue::Withheld);
        assert_eq!(find(&out, "runway_days").value, MetricValue::Withheld);
        assert!(find(&out, "emergency_fund_months").warnings.iter().any(|w| w.level == MetricWarningLevel::Withheld));
        // Descriptive figures are honest zeros, flagged as no-data, not silent.
        assert_eq!(find(&out, "avg_monthly_income").value, MetricValue::Money { cents: 0 });
        assert!(find(&out, "avg_monthly_income").warnings.iter().any(|w| w.level == MetricWarningLevel::Caution));
    }

    /// (b) Unknown-balance accounts must be disclosed as an exclusion on every
    /// balance-derived metric, never silently dropped.
    #[test]
    fn unknown_balances_are_disclosed() {
        let (mut b, r, s, a) = full_history();
        b.accounts_with_unknown_balance = 2;
        let out = assemble(&b, &r, &s, &a, None);
        let nw = find(&out, "net_worth");
        assert!(nw.exclusions.iter().any(|e| e.contains("2 accounts") && e.contains("confirmed balance")));
        assert!(nw.warnings.iter().any(|w| w.level == MetricWarningLevel::Caution));
    }

    /// (c) Money in other currencies must be disclosed, never converted or hidden.
    #[test]
    fn mixed_currency_is_disclosed() {
        let (mut b, r, s, a) = full_history();
        b.unconverted = vec![
            CurrencyHolding { code: "EUR".into(), account_count: 1, balance_cents: 120_000 },
            CurrencyHolding { code: "GBP".into(), account_count: 1, balance_cents: 90_000 },
        ];
        let out = assemble(&b, &r, &s, &a, None);
        let nw = find(&out, "net_worth");
        assert!(nw.warnings.iter().any(|w| w.message.contains("EUR") && w.message.contains("GBP")));
        // Cashflow figures scope to the primary currency and say so.
        assert!(find(&out, "avg_monthly_expense").exclusions.iter().any(|e| e.contains("other currencies")));
    }

    /// (d) A full clean single-currency history produces stated figures with no
    /// spurious caution/withheld warnings.
    #[test]
    fn clean_full_history_has_no_spurious_warnings() {
        let (b, r, s, a) = full_history();
        let out = assemble(&b, &r, &s, &a, None);
        for e in &out {
            assert!(
                !e.warnings.iter().any(|w| matches!(w.level, MetricWarningLevel::Withheld | MetricWarningLevel::Caution)),
                "metric {} raised an unexpected warning: {:?}", e.key, e.warnings
            );
            assert_ne!(e.value, MetricValue::Withheld, "metric {} should have a value", e.key);
        }
    }

    /// Member scope withholds household-only safety metrics but still explains
    /// the descriptive ones.
    #[test]
    fn member_scope_withholds_only_safety_metrics() {
        let (b, r, s, a) = full_history();
        let out = assemble(&b, &r, &s, &a, Some("member-1"));
        assert_eq!(find(&out, "emergency_fund_months").value, MetricValue::Withheld);
        assert_eq!(find(&out, "runway_days").value, MetricValue::Withheld);
        assert_eq!(find(&out, "savings_rate").value, MetricValue::Percent { pct: 30 });
        assert_eq!(find(&out, "net_worth").value, MetricValue::Money { cents: 2_000_000 });
    }

    // ── Scenario explanation (issue #71) ────────────────────────────────────

    fn scen_snap() -> crate::forecast::Snapshot {
        crate::forecast::Snapshot {
            balance_cents: 2_000_000,
            avg_monthly_income_cents: 600_000,
            avg_monthly_expense_cents: 400_000,
            goals: vec![crate::forecast::GoalInfo {
                name: "House".into(),
                remaining_cents: 1_200_000,
                monthly_cents: 100_000,
            }],
        }
    }

    /// A scenario explanation describes the very projection it was handed: the
    /// headline is that projection's monthly impact, the baseline is the inputs,
    /// only the levers this scenario sets are listed, and the engine's own
    /// considerations become the tradeoffs verbatim (so it can't disagree with
    /// the scenario card).
    #[test]
    fn scenario_explanation_describes_the_projection_it_ran() {
        let snap = scen_snap();
        let params = crate::forecast::ScenarioParams {
            monthly_expense_delta_cents: 50_000,
            one_time_cents: 300_000,
            ..Default::default()
        };
        let proj = crate::forecast::project(&snap, &params, 12);
        let ex = scenario_explanation("Adopt a dog", &params, &snap, &proj, false, 12);

        assert_eq!(ex.value, MetricValue::Money { cents: proj.monthly_impact_cents });
        assert!(ex.inputs.iter().any(|i| i.label == "Starting balance" && i.amount_cents == Some(2_000_000)));
        assert!(ex.inputs.iter().any(|i| i.label == "Monthly spending change"));
        assert!(ex.inputs.iter().any(|i| i.label == "One-time amount"));
        // A lever this scenario leaves untouched is not listed.
        assert!(!ex.inputs.iter().any(|i| i.label == "Income change"));
        // Consistency: the tradeoffs ARE the engine's considerations.
        assert_eq!(ex.tradeoffs, proj.considerations);
        assert!(ex.assumptions.iter().any(|a| a.label == "Projection horizon"));
        // A clean, non-stale, positive scenario raises no caution.
        assert!(!ex.warnings.iter().any(|w| w.level == MetricWarningLevel::Caution));
    }

    /// Staleness and a balance-goes-negative verdict each surface as a caution,
    /// so the explanation never presents a risky or outdated result as safe.
    #[test]
    fn scenario_explanation_flags_stale_and_negative_verdict() {
        let mut snap = scen_snap();
        snap.balance_cents = 100_000; // a one-time far larger than the balance
        let params = crate::forecast::ScenarioParams { one_time_cents: 3_500_000, ..Default::default() };
        let proj = crate::forecast::project(&snap, &params, 12);
        assert!(!proj.verdict, "this scenario should drive the balance negative");
        let ex = scenario_explanation("Buy a car", &params, &snap, &proj, true, 12);
        let cautions = ex.warnings.iter().filter(|w| w.level == MetricWarningLevel::Caution).count();
        assert!(cautions >= 2, "expected stale + negative-verdict cautions, got {cautions}");
    }

    /// A legacy row (no saved assumptions) withholds its value and says why,
    /// rather than fabricating a breakdown.
    #[test]
    fn legacy_scenario_is_withheld_with_reason() {
        let ex = legacy_scenario_explanation("Old scenario");
        assert_eq!(ex.value, MetricValue::Withheld);
        assert!(ex.warnings.iter().any(|w| w.level == MetricWarningLevel::Withheld));
        assert!(ex.tradeoffs.is_empty() && ex.inputs.is_empty());
    }
}

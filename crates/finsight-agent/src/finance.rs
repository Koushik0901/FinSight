use chrono::{Datelike, Duration, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const HIGH_INTEREST_APR: f64 = 8.0;
const STARTER_EMERGENCY_CENTS: i64 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinanceQuestionKind {
    CashInflow,
    GoalEta,
    DebtVsGoal,
    DebtRanking,
    Snapshot,
    GeneralPlanning,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct FinanceQuestionProfile {
    pub kind: FinanceQuestionKind,
    pub amount_cents: Option<i64>,
    pub cadence: Option<String>,
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinancialSnapshot {
    pub liquid_balance_cents: i64,
    pub total_account_balance_cents: i64,
    pub avg_monthly_income_90d_cents: i64,
    pub avg_monthly_expense_90d_cents: i64,
    pub avg_monthly_income_12m_cents: i64,
    pub avg_monthly_expense_12m_cents: i64,
    /// Median of the last up-to-12 COMPLETE months of expense — a one-off-proof
    /// "typical month" used as the basis for monthly-surplus projections, so a
    /// single large purchase in the trailing 90 days doesn't crush the surplus.
    #[serde(default)]
    pub typical_monthly_expense_cents: i64,
    pub emergency_fund_months: f64,
    pub emergency_fund_balance_cents: i64,
    pub paycheck_cadence: Option<String>,
    pub expected_paycheck_cents: Option<i64>,
    pub accounts: Vec<SnapshotAccount>,
    pub goals: Vec<SnapshotGoal>,
    pub liabilities: Vec<SnapshotLiability>,
    pub recurring_bills: Vec<SnapshotRecurringBill>,
    pub planned_transactions: Vec<SnapshotPlannedTransaction>,
    pub data_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotGoal {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub current_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub remaining_cents: i64,
    pub eta_months: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotAccount {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub balance_cents: i64,
    /// False when the account has no balance snapshot at all (e.g. an unsynced
    /// brokerage). In that case `balance_cents` is a placeholder 0 that must
    /// NOT be reported as a real $0 balance — the balance is genuinely unknown.
    #[serde(default = "default_true")]
    pub balance_known: bool,
    pub liquidity_type: String,
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotLiability {
    pub id: String,
    pub name: String,
    pub liability_type: String,
    pub balance_cents: i64,
    pub limit_cents: Option<i64>,
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub payoff_date: Option<String>,
    pub original_balance_cents: Option<i64>,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecurringBill {
    pub merchant: String,
    pub amount_cents: i64,
    pub next_expected: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPlannedTransaction {
    pub id: String,
    pub description: String,
    pub amount_cents: i64,
    pub due_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashInflowAdvice {
    pub amount_cents: i64,
    pub allocations: Vec<Allocation>,
    pub rationale: Vec<String>,
    pub missing_data: Vec<String>,
    pub investing_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    pub bucket: String,
    pub target_id: Option<String>,
    pub amount_cents: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalEtaResult {
    pub goal_id: String,
    pub goal_name: String,
    pub contribution_cents: i64,
    pub cadence: String,
    pub monthly_equivalent_cents: i64,
    pub remaining_cents: i64,
    pub eta_months: Option<i64>,
    pub eta_pay_periods: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtPayoffRanking {
    pub method: String,
    pub items: Vec<DebtRankItem>,
    pub missing_data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtRankItem {
    pub liability_id: String,
    pub name: String,
    pub balance_cents: i64,
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub rank: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtGoalComparison {
    pub goal_id: String,
    pub goal_name: String,
    pub debt_name: Option<String>,
    pub goal_current_cents: i64,
    pub compared_debt_cents: i64,
    pub highest_apr_pct: Option<f64>,
    pub recommendation: String,
    pub suggested_goal_drawdown_cents: i64,
    pub suggested_paycheck_debt_cents: i64,
    pub emergency_fund_months_after_drawdown: f64,
    pub payoff_months_current: Option<i64>,
    pub payoff_months_after_drawdown: Option<i64>,
    pub payoff_months_with_redirect: Option<i64>,
    pub estimated_interest_saved_cents: Option<i64>,
    pub alternatives: Vec<ScenarioAlternative>,
    pub missing_data: Vec<String>,
    pub rationale: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioAlternative {
    pub name: String,
    pub action: String,
    pub cash_used_cents: i64,
    pub monthly_debt_payment_cents: Option<i64>,
    pub projected_debt_balance_cents: i64,
    pub emergency_fund_months: f64,
    pub payoff_months: Option<i64>,
    pub interest_cents: Option<i64>,
    pub tradeoff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtPayoffScenarios {
    pub method: String,
    pub extra_monthly_payment_cents: i64,
    pub total_balance_cents: i64,
    pub total_minimum_payment_cents: i64,
    pub payoff_months_minimums_only: Option<i64>,
    pub payoff_months_with_extra: Option<i64>,
    pub estimated_interest_minimums_only_cents: Option<i64>,
    pub estimated_interest_with_extra_cents: Option<i64>,
    pub estimated_interest_saved_cents: Option<i64>,
    pub months_saved: Option<i64>,
    pub payoff_order: Vec<DebtRankItem>,
    pub missing_data: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAllocationScenarios {
    pub monthly_available_cents: i64,
    pub strategy: String,
    pub allocations: Vec<GoalAllocationItem>,
    pub unallocated_cents: i64,
    pub missing_data: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAllocationItem {
    pub goal_id: String,
    pub goal_name: String,
    pub target_cents: i64,
    pub current_cents: i64,
    pub remaining_cents: i64,
    pub suggested_monthly_cents: i64,
    pub eta_months: Option<i64>,
    pub target_date: Option<String>,
    pub deadline_gap_months: Option<i64>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyFundScenarios {
    pub liquid_balance_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub current_months: f64,
    /// Current monthly surplus (90-day avg income − expenses). Used as the
    /// default savings rate toward the fund when the caller supplies none.
    pub monthly_surplus_cents: i64,
    /// The contribution actually used to project completion: the caller's
    /// amount if positive, else the monthly surplus (when positive).
    pub effective_monthly_contribution_cents: i64,
    pub targets: Vec<EmergencyFundTarget>,
    pub runway_if_income_lost_months: f64,
    pub missing_data: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyFundTarget {
    pub target_months: i64,
    pub target_cents: i64,
    pub gap_cents: i64,
    pub months_to_target_at_contribution: Option<i64>,
    /// Estimated calendar date (YYYY-MM-DD) the target is reached at the
    /// effective contribution. `None` when already funded's opposite: never
    /// reached because there is no positive contribution.
    pub estimated_completion_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashflowTimeline {
    pub starting_liquid_cents: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub months: Vec<CashflowTimelineMonth>,
    pub low_balance_warnings: Vec<String>,
    pub missing_data: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashflowTimelineMonth {
    pub month_index: i64,
    pub starting_balance_cents: i64,
    pub expected_income_cents: i64,
    pub expected_expense_cents: i64,
    pub planned_net_cents: i64,
    pub ending_balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalConflictScenario {
    pub goal_id: String,
    pub goal_name: String,
    pub requested_contribution_cents: i64,
    pub upcoming_obligations_cents: i64,
    pub emergency_floor_cents: i64,
    pub starting_emergency_fund_cents: i64,
    pub emergency_fund_after_full_contribution_cents: i64,
    pub monthly_surplus_cents: i64,
    pub safe_contribution_now_cents: i64,
    pub conflicts_with_cashflow: bool,
    pub recommendation: String,
    pub alternatives: Vec<GoalConflictAlternative>,
    pub missing_data: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalConflictAlternative {
    pub name: String,
    pub action: String,
    pub goal_contribution_cents: i64,
    pub cash_after_obligations_cents: i64,
    pub tradeoff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseAffordabilityScenario {
    pub purchase_amount_cents: i64,
    pub recommendation: String,
    pub affordable_now: bool,
    pub starting_emergency_fund_cents: i64,
    pub emergency_floor_cents: i64,
    pub emergency_fund_after_purchase_cents: i64,
    pub emergency_months_after_purchase: f64,
    pub monthly_surplus_cents: i64,
    pub months_to_save_without_touching_emergency_floor: Option<i64>,
    pub high_interest_debt_cents: i64,
    pub alternatives: Vec<PurchaseAlternative>,
    pub missing_data: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseAlternative {
    pub name: String,
    pub action: String,
    pub cash_used_cents: i64,
    pub emergency_fund_after_cents: i64,
    pub emergency_months_after: f64,
    pub months_until_purchase: Option<i64>,
    pub tradeoff: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityReport {
    pub warnings: Vec<String>,
    pub missing_apr_count: i64,
    pub missing_min_payment_count: i64,
    pub uncategorized_expense_count: i64,
    pub active_goal_count: i64,
    pub active_liability_count: i64,
    pub planned_transaction_count: i64,
    pub data_sources: Vec<String>,
}

#[derive(Debug, Clone)]
struct SimDebt {
    id: String,
    name: String,
    balance_cents: f64,
    apr_pct: f64,
    min_payment_cents: i64,
}

#[derive(Debug, Clone, Copy)]
struct PayoffProjection {
    months: i64,
    interest_cents: i64,
}

pub fn infer_question_profile(question: &str) -> FinanceQuestionProfile {
    let lower = question.to_lowercase();
    let kind = if contains_any(
        &lower,
        &[
            "biweekly",
            "bi-weekly",
            "semimonthly",
            "semi-monthly",
            "twice a month",
            "twice monthly",
            "weekly",
            "monthly",
            "how soon",
            "eta",
            "reach my goal",
        ],
    ) && contains_any(&lower, &["goal", "save", "saving", "car", "house", "trip"])
    {
        FinanceQuestionKind::GoalEta
    } else if contains_any(
        &lower,
        &[
            "paycheck",
            "pay of around",
            "pay of about",
            "windfall",
            "bonus",
            "got paid",
            "what should i do with",
        ],
    ) {
        FinanceQuestionKind::CashInflow
    } else if contains_any(
        &lower,
        &[
            "vs",
            "versus",
            "should i use",
            "raid",
            "take money from savings",
            "car savings",
            "debt",
        ],
    ) {
        FinanceQuestionKind::DebtVsGoal
    } else if contains_any(
        &lower,
        &[
            "snowball",
            "avalanche",
            "pay off first",
            "rank",
            "order my debts",
        ],
    ) {
        FinanceQuestionKind::DebtRanking
    } else if contains_any(
        &lower,
        &[
            "snapshot",
            "overview",
            "summary",
            "how am i doing",
            "financial picture",
        ],
    ) {
        FinanceQuestionKind::Snapshot
    } else if contains_any(
        &lower,
        &[
            "invest",
            "etf",
            "stocks",
            "emergency fund",
            "debt",
            "budget",
            "goal",
        ],
    ) {
        FinanceQuestionKind::GeneralPlanning
    } else {
        FinanceQuestionKind::Unknown
    };

    FinanceQuestionProfile {
        kind,
        amount_cents: parse_amount_cents(question),
        cadence: infer_cadence(&lower).map(ToString::to_string),
        method: if lower.contains("snowball") {
            Some("snowball".to_string())
        } else if lower.contains("avalanche") {
            Some("avalanche".to_string())
        } else {
            None
        },
    }
}

pub fn parse_amount_cents(question: &str) -> Option<i64> {
    let bytes = question.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let mut j = i + 1;
            let mut seen_digit = false;
            while j < bytes.len() {
                let ch = bytes[j] as char;
                if ch.is_ascii_digit() || ch == ',' || ch == '.' {
                    seen_digit = seen_digit || ch.is_ascii_digit();
                    j += 1;
                } else {
                    break;
                }
            }
            if seen_digit {
                let raw = question[i + 1..j].replace(',', "");
                if let Ok(value) = raw.parse::<f64>() {
                    return Some((value * 100.0).round() as i64);
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    None
}

pub fn infer_cadence(question: &str) -> Option<&'static str> {
    let lower = question.to_lowercase();
    if lower.contains("semimonthly")
        || lower.contains("semi-monthly")
        || lower.contains("twice a month")
        || lower.contains("twice monthly")
    {
        Some("semimonthly")
    } else if lower.contains("biweekly") || lower.contains("bi-weekly") {
        Some("biweekly")
    } else if lower.contains("weekly") {
        Some("weekly")
    } else if lower.contains("monthly") {
        Some("monthly")
    } else {
        None
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

/// Median of the last up-to-12 COMPLETE calendar months of expense (transfers
/// excluded). Robust to a single one-off purchase that inflates a 90-day
/// average. Returns 0 when there is less than 3 months of completed history, so
/// callers fall back to the 90-day average.
fn robust_monthly_expense_cents(conn: &Connection) -> rusqlite::Result<i64> {
    let this_month = chrono::Utc::now().format("%Y-%m").to_string();
    // A `settle_up = 1` row nets as `-amount_cents`, matching metrics.rs
    // cashflow, instead of being silently dropped by an `amount_cents < 0`-only
    // CASE.
    let mut stmt = conn.prepare(
        "SELECT SUM(CASE WHEN settle_up = 1 THEN -amount_cents \
                          WHEN amount_cents < 0 THEN -amount_cents \
                          ELSE 0 END) AS spent \
         FROM transactions \
         WHERE is_transfer = 0 AND substr(posted_at,1,7) < ?1 \
         GROUP BY substr(posted_at,1,7) \
         ORDER BY substr(posted_at,1,7) DESC LIMIT 12",
    )?;
    let mut vals: Vec<i64> = stmt
        .query_map(rusqlite::params![this_month], |r| r.get::<_, i64>(0))?
        .filter_map(|r| r.ok())
        .collect();
    if vals.len() < 3 {
        return Ok(0);
    }
    vals.sort_unstable();
    let mid = vals.len() / 2;
    Ok(if vals.len() % 2 == 0 {
        (vals[mid - 1] + vals[mid]) / 2
    } else {
        vals[mid]
    })
}

pub fn build_snapshot(conn: &mut Connection) -> rusqlite::Result<FinancialSnapshot> {
    let now = Utc::now();
    let cut90 = (now - Duration::days(90)).to_rfc3339();
    let cut365 = (now - Duration::days(365)).to_rfc3339();

    let accounts = accounts(conn)?;
    let total_account_balance_cents: i64 = accounts.iter().map(|a| a.balance_cents).sum();
    // Debt (Credit/Loan accounts) is never a liquid asset or emergency-fund
    // balance, regardless of its liquidity_type tag — these used to live in a
    // separate liabilities table that this sum never saw; now that debt is an
    // Account too, it must be excluded explicitly rather than relying on
    // liquidity_type alone.
    let is_debt = |a: &&SnapshotAccount| a.account_type == "Credit" || a.account_type == "Loan";
    let liquid_balance_cents: i64 = accounts
        .iter()
        .filter(|a| a.liquidity_type != "illiquid" && !is_debt(a))
        .map(|a| a.balance_cents)
        .sum();
    let emergency_fund_balance_cents: i64 = accounts
        .iter()
        .filter(|a| a.emergency_fund_eligible && a.liquidity_type != "illiquid" && !is_debt(a))
        .map(|a| a.balance_cents)
        .sum();
    let (income90, expense90) = income_expense_since(conn, &cut90)?;
    let (income365, expense365) = income_expense_since(conn, &cut365)?;
    let avg_monthly_income_90d_cents = income90 / 3;
    let avg_monthly_expense_90d_cents = expense90 / 3;
    let avg_monthly_income_12m_cents = income365 / 12;
    let avg_monthly_expense_12m_cents = expense365 / 12;
    // One-off-proof "typical month" for surplus projections; fall back to the
    // 90-day average when there isn't enough completed history to take a median.
    let typical_monthly_expense_cents = {
        let median = robust_monthly_expense_cents(conn)?;
        if median > 0 {
            median
        } else {
            avg_monthly_expense_90d_cents
        }
    };
    // ONE emergency-fund-months definition (EF-eligible balance ÷ avg expense,
    // capped) from the shared metrics layer, so the snapshot, the drawdown
    // scenarios below, and every screen report the same number.
    let emergency_fund_months = finsight_core::metrics::emergency_fund_months(
        emergency_fund_balance_cents,
        avg_monthly_expense_90d_cents,
    );

    let paycheck_cadence = setting_string(conn, "planning.paycheck_cadence")?;
    let expected_paycheck_cents = setting_i64(conn, "planning.expected_paycheck_cents")?;
    let goals = goals(conn)?;
    let liabilities = liabilities(conn)?;
    let recurring_bills = recurring_bills(conn)?;
    let planned_transactions = planned_transactions(conn)?;
    let uncategorized_count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM transactions t \
             WHERE category_id IS NULL AND amount_cents < 0 AND is_transfer = 0 AND {}",
            finsight_core::metrics::non_investment_txn_predicate("t")
        ),
        [],
        |r| r.get(0),
    )?;

    let mut data_warnings = Vec::new();
    if uncategorized_count > 0 {
        data_warnings.push(format!("{uncategorized_count} expense transaction(s) are uncategorized, so spending analysis may be incomplete."));
    }
    for a in &accounts {
        if !a.balance_known {
            data_warnings.push(format!(
                "{} has no balance snapshot; its balance is UNKNOWN (not $0) and is excluded from totals.",
                a.name
            ));
        }
    }
    for l in &liabilities {
        if l.apr_pct.is_none() {
            data_warnings.push(format!(
                "{} is missing APR; debt priority is provisional.",
                l.name
            ));
        }
        if l.min_payment_cents.is_none() {
            data_warnings.push(format!(
                "{} is missing minimum payment; payoff timing is provisional.",
                l.name
            ));
        }
    }

    Ok(FinancialSnapshot {
        liquid_balance_cents,
        total_account_balance_cents,
        avg_monthly_income_90d_cents,
        avg_monthly_expense_90d_cents,
        avg_monthly_income_12m_cents,
        avg_monthly_expense_12m_cents,
        typical_monthly_expense_cents,
        emergency_fund_months,
        emergency_fund_balance_cents,
        paycheck_cadence,
        expected_paycheck_cents,
        accounts,
        goals,
        liabilities,
        recurring_bills,
        planned_transactions,
        data_warnings,
    })
}

pub fn analyze_cash_inflow(
    conn: &mut Connection,
    amount_cents: i64,
) -> rusqlite::Result<CashInflowAdvice> {
    let snapshot = build_snapshot(conn)?;
    let mut remaining = amount_cents.max(0);
    let mut allocations = Vec::new();
    let mut rationale = Vec::new();
    let mut missing_data = snapshot.data_warnings.clone();

    let emergency_target = if snapshot.avg_monthly_expense_90d_cents > 0 {
        snapshot.avg_monthly_expense_90d_cents
    } else {
        STARTER_EMERGENCY_CENTS
    };
    let emergency_gap = (emergency_target - snapshot.liquid_balance_cents).max(0);
    if remaining > 0 && emergency_gap > 0 {
        let amount = remaining.min(emergency_gap);
        allocations.push(Allocation {
            bucket: "starter_emergency_fund".to_string(),
            target_id: None,
            amount_cents: amount,
            reason: "Bring liquid savings closer to one month of expenses before investing or extra goals.".to_string(),
        });
        rationale
            .push("Emergency fund coverage below one month is the first priority.".to_string());
        remaining -= amount;
    }

    let mut debts = snapshot.liabilities.clone();
    debts.retain(|d| d.balance_cents > 0);
    debts.sort_by(|a, b| {
        b.apr_pct
            .partial_cmp(&a.apr_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for debt in debts
        .iter()
        .filter(|d| d.apr_pct.unwrap_or(0.0) >= HIGH_INTEREST_APR)
    {
        if remaining <= 0 {
            break;
        }
        let amount = remaining.min(debt.balance_cents);
        allocations.push(Allocation {
            bucket: "high_interest_debt".to_string(),
            target_id: Some(debt.id.clone()),
            amount_cents: amount,
            reason: format!(
                "{} has {}% APR, which takes priority over car savings and investing.",
                debt.name,
                debt.apr_pct.unwrap_or(0.0)
            ),
        });
        rationale
            .push("High-interest debt is treated as a guaranteed negative return.".to_string());
        remaining -= amount;
    }

    if remaining > 0 {
        if let Some(goal) = snapshot
            .goals
            .iter()
            .find(|g| g.name.to_lowercase().contains("car") && g.remaining_cents > 0)
        {
            let amount = remaining.min(goal.remaining_cents);
            allocations.push(Allocation {
                bucket: "goal_savings".to_string(),
                target_id: Some(goal.id.clone()),
                amount_cents: amount,
                reason: format!(
                    "Fund {} after emergency and high-interest debt priorities.",
                    goal.name
                ),
            });
            remaining -= amount;
        }
    }

    if remaining > 0 {
        allocations.push(Allocation {
            bucket: "extra_debt_or_savings".to_string(),
            target_id: None,
            amount_cents: remaining,
            reason: "Use this for the next debt in priority order or increase cash reserves; do not invest until emergency/debt checks pass.".to_string(),
        });
    }

    if snapshot
        .liabilities
        .iter()
        .any(|l| l.balance_cents > 0 && l.apr_pct.is_none())
    {
        missing_data.push("Add APRs before finalizing exact debt allocation.".to_string());
    }
    let investing_allowed = snapshot.emergency_fund_months >= 1.0
        && !snapshot
            .liabilities
            .iter()
            .any(|l| l.balance_cents > 0 && l.apr_pct.unwrap_or(0.0) >= HIGH_INTEREST_APR);
    if !investing_allowed {
        rationale.push("Investing is not recommended yet; answer should stay principles-only and focus on debt/liquidity.".to_string());
    }

    Ok(CashInflowAdvice {
        amount_cents,
        allocations,
        rationale,
        missing_data,
        investing_allowed,
    })
}

pub fn calculate_goal_eta(
    conn: &mut Connection,
    goal_id: &str,
    contribution_cents: i64,
    cadence: &str,
) -> rusqlite::Result<GoalEtaResult> {
    let goal = goal_by_id(conn, goal_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let periods_per_year = match cadence {
        "weekly" => 52,
        "biweekly" | "bi-weekly" => 26,
        "semimonthly" | "semi-monthly" => 24,
        "monthly" => 12,
        _ => 12,
    };
    let monthly_equivalent_cents =
        ((contribution_cents.max(0) as f64) * periods_per_year as f64 / 12.0).round() as i64;
    let eta_months = if monthly_equivalent_cents > 0 && goal.remaining_cents > 0 {
        Some(div_ceil(goal.remaining_cents, monthly_equivalent_cents))
    } else if goal.remaining_cents == 0 {
        Some(0)
    } else {
        None
    };
    let eta_pay_periods = if contribution_cents > 0 && goal.remaining_cents > 0 {
        Some(div_ceil(goal.remaining_cents, contribution_cents))
    } else if goal.remaining_cents == 0 {
        Some(0)
    } else {
        None
    };
    Ok(GoalEtaResult {
        goal_id: goal.id,
        goal_name: goal.name,
        contribution_cents,
        cadence: cadence.to_string(),
        monthly_equivalent_cents,
        remaining_cents: goal.remaining_cents,
        eta_months,
        eta_pay_periods,
    })
}

pub fn rank_debt_payoff(
    conn: &mut Connection,
    method: &str,
) -> rusqlite::Result<DebtPayoffRanking> {
    let mut debts = liabilities(conn)?;
    debts.retain(|d| d.balance_cents > 0);
    let mut missing_data = Vec::new();
    for d in &debts {
        if d.apr_pct.is_none() {
            missing_data.push(format!("{} is missing APR.", d.name));
        }
        if d.min_payment_cents.is_none() {
            missing_data.push(format!("{} is missing minimum payment.", d.name));
        }
    }
    if method == "snowball" {
        debts.sort_by_key(|d| d.balance_cents);
    } else {
        debts.sort_by(|a, b| {
            b.apr_pct
                .partial_cmp(&a.apr_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.balance_cents.cmp(&b.balance_cents))
        });
    }
    let items = debts
        .into_iter()
        .enumerate()
        .map(|(idx, d)| {
            let reason = if method == "snowball" {
                "Smallest remaining balance first for behavioral momentum.".to_string()
            } else if let Some(apr) = d.apr_pct {
                format!("Highest APR first avoids the most interest; APR is {apr}%.")
            } else {
                "APR missing; rank is provisional and should be confirmed.".to_string()
            };
            DebtRankItem {
                liability_id: d.id,
                name: d.name,
                balance_cents: d.balance_cents,
                apr_pct: d.apr_pct,
                min_payment_cents: d.min_payment_cents,
                rank: idx as i64 + 1,
                reason,
            }
        })
        .collect();
    Ok(DebtPayoffRanking {
        method: method.to_string(),
        items,
        missing_data,
    })
}

pub fn compare_debt_vs_goal(
    conn: &mut Connection,
    goal_id: &str,
    liability_id: Option<&str>,
) -> rusqlite::Result<DebtGoalComparison> {
    let snapshot = build_snapshot(conn)?;
    let goal = snapshot
        .goals
        .iter()
        .find(|g| g.id == goal_id)
        .cloned()
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let debts: Vec<_> = snapshot
        .liabilities
        .iter()
        .filter(|d| d.balance_cents > 0 && liability_id.map(|id| id == d.id).unwrap_or(true))
        .cloned()
        .collect();
    let compared_debt_cents: i64 = debts.iter().map(|d| d.balance_cents).sum();
    let debt_name = if debts.len() == 1 {
        Some(debts[0].name.clone())
    } else if debts.len() > 1 {
        Some(format!("{} debts", debts.len()))
    } else {
        None
    };
    let highest_apr_pct = debts
        .iter()
        .filter_map(|d| d.apr_pct)
        .fold(None, |acc: Option<f64>, apr| {
            Some(acc.map_or(apr, |v| v.max(apr)))
        });
    let mut missing_data = snapshot.data_warnings.clone();
    if debts.iter().any(|d| d.apr_pct.is_none()) {
        missing_data.push(
            "At least one compared debt is missing APR; recommendation is provisional.".to_string(),
        );
    }
    if debts.iter().any(|d| d.min_payment_cents.is_none()) {
        missing_data.push(
            "At least one compared debt is missing a minimum payment; payoff timing is provisional."
                .to_string(),
        );
    }

    let emergency_floor = snapshot
        .avg_monthly_expense_90d_cents
        .max(STARTER_EMERGENCY_CENTS);
    let max_safe_drawdown = (snapshot.liquid_balance_cents - emergency_floor).max(0);
    let suggested_goal_drawdown_cents = if highest_apr_pct.unwrap_or(0.0) >= HIGH_INTEREST_APR {
        goal.current_cents
            .min(max_safe_drawdown)
            .min(compared_debt_cents)
    } else {
        0
    };
    // Same EF-months definition as the snapshot (EF-eligible pool ÷ avg expense,
    // capped), conservatively treating the drawdown as coming out of the safety
    // net — so "months of emergency fund" means the same thing everywhere.
    let emergency_fund_months_after_drawdown = finsight_core::metrics::emergency_fund_months(
        (snapshot.emergency_fund_balance_cents - suggested_goal_drawdown_cents).max(0),
        snapshot.avg_monthly_expense_90d_cents,
    );
    let monthly_min_payment_cents: i64 = debts.iter().filter_map(|d| d.min_payment_cents).sum();
    let weighted_apr = weighted_apr(&debts);
    let payoff_current = weighted_apr
        .and_then(|apr| payoff_projection(compared_debt_cents, apr, monthly_min_payment_cents));
    let payoff_after_drawdown = weighted_apr.and_then(|apr| {
        payoff_projection(
            compared_debt_cents.saturating_sub(suggested_goal_drawdown_cents),
            apr,
            monthly_min_payment_cents,
        )
    });
    let redirected_monthly = monthly_min_payment_cents + goal.monthly_cents.max(0);
    let payoff_with_redirect = weighted_apr.and_then(|apr| {
        payoff_projection(
            compared_debt_cents.saturating_sub(suggested_goal_drawdown_cents),
            apr,
            redirected_monthly,
        )
    });
    let estimated_interest_saved_cents = match (payoff_current, payoff_with_redirect) {
        (Some(current), Some(redirected)) => {
            Some((current.interest_cents - redirected.interest_cents).max(0))
        }
        _ => None,
    };
    let alternatives = build_debt_goal_alternatives(
        &snapshot,
        &goal,
        compared_debt_cents,
        suggested_goal_drawdown_cents,
        monthly_min_payment_cents,
        payoff_current,
        payoff_after_drawdown,
        payoff_with_redirect,
    );

    let recommendation = if snapshot.emergency_fund_months < 1.0 {
        "Do not raid car savings yet; preserve liquidity and divert future paycheck surplus toward debt first.".to_string()
    } else if highest_apr_pct.unwrap_or(0.0) >= HIGH_INTEREST_APR
        && suggested_goal_drawdown_cents > 0
    {
        "Use only the portion of car savings above the emergency floor for high-interest debt, then redirect future paychecks to the remaining debt.".to_string()
    } else if highest_apr_pct.unwrap_or(0.0) >= HIGH_INTEREST_APR {
        "Prioritize future paycheck surplus toward high-interest debt before adding more to the car goal.".to_string()
    } else {
        "Keep car savings intact and direct new paycheck surplus to debt according to the payoff ranking.".to_string()
    };
    let mut rationale = vec![
        format!(
            "{} has ${:.2} earmarked and ${:.2} remaining.",
            goal.name,
            goal.current_cents as f64 / 100.0,
            goal.remaining_cents as f64 / 100.0
        ),
        format!(
            "Current emergency coverage is {:.1} month(s).",
            snapshot.emergency_fund_months
        ),
    ];
    rationale.push(format!(
        "Liquid balance is ${:.2}; emergency floor is ${:.2}.",
        snapshot.liquid_balance_cents as f64 / 100.0,
        emergency_floor as f64 / 100.0
    ));
    if let Some(apr) = highest_apr_pct {
        rationale.push(format!("Highest compared APR is {apr}%."));
    }
    if let Some(saved) = estimated_interest_saved_cents {
        rationale.push(format!(
            "Scenario math estimates about ${:.2} of interest avoided by combining safe drawdown with redirected goal contributions.",
            saved as f64 / 100.0
        ));
    }

    Ok(DebtGoalComparison {
        goal_id: goal.id,
        goal_name: goal.name,
        debt_name,
        goal_current_cents: goal.current_cents,
        compared_debt_cents,
        highest_apr_pct,
        recommendation,
        suggested_goal_drawdown_cents,
        suggested_paycheck_debt_cents: compared_debt_cents
            .saturating_sub(suggested_goal_drawdown_cents),
        emergency_fund_months_after_drawdown,
        payoff_months_current: payoff_current.map(|p| p.months),
        payoff_months_after_drawdown: payoff_after_drawdown.map(|p| p.months),
        payoff_months_with_redirect: payoff_with_redirect.map(|p| p.months),
        estimated_interest_saved_cents,
        alternatives,
        missing_data,
        rationale,
    })
}

pub fn run_debt_payoff_scenarios(
    conn: &mut Connection,
    method: &str,
    extra_monthly_payment_cents: i64,
) -> rusqlite::Result<DebtPayoffScenarios> {
    let ranking = rank_debt_payoff(conn, method)?;
    let debts = liabilities(conn)?
        .into_iter()
        .filter(|d| d.balance_cents > 0)
        .collect::<Vec<_>>();
    let total_balance_cents = debts.iter().map(|d| d.balance_cents).sum();
    let total_minimum_payment_cents = debts.iter().filter_map(|d| d.min_payment_cents).sum();
    let mut missing_data = ranking.missing_data.clone();
    let can_project = debts
        .iter()
        .all(|d| d.apr_pct.is_some() && d.min_payment_cents.unwrap_or(0) > 0);

    let (minimum, with_extra) = if can_project {
        let minimum = simulate_debt_payoff(&debts, method, 0);
        let with_extra = simulate_debt_payoff(&debts, method, extra_monthly_payment_cents.max(0));
        (minimum, with_extra)
    } else {
        missing_data.push(
            "Debt payoff scenarios need APR and minimum payment for every active liability."
                .to_string(),
        );
        (None, None)
    };

    Ok(DebtPayoffScenarios {
        method: method.to_string(),
        extra_monthly_payment_cents: extra_monthly_payment_cents.max(0),
        total_balance_cents,
        total_minimum_payment_cents,
        payoff_months_minimums_only: minimum.map(|p| p.months),
        payoff_months_with_extra: with_extra.map(|p| p.months),
        estimated_interest_minimums_only_cents: minimum.map(|p| p.interest_cents),
        estimated_interest_with_extra_cents: with_extra.map(|p| p.interest_cents),
        estimated_interest_saved_cents: match (minimum, with_extra) {
            (Some(a), Some(b)) => Some((a.interest_cents - b.interest_cents).max(0)),
            _ => None,
        },
        months_saved: match (minimum, with_extra) {
            (Some(a), Some(b)) => Some((a.months - b.months).max(0)),
            _ => None,
        },
        payoff_order: ranking.items,
        missing_data,
        assumptions: vec![
            "Minimum payments are made on every debt before extra payments are applied."
                .to_string(),
            "Freed minimum payments roll into the next debt in the selected payoff order."
                .to_string(),
        ],
    })
}

pub fn run_goal_allocation_scenarios(
    conn: &mut Connection,
    monthly_available_cents: i64,
    strategy: &str,
) -> rusqlite::Result<GoalAllocationScenarios> {
    let mut goals = goals(conn)?
        .into_iter()
        .filter(|g| g.remaining_cents > 0)
        .collect::<Vec<_>>();
    let strategy = match strategy {
        "deadline" | "proportional" | "priority" => strategy,
        _ => "priority",
    };
    match strategy {
        "deadline" => goals.sort_by(|a, b| a.target_date.cmp(&b.target_date)),
        "proportional" => goals.sort_by_key(|g| std::cmp::Reverse(g.remaining_cents)),
        _ => goals.sort_by_key(|g| (g.goal_type != "save-by-date", g.target_date.clone())),
    }

    let mut remaining_monthly = monthly_available_cents.max(0);
    let total_remaining: i64 = goals.iter().map(|g| g.remaining_cents).sum();
    let today = Utc::now().date_naive();
    let mut allocations = Vec::new();

    for (idx, goal) in goals.iter().enumerate() {
        let suggested = if strategy == "proportional" && total_remaining > 0 {
            if idx == goals.len() - 1 {
                remaining_monthly
            } else {
                let share = (monthly_available_cents.max(0) as f64 * goal.remaining_cents as f64
                    / total_remaining as f64)
                    .round() as i64;
                share.min(remaining_monthly)
            }
        } else {
            remaining_monthly.min(goal.remaining_cents)
        };
        remaining_monthly -= suggested;
        let eta_months = if suggested > 0 {
            Some(div_ceil(goal.remaining_cents, suggested))
        } else {
            None
        };
        let deadline_gap_months = match (&goal.target_date, eta_months) {
            (Some(date), Some(eta)) => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .map(|target| ((target - today).num_days() / 30) - eta),
            _ => None,
        };
        allocations.push(GoalAllocationItem {
            goal_id: goal.id.clone(),
            goal_name: goal.name.clone(),
            target_cents: goal.target_cents,
            current_cents: goal.current_cents,
            remaining_cents: goal.remaining_cents,
            suggested_monthly_cents: suggested,
            eta_months,
            target_date: goal.target_date.clone(),
            deadline_gap_months,
            rationale: match strategy {
                "deadline" => "Prioritized by nearest target date.".to_string(),
                "proportional" => "Allocated proportionally by remaining goal size.".to_string(),
                _ => "Allocated to the highest-priority unfinished goal first.".to_string(),
            },
        });
        if remaining_monthly <= 0 {
            for goal in goals.iter().skip(idx + 1) {
                allocations.push(GoalAllocationItem {
                    goal_id: goal.id.clone(),
                    goal_name: goal.name.clone(),
                    target_cents: goal.target_cents,
                    current_cents: goal.current_cents,
                    remaining_cents: goal.remaining_cents,
                    suggested_monthly_cents: 0,
                    eta_months: None,
                    target_date: goal.target_date.clone(),
                    deadline_gap_months: None,
                    rationale: "No monthly dollars remain after higher-priority goals.".to_string(),
                });
            }
            break;
        }
    }

    Ok(GoalAllocationScenarios {
        monthly_available_cents: monthly_available_cents.max(0),
        strategy: strategy.to_string(),
        allocations,
        unallocated_cents: remaining_monthly.max(0),
        missing_data: Vec::new(),
        assumptions: vec![
            "Goal balances and monthly allocations are modeled from current goal records only."
                .to_string(),
        ],
    })
}

pub fn run_emergency_fund_scenarios(
    conn: &mut Connection,
    monthly_contribution_cents: i64,
) -> rusqlite::Result<EmergencyFundScenarios> {
    let snapshot = build_snapshot(conn)?;
    let expense = snapshot.avg_monthly_expense_90d_cents.max(0);
    // Default the savings rate to the current monthly surplus so "when will my
    // emergency fund be full?" is answerable without the user quoting a number.
    // Uses the one-off-proof typical expense so a single big purchase in the
    // last 90 days doesn't zero out the projected surplus.
    let monthly_surplus_cents =
        expected_monthly_income_cents(&snapshot) - snapshot.typical_monthly_expense_cents;
    let effective_monthly_contribution_cents = if monthly_contribution_cents > 0 {
        monthly_contribution_cents
    } else {
        monthly_surplus_cents.max(0)
    };
    let today = Utc::now().date_naive();
    let targets = [1, 3, 6]
        .into_iter()
        .map(|target_months| {
            let target_cents = expense * target_months;
            let gap_cents = (target_cents - snapshot.liquid_balance_cents).max(0);
            let months_to_target = if gap_cents == 0 {
                Some(0)
            } else if effective_monthly_contribution_cents > 0 {
                Some(div_ceil(gap_cents, effective_monthly_contribution_cents))
            } else {
                None
            };
            let estimated_completion_date =
                months_to_target.map(|m| add_months(today, m).format("%Y-%m-%d").to_string());
            EmergencyFundTarget {
                target_months,
                target_cents,
                gap_cents,
                months_to_target_at_contribution: months_to_target,
                estimated_completion_date,
            }
        })
        .collect();
    let current_months = if expense > 0 {
        snapshot.liquid_balance_cents.max(0) as f64 / expense as f64
    } else {
        0.0
    };

    let mut assumptions = vec![
        "Emergency fund targets use the 90-day average monthly expense from local transactions."
            .to_string(),
    ];
    if monthly_contribution_cents <= 0 {
        assumptions.push(if monthly_surplus_cents > 0 {
            "Completion dates assume you keep saving your current monthly surplus (income minus expenses) toward the fund.".to_string()
        } else {
            "Your current monthly surplus is not positive, so no completion date can be projected until income exceeds expenses or you set a contribution.".to_string()
        });
    }

    Ok(EmergencyFundScenarios {
        liquid_balance_cents: snapshot.emergency_fund_balance_cents,
        avg_monthly_expense_cents: expense,
        current_months,
        monthly_surplus_cents,
        effective_monthly_contribution_cents,
        targets,
        runway_if_income_lost_months: current_months,
        missing_data: snapshot.data_warnings,
        assumptions,
    })
}

/// Add `months` calendar months to a date, clamping the day to the last valid
/// day of the resulting month.
fn add_months(date: chrono::NaiveDate, months: i64) -> chrono::NaiveDate {
    if months <= 0 {
        return date;
    }
    let zero_based = date.month0() as i64 + months;
    let year = date.year() + (zero_based / 12) as i32;
    let month0 = (zero_based % 12) as u32;
    let month = month0 + 1;
    // Clamp day to the last day of the target month.
    let last_day = last_day_of_month(year, month);
    let day = date.day().min(last_day);
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap_or(date)
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = chrono::NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    (first_next - chrono::Duration::days(1)).day0() + 1
}

pub fn run_cashflow_timeline(
    conn: &mut Connection,
    months: i64,
) -> rusqlite::Result<CashflowTimeline> {
    let snapshot = build_snapshot(conn)?;
    let months = months.clamp(1, 24);
    // A cashflow timeline projects SPENDABLE cash over time, so it starts from
    // the liquid balance — not the emergency-fund-eligible subset (a prior bug
    // that understated the runway for anyone with non-EF liquid accounts).
    let mut balance = snapshot.liquid_balance_cents;
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    let emergency_floor = snapshot
        .avg_monthly_expense_90d_cents
        .max(STARTER_EMERGENCY_CENTS);
    let planned_total: i64 = snapshot
        .planned_transactions
        .iter()
        .map(|p| p.amount_cents)
        .sum();
    let planned_monthly = if months > 0 {
        planned_total / months
    } else {
        0
    };

    for month_index in 1..=months {
        let starting = balance;
        let expected_income = expected_monthly_income_cents(&snapshot);
        let expected_expense = snapshot.avg_monthly_expense_90d_cents;
        let planned_net = planned_monthly;
        balance = balance + expected_income - expected_expense + planned_net;
        if balance < emergency_floor {
            warnings.push(format!(
                "Month {month_index} projected ending balance falls below the emergency floor."
            ));
        }
        out.push(CashflowTimelineMonth {
            month_index,
            starting_balance_cents: starting,
            expected_income_cents: expected_income,
            expected_expense_cents: expected_expense,
            planned_net_cents: planned_net,
            ending_balance_cents: balance,
        });
    }

    Ok(CashflowTimeline {
        starting_liquid_cents: snapshot.liquid_balance_cents,
        avg_monthly_income_cents: snapshot.avg_monthly_income_90d_cents,
        avg_monthly_expense_cents: snapshot.avg_monthly_expense_90d_cents,
        months: out,
        low_balance_warnings: warnings,
        missing_data: snapshot.data_warnings,
        assumptions: vec![
            "Timeline uses 90-day average income and expenses, plus planned transactions spread across the requested horizon."
                .to_string(),
        ],
    })
}

pub fn run_goal_conflict_scenario(
    conn: &mut Connection,
    goal_id: &str,
    requested_contribution_cents: i64,
) -> rusqlite::Result<GoalConflictScenario> {
    let snapshot = build_snapshot(conn)?;
    let goal = goal_by_id(conn, goal_id)?.unwrap_or_else(|| SnapshotGoal {
        id: goal_id.to_string(),
        name: "Selected goal".to_string(),
        goal_type: "unknown".to_string(),
        target_cents: 0,
        current_cents: 0,
        monthly_cents: 0,
        target_date: None,
        remaining_cents: 0,
        eta_months: None,
    });
    let requested_contribution_cents = requested_contribution_cents.max(0);
    let upcoming_planned_outflows: i64 = snapshot
        .planned_transactions
        .iter()
        .filter(|p| p.amount_cents < 0)
        .map(|p| p.amount_cents.abs())
        .sum();
    let upcoming_recurring_bills: i64 = snapshot
        .recurring_bills
        .iter()
        .map(|b| b.amount_cents.abs())
        .sum();
    let upcoming_obligations_cents = upcoming_planned_outflows + upcoming_recurring_bills;
    let emergency_floor_cents = snapshot
        .avg_monthly_expense_90d_cents
        .max(STARTER_EMERGENCY_CENTS);
    let starting_emergency_fund_cents = snapshot.emergency_fund_balance_cents.max(0);
    let monthly_surplus_cents =
        expected_monthly_income_cents(&snapshot) - snapshot.typical_monthly_expense_cents;
    let available_for_goal_after_floor = (starting_emergency_fund_cents
        + monthly_surplus_cents.max(0)
        - upcoming_obligations_cents
        - emergency_floor_cents)
        .max(0);
    let safe_contribution_now_cents = requested_contribution_cents
        .min(goal.remaining_cents.max(0))
        .min(available_for_goal_after_floor);
    let emergency_fund_after_full_contribution_cents =
        starting_emergency_fund_cents - requested_contribution_cents - upcoming_obligations_cents;
    let conflicts_with_cashflow = requested_contribution_cents > safe_contribution_now_cents
        || emergency_fund_after_full_contribution_cents < emergency_floor_cents
        || monthly_surplus_cents < 0;
    let recommendation = if requested_contribution_cents <= 0 {
        "I need a goal contribution amount before comparing it with upcoming bills.".to_string()
    } else if conflicts_with_cashflow {
        format!(
            "Delay or reduce the {} contribution until upcoming bills are covered and the emergency floor is protected.",
            goal.name
        )
    } else {
        format!(
            "The {} contribution appears safe after modeled upcoming bills and the emergency floor.",
            goal.name
        )
    };
    let cash_after_safe =
        starting_emergency_fund_cents - safe_contribution_now_cents - upcoming_obligations_cents;

    Ok(GoalConflictScenario {
        goal_id: goal.id.clone(),
        goal_name: goal.name.clone(),
        requested_contribution_cents,
        upcoming_obligations_cents,
        emergency_floor_cents,
        starting_emergency_fund_cents,
        emergency_fund_after_full_contribution_cents,
        monthly_surplus_cents,
        safe_contribution_now_cents,
        conflicts_with_cashflow,
        recommendation,
        alternatives: vec![
            GoalConflictAlternative {
                name: "Fund goal now".to_string(),
                action: format!(
                    "Move the full requested contribution to {} now.",
                    goal.name
                ),
                goal_contribution_cents: requested_contribution_cents,
                cash_after_obligations_cents: emergency_fund_after_full_contribution_cents,
                tradeoff: "Keeps the goal moving fastest, but can crowd out upcoming bills or emergency cash.".to_string(),
            },
            GoalConflictAlternative {
                name: "Delay until bills clear".to_string(),
                action: "Hold the contribution in cash until upcoming planned bills are paid.".to_string(),
                goal_contribution_cents: 0,
                cash_after_obligations_cents: starting_emergency_fund_cents - upcoming_obligations_cents,
                tradeoff: "Protects cashflow first, but slows goal progress this cycle.".to_string(),
            },
            GoalConflictAlternative {
                name: "Partial safe contribution".to_string(),
                action: format!(
                    "Contribute only the modeled safe amount to {} now.",
                    goal.name
                ),
                goal_contribution_cents: safe_contribution_now_cents,
                cash_after_obligations_cents: cash_after_safe,
                tradeoff: "Balances goal progress with the emergency floor and known upcoming obligations.".to_string(),
            },
        ],
        missing_data: snapshot.data_warnings,
        assumptions: vec![
            "Upcoming obligations include detected recurring bills and planned negative transactions currently stored locally.".to_string(),
            "The safe contribution preserves the larger of the starter emergency fund and one month of recent average expenses.".to_string(),
            "This scenario does not know exact paycheck and bill ordering unless those dates are stored as planned transactions.".to_string(),
        ],
    })
}

pub fn run_purchase_affordability(
    conn: &mut Connection,
    purchase_amount_cents: i64,
) -> rusqlite::Result<PurchaseAffordabilityScenario> {
    let snapshot = build_snapshot(conn)?;
    let purchase_amount_cents = purchase_amount_cents.max(0);
    let emergency_floor_cents = snapshot
        .avg_monthly_expense_90d_cents
        .max(STARTER_EMERGENCY_CENTS);
    let starting_emergency_fund_cents = snapshot.emergency_fund_balance_cents.max(0);
    let emergency_fund_after_purchase_cents = starting_emergency_fund_cents - purchase_amount_cents;
    let emergency_months_after_purchase = if snapshot.avg_monthly_expense_90d_cents > 0 {
        emergency_fund_after_purchase_cents.max(0) as f64
            / snapshot.avg_monthly_expense_90d_cents as f64
    } else {
        0.0
    };
    let monthly_income = expected_monthly_income_cents(&snapshot);
    // Typical (median-month) expense basis so one recent big purchase doesn't
    // make everything look unaffordable.
    let monthly_surplus_cents = monthly_income - snapshot.typical_monthly_expense_cents;
    let high_interest_debt_cents: i64 = snapshot
        .liabilities
        .iter()
        .filter(|l| l.balance_cents > 0 && l.apr_pct.unwrap_or(0.0) >= HIGH_INTEREST_APR)
        .map(|l| l.balance_cents)
        .sum();
    let safe_cash_available_cents = (starting_emergency_fund_cents - emergency_floor_cents).max(0);
    let shortfall_to_cash_purchase_cents =
        (purchase_amount_cents - safe_cash_available_cents).max(0);
    let months_to_save_without_touching_emergency_floor = if purchase_amount_cents == 0 {
        Some(0)
    } else if shortfall_to_cash_purchase_cents == 0 {
        Some(0)
    } else if monthly_surplus_cents > 0 {
        Some(div_ceil(
            shortfall_to_cash_purchase_cents,
            monthly_surplus_cents,
        ))
    } else {
        None
    };

    let affordable_now = purchase_amount_cents > 0
        && emergency_fund_after_purchase_cents >= emergency_floor_cents
        && high_interest_debt_cents == 0
        && monthly_surplus_cents > 0;
    let recommendation = if purchase_amount_cents <= 0 {
        "I need the purchase amount before I can judge affordability.".to_string()
    } else if affordable_now {
        "The purchase looks affordable from current local cashflow and emergency-fund data."
            .to_string()
    } else if high_interest_debt_cents > 0 {
        "Delay the purchase until high-interest debt is handled or the purchase can be made without weakening debt payoff.".to_string()
    } else if emergency_fund_after_purchase_cents < emergency_floor_cents {
        "Delay or reduce the purchase because it would drop emergency cash below the protected floor.".to_string()
    } else if monthly_surplus_cents <= 0 {
        "Delay the purchase because current monthly cashflow does not show reliable surplus."
            .to_string()
    } else {
        "Treat the purchase as provisional and save into it without touching the emergency floor."
            .to_string()
    };

    let emergency_after_wait = starting_emergency_fund_cents;
    let emergency_months_after_wait = if snapshot.avg_monthly_expense_90d_cents > 0 {
        emergency_after_wait as f64 / snapshot.avg_monthly_expense_90d_cents as f64
    } else {
        0.0
    };
    let smaller_purchase_cents = safe_cash_available_cents.min(purchase_amount_cents);
    let smaller_after = starting_emergency_fund_cents - smaller_purchase_cents;
    let smaller_months_after = if snapshot.avg_monthly_expense_90d_cents > 0 {
        smaller_after.max(0) as f64 / snapshot.avg_monthly_expense_90d_cents as f64
    } else {
        0.0
    };

    Ok(PurchaseAffordabilityScenario {
        purchase_amount_cents,
        recommendation,
        affordable_now,
        starting_emergency_fund_cents,
        emergency_floor_cents,
        emergency_fund_after_purchase_cents,
        emergency_months_after_purchase,
        monthly_surplus_cents,
        months_to_save_without_touching_emergency_floor,
        high_interest_debt_cents,
        alternatives: vec![
            PurchaseAlternative {
                name: "Buy now".to_string(),
                action: format!(
                    "Spend {} now from emergency-eligible cash.",
                    format!("${:.2}", purchase_amount_cents as f64 / 100.0)
                ),
                cash_used_cents: purchase_amount_cents,
                emergency_fund_after_cents: emergency_fund_after_purchase_cents,
                emergency_months_after: emergency_months_after_purchase,
                months_until_purchase: Some(0),
                tradeoff: "Fastest option, but only acceptable if it preserves the emergency floor and does not compete with high-interest debt.".to_string(),
            },
            PurchaseAlternative {
                name: "Wait and save".to_string(),
                action: "Set aside monthly surplus until the purchase fits above the emergency floor.".to_string(),
                cash_used_cents: 0,
                emergency_fund_after_cents: emergency_after_wait,
                emergency_months_after: emergency_months_after_wait,
                months_until_purchase: months_to_save_without_touching_emergency_floor,
                tradeoff: "Slower, but preserves liquidity and keeps the purchase separate from emergency cash.".to_string(),
            },
            PurchaseAlternative {
                name: "Reduce purchase size".to_string(),
                action: format!(
                    "Cap the purchase near currently safe cash above the emergency floor: {}.",
                    format!("${:.2}", smaller_purchase_cents as f64 / 100.0)
                ),
                cash_used_cents: smaller_purchase_cents,
                emergency_fund_after_cents: smaller_after,
                emergency_months_after: smaller_months_after,
                months_until_purchase: Some(0),
                tradeoff: "Keeps the purchase immediate while protecting the emergency reserve, but may require choosing a cheaper option.".to_string(),
            },
        ],
        missing_data: snapshot.data_warnings,
        assumptions: vec![
            "Affordability uses emergency-eligible cash, 90-day average expenses, expected monthly income when configured, and high-interest debt at or above 8% APR.".to_string(),
            "This does not use external price, tax, insurance, financing, or market data.".to_string(),
        ],
    })
}

pub fn get_data_quality_report(conn: &mut Connection) -> rusqlite::Result<DataQualityReport> {
    let snapshot = build_snapshot(conn)?;
    let uncategorized_expense_count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM transactions t \
             WHERE category_id IS NULL AND amount_cents < 0 AND is_transfer = 0 AND {}",
            finsight_core::metrics::non_investment_txn_predicate("t")
        ),
        [],
        |r| r.get(0),
    )?;
    let missing_apr_count = snapshot
        .liabilities
        .iter()
        .filter(|l| l.balance_cents > 0 && l.apr_pct.is_none())
        .count() as i64;
    let missing_min_payment_count = snapshot
        .liabilities
        .iter()
        .filter(|l| l.balance_cents > 0 && l.min_payment_cents.is_none())
        .count() as i64;
    Ok(DataQualityReport {
        warnings: snapshot.data_warnings,
        missing_apr_count,
        missing_min_payment_count,
        uncategorized_expense_count,
        active_goal_count: snapshot.goals.len() as i64,
        active_liability_count: snapshot
            .liabilities
            .iter()
            .filter(|l| l.balance_cents > 0)
            .count() as i64,
        planned_transaction_count: snapshot.planned_transactions.len() as i64,
        data_sources: vec![
            "accounts".to_string(),
            "account_balances".to_string(),
            "transactions".to_string(),
            "goals".to_string(),
            "liabilities".to_string(),
            "planned_transactions".to_string(),
        ],
    })
}

fn simulate_debt_payoff(
    debts: &[SnapshotLiability],
    method: &str,
    extra_monthly_payment_cents: i64,
) -> Option<PayoffProjection> {
    let mut sim = debts
        .iter()
        .map(|d| {
            Some(SimDebt {
                id: d.id.clone(),
                name: d.name.clone(),
                balance_cents: d.balance_cents as f64,
                apr_pct: d.apr_pct?,
                min_payment_cents: d.min_payment_cents?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let mut months = 0_i64;
    let mut interest = 0.0;
    let base_minimums: i64 = sim.iter().map(|d| d.min_payment_cents).sum();
    let total_payment = base_minimums + extra_monthly_payment_cents.max(0);

    while sim.iter().any(|d| d.balance_cents > 0.5) && months < 600 {
        months += 1;
        for debt in sim.iter_mut().filter(|d| d.balance_cents > 0.5) {
            let month_interest = debt.balance_cents * (debt.apr_pct.max(0.0) / 100.0 / 12.0);
            interest += month_interest;
            debt.balance_cents += month_interest;
        }

        let mut remaining_payment = total_payment as f64;
        for debt in sim.iter_mut().filter(|d| d.balance_cents > 0.5) {
            let pay = (debt.min_payment_cents as f64)
                .min(debt.balance_cents)
                .min(remaining_payment);
            debt.balance_cents -= pay;
            remaining_payment -= pay;
        }

        while remaining_payment > 0.5 {
            let Some(idx) = next_debt_index(&sim, method) else {
                break;
            };
            let pay = remaining_payment.min(sim[idx].balance_cents);
            sim[idx].balance_cents -= pay;
            remaining_payment -= pay;
        }

        let active_interest: f64 = sim
            .iter()
            .filter(|d| d.balance_cents > 0.5)
            .map(|d| d.balance_cents * (d.apr_pct.max(0.0) / 100.0 / 12.0))
            .sum();
        if total_payment as f64 <= active_interest && active_interest > 0.0 {
            return None;
        }
    }

    if months >= 600 {
        return None;
    }
    Some(PayoffProjection {
        months,
        interest_cents: interest.round() as i64,
    })
}

fn next_debt_index(debts: &[SimDebt], method: &str) -> Option<usize> {
    debts
        .iter()
        .enumerate()
        .filter(|(_, debt)| debt.balance_cents > 0.5)
        .min_by(|(_, a), (_, b)| {
            if method == "snowball" {
                a.balance_cents
                    .partial_cmp(&b.balance_cents)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.name.cmp(&b.name))
            } else {
                b.apr_pct
                    .partial_cmp(&a.apr_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        a.balance_cents
                            .partial_cmp(&b.balance_cents)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| a.id.cmp(&b.id))
            }
        })
        .map(|(idx, _)| idx)
}

fn weighted_apr(debts: &[SnapshotLiability]) -> Option<f64> {
    let mut weighted = 0.0;
    let mut total = 0_i64;
    for debt in debts {
        let apr = debt.apr_pct?;
        weighted += apr * debt.balance_cents.max(0) as f64;
        total += debt.balance_cents.max(0);
    }
    if total > 0 {
        Some(weighted / total as f64)
    } else {
        None
    }
}

fn payoff_projection(
    balance_cents: i64,
    apr_pct: f64,
    monthly_payment_cents: i64,
) -> Option<PayoffProjection> {
    if balance_cents <= 0 {
        return Some(PayoffProjection {
            months: 0,
            interest_cents: 0,
        });
    }
    if monthly_payment_cents <= 0 {
        return None;
    }
    let monthly_rate = (apr_pct.max(0.0) / 100.0) / 12.0;
    let mut balance = balance_cents as f64;
    let payment = monthly_payment_cents as f64;
    let mut months = 0_i64;
    let mut interest = 0.0;
    while balance > 0.5 && months < 600 {
        let month_interest = balance * monthly_rate;
        if payment <= month_interest && monthly_rate > 0.0 {
            return None;
        }
        interest += month_interest;
        balance = (balance + month_interest - payment).max(0.0);
        months += 1;
    }
    if months >= 600 {
        return None;
    }
    Some(PayoffProjection {
        months,
        interest_cents: interest.round() as i64,
    })
}

fn build_debt_goal_alternatives(
    snapshot: &FinancialSnapshot,
    goal: &SnapshotGoal,
    debt_cents: i64,
    safe_drawdown_cents: i64,
    min_payment_cents: i64,
    current: Option<PayoffProjection>,
    after_drawdown: Option<PayoffProjection>,
    with_redirect: Option<PayoffProjection>,
) -> Vec<ScenarioAlternative> {
    let post_drawdown_debt = debt_cents.saturating_sub(safe_drawdown_cents);
    // EF-months on the single (EF-eligible, capped) definition, conservatively
    // netting the drawdown against the emergency-fund pool.
    let emergency_after = finsight_core::metrics::emergency_fund_months(
        (snapshot.emergency_fund_balance_cents - safe_drawdown_cents).max(0),
        snapshot.avg_monthly_expense_90d_cents,
    );

    vec![
        ScenarioAlternative {
            name: "Keep car savings intact".to_string(),
            action: "Make minimum debt payments and keep adding to the car goal.".to_string(),
            cash_used_cents: 0,
            monthly_debt_payment_cents: Some(min_payment_cents),
            projected_debt_balance_cents: debt_cents,
            emergency_fund_months: snapshot.emergency_fund_months,
            payoff_months: current.map(|p| p.months),
            interest_cents: current.map(|p| p.interest_cents),
            tradeoff: "Protects the car timeline and liquidity, but usually costs more interest."
                .to_string(),
        },
        ScenarioAlternative {
            name: "Use only safe excess savings".to_string(),
            action: format!(
                "Apply up to ${:.2} from the goal balance while preserving the emergency floor.",
                safe_drawdown_cents as f64 / 100.0
            ),
            cash_used_cents: safe_drawdown_cents,
            monthly_debt_payment_cents: Some(min_payment_cents),
            projected_debt_balance_cents: post_drawdown_debt,
            emergency_fund_months: emergency_after,
            payoff_months: after_drawdown.map(|p| p.months),
            interest_cents: after_drawdown.map(|p| p.interest_cents),
            tradeoff: "Reduces interest without dropping below the emergency reserve floor, but delays the car goal."
                .to_string(),
        },
        ScenarioAlternative {
            name: "Safe drawdown plus redirected car contributions".to_string(),
            action: format!(
                "Use safe excess savings and redirect the current {} contribution of ${:.2}/mo to debt until it is cleared.",
                goal.name,
                goal.monthly_cents.max(0) as f64 / 100.0
            ),
            cash_used_cents: safe_drawdown_cents,
            monthly_debt_payment_cents: Some(min_payment_cents + goal.monthly_cents.max(0)),
            projected_debt_balance_cents: post_drawdown_debt,
            emergency_fund_months: emergency_after,
            payoff_months: with_redirect.map(|p| p.months),
            interest_cents: with_redirect.map(|p| p.interest_cents),
            tradeoff: "Usually clears debt fastest while protecting emergency cash; car savings pauses temporarily."
                .to_string(),
        },
    ]
}

fn income_expense_since(conn: &Connection, cutoff: &str) -> rusqlite::Result<(i64, i64)> {
    // Route through the shared metrics layer so the forecast/scenario averages
    // use the exact same income/expense definition (transfers excluded) as every
    // screen and the Copilot. Unwrap the DB error back to `rusqlite::Error` to
    // preserve this function's signature; that's the only failure this can hit.
    finsight_core::metrics::income_expense_since(conn, cutoff).map_err(|e| match e {
        finsight_core::CoreError::Database(e) => e,
        other => rusqlite::Error::ToSqlConversionFailure(Box::new(other)),
    })
}

fn expected_monthly_income_cents(snapshot: &FinancialSnapshot) -> i64 {
    let Some(paycheck) = snapshot.expected_paycheck_cents else {
        return snapshot.avg_monthly_income_90d_cents;
    };
    match snapshot.paycheck_cadence.as_deref() {
        Some("weekly") => (paycheck as f64 * 52.0 / 12.0).round() as i64,
        Some("biweekly") | Some("bi-weekly") => (paycheck as f64 * 26.0 / 12.0).round() as i64,
        Some("semimonthly") | Some("semi-monthly") => paycheck * 2,
        Some("monthly") => paycheck,
        _ => snapshot.avg_monthly_income_90d_cents,
    }
}

fn accounts(conn: &mut Connection) -> rusqlite::Result<Vec<SnapshotAccount>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.type,
                COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC, CASE source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END LIMIT 1), 0) AS balance,
                a.liquidity_type, a.emergency_fund_eligible, a.goal_earmark, a.apy_pct,
                EXISTS(SELECT 1 FROM account_balances b WHERE b.account_id = a.id) AS balance_known
         FROM accounts a
         WHERE a.archived_at IS NULL
         ORDER BY a.bank, a.name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SnapshotAccount {
            id: r.get(0)?,
            name: r.get(1)?,
            account_type: r.get(2)?,
            balance_cents: r.get(3)?,
            liquidity_type: r.get(4)?,
            emergency_fund_eligible: r.get::<_, i64>(5)? != 0,
            goal_earmark: r.get(6)?,
            apy_pct: r.get(7)?,
            balance_known: r.get::<_, i64>(8)? != 0,
        })
    })?;
    rows.collect()
}

fn setting_string(conn: &mut Connection, key: &str) -> rusqlite::Result<Option<String>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    Ok(raw.and_then(|value| {
        serde_json::from_str::<Option<String>>(&value)
            .ok()
            .flatten()
    }))
}

fn setting_i64(conn: &mut Connection, key: &str) -> rusqlite::Result<Option<i64>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    Ok(raw.and_then(|value| serde_json::from_str::<Option<i64>>(&value).ok().flatten()))
}

fn goals(conn: &mut Connection) -> rusqlite::Result<Vec<SnapshotGoal>> {
    let mut stmt = conn.prepare("SELECT id, name, type, target_cents, current_cents, monthly_cents, target_date FROM goals WHERE archived_at IS NULL ORDER BY sort_order, created_at")?;
    let rows = stmt.query_map([], |r| {
        let target: i64 = r.get(3)?;
        let current: i64 = r.get(4)?;
        let monthly: i64 = r.get(5)?;
        let remaining = (target - current).max(0);
        let eta_months = if monthly > 0 && remaining > 0 {
            Some(div_ceil(remaining, monthly))
        } else if remaining == 0 {
            Some(0)
        } else {
            None
        };
        Ok(SnapshotGoal {
            id: r.get(0)?,
            name: r.get(1)?,
            goal_type: r.get(2)?,
            target_cents: target,
            current_cents: current,
            monthly_cents: monthly,
            target_date: r.get(6)?,
            remaining_cents: remaining,
            eta_months,
        })
    })?;
    rows.collect()
}

fn goal_by_id(conn: &mut Connection, goal_id: &str) -> rusqlite::Result<Option<SnapshotGoal>> {
    goals(conn).map(|goals| goals.into_iter().find(|g| g.id == goal_id))
}

/// Debt used to live in a separate `liabilities` table (positive
/// `balance_cents` = amount owed); it's now a Credit/Loan-type Account with a
/// negative balance. This reads the latest known balance per debt account and
/// negates it back to the old "amount owed" convention `SnapshotLiability`
/// (and everything downstream that reasons about debt) already expects — the
/// planning/reasoning logic itself is unchanged, only its data source moved.
fn liabilities(conn: &mut Connection) -> rusqlite::Result<Vec<SnapshotLiability>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, balance, limit_cents, apr_pct, min_payment_cents, payoff_date, original_balance_cents, started_at FROM (
             SELECT a.id, a.name, a.type,
                    -COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC, CASE source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END LIMIT 1), 0) AS balance,
                    a.limit_cents, a.apr_pct, a.min_payment_cents, a.payoff_date, a.original_balance_cents, a.started_at
             FROM accounts a
             WHERE a.archived_at IS NULL AND a.type IN ('Credit', 'Loan')
         ) WHERE balance > 0
         ORDER BY balance DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let account_type: String = r.get(2)?;
        Ok(SnapshotLiability {
            id: r.get(0)?,
            name: r.get(1)?,
            liability_type: if account_type == "Credit" {
                "credit-card".into()
            } else {
                "loan".into()
            },
            balance_cents: r.get(3)?,
            limit_cents: r.get(4)?,
            apr_pct: r.get(5)?,
            min_payment_cents: r.get(6)?,
            payoff_date: r.get(7)?,
            original_balance_cents: r.get(8)?,
            started_at: r.get(9)?,
        })
    })?;
    rows.collect()
}

fn recurring_bills(conn: &mut Connection) -> rusqlite::Result<Vec<SnapshotRecurringBill>> {
    let cutoff = (Utc::now() - Duration::days(395))
        .format("%Y-%m-%d")
        .to_string();
    let mut stmt = conn.prepare(
        "WITH gaps AS (
            SELECT merchant_raw, date(posted_at) AS d, amount_cents, LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) AS prev_d
            FROM transactions WHERE posted_at >= ?1
         ), agg AS (
            SELECT merchant_raw, AVG(julianday(d)-julianday(prev_d)) AS avg_gap, MAX(d) AS last_seen, MAX(amount_cents) AS last_amount, COUNT(*) AS occ
            FROM gaps WHERE prev_d IS NOT NULL GROUP BY merchant_raw HAVING occ >= 2 AND AVG(julianday(d)-julianday(prev_d)) BETWEEN 5 AND 400 AND MAX(amount_cents) < 0
         ) SELECT merchant_raw, avg_gap, last_seen, ABS(last_amount) FROM agg ORDER BY ABS(last_amount) DESC LIMIT 10"
    )?;
    let rows = stmt.query_map(params![cutoff], |r| {
        let avg_gap: f64 = r.get(1)?;
        let last_seen: String = r.get(2)?;
        let next_expected = chrono::NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d")
            .map(|d| {
                (d + Duration::days(avg_gap.round() as i64))
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .unwrap_or(last_seen);
        Ok(SnapshotRecurringBill {
            merchant: r.get(0)?,
            amount_cents: r.get(3)?,
            next_expected,
        })
    })?;
    rows.collect()
}

fn planned_transactions(
    conn: &mut Connection,
) -> rusqlite::Result<Vec<SnapshotPlannedTransaction>> {
    let mut stmt = conn.prepare("SELECT id, description, amount_cents, due_date, status FROM planned_transactions WHERE status = 'planned' ORDER BY due_date ASC LIMIT 10")?;
    let rows = stmt.query_map([], |r| {
        Ok(SnapshotPlannedTransaction {
            id: r.get(0)?,
            description: r.get(1)?,
            amount_cents: r.get(2)?,
            due_date: r.get(3)?,
            status: r.get(4)?,
        })
    })?;
    rows.collect()
}

fn div_ceil(n: i64, d: i64) -> i64 {
    if d <= 0 {
        return i64::MAX;
    }
    (n + d - 1) / d
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("finance.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut Connection) {
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id, as_of_date, balance_cents) VALUES('a1','2026-06-01',500000)", []).unwrap();
        conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('car','Car','save-by-date',2000000,500000,0,'#fff',0,datetime('now'))", []).unwrap();
        // Debt is now a Credit/Loan-type Account with a negative balance, not
        // a separate liabilities-table row.
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,limit_cents,created_at) VALUES('cc','Household','Manual','Credit','Credit Card','USD','#F97316','manual','restricted',0,'debt',24.9,5000,500000,datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('cc',date('now'),-250000,'manual')", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,created_at) VALUES('loan','Household','Manual','Loan','Loan','USD','#F87171','manual','restricted',0,'debt',5.0,30000,datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('loan',date('now'),-1800000,'manual')", []).unwrap();
        for days in [10, 40, 70] {
            conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES(hex(randomblob(16)),'a1',datetime('now', ?1),300000,'Payroll','cleared',datetime('now'))", [format!("-{days} days")]).unwrap();
        }
        for days in [5, 35, 65] {
            conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES(hex(randomblob(16)),'a1',datetime('now', ?1),-200000,'Rent','cleared',datetime('now'))", [format!("-{days} days")]).unwrap();
        }
    }

    #[test]
    fn typical_monthly_expense_ignores_one_off_spike() {
        // P1-3.3: a single large one-off in the last 90 days inflates the 90-day
        // average expense and crushes projected surplus. The median-month basis
        // is immune, so surplus projections stay sane.
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('a1','Me','Bank','Checking','Chk','USD','#fff',datetime('now'))", []).unwrap();
        let today = chrono::Utc::now().date_naive();
        // 12 completed months of a steady ~$2,000/month expense.
        for m in 1..=12 {
            let d = today - chrono::Duration::days(30 * m);
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
                 VALUES(hex(randomblob(16)),'a1',?1,-200000,'Groceries','cleared',datetime('now'))",
                [format!("{}T12:00:00Z", d.format("%Y-%m-%d"))],
            )
            .unwrap();
        }
        // One-off $20,000 spike ~1 month ago (inside the 90-day window).
        let spike = today - chrono::Duration::days(30);
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
             VALUES(hex(randomblob(16)),'a1',?1,-2000000,'Car repair','cleared',datetime('now'))",
            [format!("{}T12:00:00Z", spike.format("%Y-%m-%d"))],
        )
        .unwrap();

        let snap = build_snapshot(&mut conn).unwrap();
        assert!(
            snap.typical_monthly_expense_cents <= 300_000,
            "median month is not inflated by the one-off (got {})",
            snap.typical_monthly_expense_cents
        );
        assert!(
            snap.avg_monthly_expense_90d_cents > snap.typical_monthly_expense_cents,
            "the 90-day average IS dragged up by the spike — so the robust basis matters"
        );
    }

    #[test]
    fn question_profile_detects_cash_inflow_and_amount() {
        let profile = infer_question_profile("I got a pay of around $3,000. What should I do?");
        assert_eq!(profile.kind, FinanceQuestionKind::CashInflow);
        assert_eq!(profile.amount_cents, Some(300_000));
    }

    #[test]
    fn question_profile_detects_goal_eta_cadence() {
        let profile =
            infer_question_profile("If I save $500 bi-weekly, how soon will I reach my car goal?");
        assert_eq!(profile.kind, FinanceQuestionKind::GoalEta);
        assert_eq!(profile.amount_cents, Some(50_000));
        assert_eq!(profile.cadence.as_deref(), Some("biweekly"));
    }

    #[test]
    fn question_profile_detects_debt_vs_goal() {
        let profile = infer_question_profile(
            "Should I take money from my car savings and pay off the loan first?",
        );
        assert_eq!(profile.kind, FinanceQuestionKind::DebtVsGoal);
    }

    #[test]
    fn snapshot_uses_planning_metadata_for_emergency_cash_and_paychecks() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "UPDATE accounts SET liquidity_type = 'restricted', emergency_fund_eligible = 0, goal_earmark = 'car' WHERE id = 'a1'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('planning.paycheck_cadence', '\"biweekly\"')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('planning.expected_paycheck_cents', '300000')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();

        let snapshot = build_snapshot(&mut conn).unwrap();

        assert_eq!(snapshot.liquid_balance_cents, 500_000);
        assert_eq!(snapshot.emergency_fund_balance_cents, 0);
        assert_eq!(snapshot.emergency_fund_months, 0.0);
        assert_eq!(snapshot.paycheck_cadence.as_deref(), Some("biweekly"));
        assert_eq!(snapshot.expected_paycheck_cents, Some(300_000));
        assert_eq!(snapshot.accounts[0].goal_earmark.as_deref(), Some("car"));
        let timeline = run_cashflow_timeline(&mut conn, 1).unwrap();
        assert_eq!(timeline.months[0].expected_income_cents, 650_000);
    }

    #[test]
    fn cash_inflow_prioritizes_emergency_then_high_interest_debt() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let advice = analyze_cash_inflow(&mut conn, 300000).unwrap();
        assert!(advice
            .allocations
            .iter()
            .any(|a| a.bucket == "high_interest_debt" && a.target_id.as_deref() == Some("cc")));
        assert!(!advice.investing_allowed);
    }

    #[test]
    fn biweekly_goal_eta_uses_26_periods() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let eta = calculate_goal_eta(&mut conn, "car", 50000, "biweekly").unwrap();
        assert_eq!(eta.monthly_equivalent_cents, 108333);
        assert_eq!(eta.eta_pay_periods, Some(30));
    }

    #[test]
    fn semimonthly_goal_eta_uses_24_periods() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let eta = calculate_goal_eta(&mut conn, "car", 50_000, "semimonthly").unwrap();
        assert_eq!(eta.monthly_equivalent_cents, 100_000);
        assert_eq!(eta.eta_months, Some(15));
        assert_eq!(eta.eta_pay_periods, Some(30));
    }

    #[test]
    fn debt_ranking_supports_avalanche_and_snowball() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let avalanche = rank_debt_payoff(&mut conn, "avalanche").unwrap();
        assert_eq!(avalanche.items[0].liability_id, "cc");
        let snowball = rank_debt_payoff(&mut conn, "snowball").unwrap();
        assert_eq!(snowball.items[0].liability_id, "cc");
    }

    #[test]
    fn car_savings_vs_debt_protects_emergency_fund() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let cmp = compare_debt_vs_goal(&mut conn, "car", Some("cc")).unwrap();
        assert!(cmp.recommendation.contains("debt") || cmp.recommendation.contains("liquidity"));
        assert!(cmp.suggested_goal_drawdown_cents <= 250000);
        assert_eq!(cmp.alternatives.len(), 3);
        assert!(cmp.payoff_months_after_drawdown.is_some());
        assert!(cmp
            .missing_data
            .iter()
            .all(|warning| !warning.contains("APR missing")));
    }

    #[test]
    fn car_savings_vs_similar_sized_loan_compares_timeline_and_interest() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "UPDATE goals SET monthly_cents = 50000 WHERE id = 'car'",
            [],
        )
        .unwrap();

        let cmp = compare_debt_vs_goal(&mut conn, "car", Some("loan")).unwrap();

        assert_eq!(cmp.debt_name.as_deref(), Some("Loan"));
        assert_eq!(cmp.highest_apr_pct, Some(5.0));
        assert_eq!(cmp.alternatives.len(), 3);
        assert!(cmp.payoff_months_current.is_some());
        assert!(cmp.payoff_months_with_redirect.is_some());
        assert!(cmp.estimated_interest_saved_cents.unwrap_or(0) > 0);
        assert_eq!(cmp.suggested_goal_drawdown_cents, 0);
    }

    #[test]
    fn debt_payoff_scenarios_show_extra_payment_savings() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);

        let scenarios = run_debt_payoff_scenarios(&mut conn, "avalanche", 50_000).unwrap();

        assert_eq!(scenarios.payoff_order[0].liability_id, "cc");
        assert!(
            scenarios.payoff_months_minimums_only.unwrap()
                > scenarios.payoff_months_with_extra.unwrap()
        );
        assert!(scenarios.estimated_interest_saved_cents.unwrap() > 0);
        assert!(scenarios.months_saved.unwrap() > 0);
    }

    #[test]
    fn goal_allocation_scenarios_allocate_monthly_savings() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);

        let scenarios = run_goal_allocation_scenarios(&mut conn, 50_000, "priority").unwrap();

        assert_eq!(scenarios.monthly_available_cents, 50_000);
        assert_eq!(scenarios.allocations[0].goal_id, "car");
        assert_eq!(scenarios.allocations[0].suggested_monthly_cents, 50_000);
        assert_eq!(scenarios.allocations[0].eta_months, Some(30));
    }

    #[test]
    fn goal_conflict_delays_contribution_for_upcoming_bills() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "INSERT INTO planned_transactions(id, description, amount_cents, account_id, due_date, status, source, created_at) VALUES('bill1','Insurance premium',-350000,'a1',date('now','+7 days'),'planned','manual',datetime('now'))",
            [],
        )
        .unwrap();

        let scenario = run_goal_conflict_scenario(&mut conn, "car", 100_000).unwrap();

        assert!(scenario.conflicts_with_cashflow);
        assert_eq!(scenario.goal_name, "Car");
        assert!(scenario.upcoming_obligations_cents >= 350_000);
        assert_eq!(scenario.safe_contribution_now_cents, 0);
        assert_eq!(scenario.alternatives.len(), 3);
        assert!(scenario.recommendation.contains("Delay or reduce"));
    }

    #[test]
    fn emergency_fund_scenarios_model_one_three_six_month_targets() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);

        let scenarios = run_emergency_fund_scenarios(&mut conn, 50_000).unwrap();

        assert_eq!(scenarios.targets.len(), 3);
        assert_eq!(scenarios.targets[0].target_months, 1);
        assert!(scenarios.current_months > 0.0);
    }

    #[test]
    fn emergency_fund_defaults_contribution_to_surplus_and_projects_completion_date() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        // seed(): ~$3,000/mo income, ~$2,000/mo expense → positive surplus.
        // Balance $5,000 → below the 3-month target ($6,000), so a gap exists.

        // No contribution provided: should default to the monthly surplus.
        let scenarios = run_emergency_fund_scenarios(&mut conn, 0).unwrap();
        assert!(
            scenarios.monthly_surplus_cents > 0,
            "seed should produce a positive surplus"
        );
        assert_eq!(
            scenarios.effective_monthly_contribution_cents, scenarios.monthly_surplus_cents,
            "with no contribution, the surplus is used"
        );

        let three_month = scenarios
            .targets
            .iter()
            .find(|t| t.target_months == 3)
            .unwrap();
        assert!(
            three_month.gap_cents > 0,
            "3-month target should have a gap"
        );
        assert!(
            three_month.months_to_target_at_contribution.is_some(),
            "a completion timeline should be projected from the surplus"
        );
        let date = three_month
            .estimated_completion_date
            .as_ref()
            .expect("completion date present");
        assert!(
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok(),
            "completion date must be a valid YYYY-MM-DD: {date}"
        );
        assert!(
            date.as_str() > chrono::Utc::now().format("%Y-%m-%d").to_string().as_str(),
            "completion date must be in the future"
        );
    }

    #[test]
    fn add_months_handles_year_and_month_end_rollover() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        // +1 month from Jan 31 clamps to Feb 28 (2026 not a leap year).
        assert_eq!(
            add_months(d, 1),
            chrono::NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        // +13 months crosses a year boundary.
        assert_eq!(
            add_months(chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(), 13),
            chrono::NaiveDate::from_ymd_opt(2027, 7, 15).unwrap()
        );
    }

    #[test]
    fn purchase_affordability_delays_when_emergency_floor_would_break() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute("UPDATE accounts SET apr_pct = 5.0 WHERE id = 'cc'", [])
            .unwrap();

        let scenario = run_purchase_affordability(&mut conn, 450_000).unwrap();

        assert!(!scenario.affordable_now);
        assert!(scenario.emergency_fund_after_purchase_cents < scenario.emergency_floor_cents);
        assert_eq!(scenario.alternatives.len(), 3);
        assert!(scenario
            .recommendation
            .contains("emergency cash below the protected floor"));
    }
    #[test]
    fn cashflow_timeline_warns_when_projected_balance_is_low() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "UPDATE account_balances SET balance_cents = 10000 WHERE account_id = 'a1'",
            [],
        )
        .unwrap();

        let timeline = run_cashflow_timeline(&mut conn, 2).unwrap();

        assert_eq!(timeline.months.len(), 2);
        assert!(!timeline.low_balance_warnings.is_empty());
    }

    #[test]
    fn data_quality_report_counts_missing_debt_fields() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "UPDATE accounts SET apr_pct = NULL, min_payment_cents = NULL WHERE id = 'loan'",
            [],
        )
        .unwrap();

        let report = get_data_quality_report(&mut conn).unwrap();

        assert_eq!(report.missing_apr_count, 1);
        assert_eq!(report.missing_min_payment_count, 1);
        assert!(report.active_goal_count >= 1);
    }
}

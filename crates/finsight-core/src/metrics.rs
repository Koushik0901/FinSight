//! Single source of truth for derived financial numbers.
//!
//! Every screen and the Copilot must agree on what "savings rate", "runway",
//! "liquid", "emergency fund", and "average monthly income/expense" mean. Before
//! this module those definitions were hand-rolled in a dozen places — savings
//! rate had five variants, runway three, and the transfer-exclusion rule was
//! forgotten in six queries. Route every consumer through here so a definition
//! change (or a bug fix) happens exactly once.
//!
//! Convention decisions made here, deliberately:
//! - **Transfers are never income or expense.** Every aggregate below filters
//!   `is_transfer = 0`; callers cannot forget it because they never write the SQL.
//! - **Investment-account activity is never income or expense, EXCEPT
//!   dividends/interest/withholding tax.** Trades and contributions inside a
//!   brokerage account are wealth moving within the investment world
//!   ([`non_investment_txn_predicate`]); the spendable-side contribution leg
//!   is handled by transfer detection/review. But a dividend or interest
//!   payment IS real income the moment it lands (`activity_type` = 'Dividend'
//!   | 'Interest'), and withholding tax on it IS a real expense
//!   (`activity_type` = 'Tax') — both are carved back in so a brokerage
//!   account's income shows up in savings rate, budget, and Copilot context
//!   like any other income.
//! - **Savings rate is signed and honest.** A deficit month yields a *negative*
//!   rate; it is not clamped to zero. Callers that want a progress bar can clamp
//!   at the display edge, but the metric itself never hides a deficit.
//! - **Runway is liquid ÷ average burn.** Not net worth (which includes illiquid
//!   assets and debts), not month-to-date spend (which lurches with pay cycles).
//! - **Balances classify by account TYPE, not balance sign.** An overdrawn
//!   checking account is still liquid; a credit card is debt whatever its sign.

use crate::error::CoreResult;
use crate::forecast;
use crate::models::AccountType;
use crate::repos::{accounts, net_worth};
use rusqlite::{params, Connection};

/// Period used to turn an average monthly outflow into a daily burn for runway.
pub const RUNWAY_PERIOD_DAYS: i64 = 30;

/// Emergency-fund coverage is capped so an outsized cash balance doesn't render
/// an absurd "hundreds of months" figure.
pub const EMERGENCY_FUND_MONTHS_CAP: f64 = 24.0;

// ── Account classification ──────────────────────────────────────────────────

/// Credit cards and loans — debt, regardless of the current balance sign.
pub fn is_debt_type(t: AccountType) -> bool {
    matches!(t, AccountType::Credit | AccountType::Loan)
}

/// Brokerage / retirement holdings — assets, but not spendable liquidity.
pub fn is_investment_type(t: AccountType) -> bool {
    matches!(t, AccountType::Investment)
}

/// SQL predicate: the transaction (on the given `transactions` alias) does not
/// live in an investment account, OR it does but is a Dividend/Interest/Tax
/// row (V048 `activity_type` — real income/expense, not wealth-shuffling).
/// BUY/SELL trades and contributions are wealth moving inside the investment
/// world, not living income or spending; letting them into cashflow swings the
/// savings rate with every trade, and letting them into categorization/recurring/
/// anomaly surfaces nags the user about rows that aren't budget material. Must
/// stay in lockstep with [`is_investment_type`] (pinned by a unit test) for the
/// account-type half; the activity carve-out is independently pinned by
/// `income_expense_includes_dividend_and_interest_in_investment_accounts`.
///
/// Untyped investment-account rows (a manual entry, or a non-CSV provider that
/// never populates `activity_type`) fall through to the blanket exclusion —
/// only rows explicitly typed as income/expense earn the exception.
pub fn non_investment_txn_predicate(alias: &str) -> String {
    format!(
        "({alias}.account_id NOT IN (SELECT id FROM accounts WHERE type = 'Investment') \
          OR {alias}.activity_type IN ('Dividend', 'Interest', 'Tax'))"
    )
}

/// Cash and near-cash: everything that isn't debt or an investment. This is the
/// pool runway and emergency-fund coverage are measured against.
pub fn is_liquid_type(t: AccountType) -> bool {
    !is_debt_type(t) && !is_investment_type(t)
}

// ── The one savings-rate formula ────────────────────────────────────────────

/// Savings rate as a signed percentage: `(income - expense) / income * 100`.
/// Returns 0 when there is no income to divide by. NOT clamped — a deficit
/// shows as negative, on every surface.
pub fn savings_rate_pct(income_cents: i64, expense_cents: i64) -> i64 {
    if income_cents <= 0 {
        0
    } else {
        ((income_cents - expense_cents) * 100) / income_cents
    }
}

/// Months of expenses the given emergency-fund balance covers, capped. Returns
/// 0.0 when average expense is unknown (can't divide).
pub fn emergency_fund_months(emergency_fund_cents: i64, avg_monthly_expense_cents: i64) -> f64 {
    if avg_monthly_expense_cents > 0 {
        (emergency_fund_cents.max(0) as f64 / avg_monthly_expense_cents as f64)
            .min(EMERGENCY_FUND_MONTHS_CAP)
    } else {
        0.0
    }
}

/// Days a liquid balance lasts at a given average monthly burn — the single
/// runway definition. Delegates to [`forecast::runway_days`] with a fixed
/// 30-day period so income cadence doesn't distort the figure.
pub fn runway_days(liquid_cents: i64, avg_monthly_expense_cents: i64) -> i64 {
    forecast::runway_days(liquid_cents, avg_monthly_expense_cents, RUNWAY_PERIOD_DAYS)
}

// ── User-configurable assumptions ───────────────────────────────────────────

/// Targets and rates the user can tune, with defaults drawn from the Financial
/// Freedom Framework (pay-yourself-first ≥20%, a 6-month full emergency fund,
/// a 7% long-run return). Stored in the settings KV so the screens, health
/// score, and compound projector all read the same numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Assumptions {
    pub target_savings_rate_pct: i64,
    pub emergency_fund_target_months: f64,
    pub expected_annual_return_pct: f64,
}

impl Default for Assumptions {
    fn default() -> Self {
        Self {
            target_savings_rate_pct: 20,
            emergency_fund_target_months: 6.0,
            expected_annual_return_pct: 7.0,
        }
    }
}

pub const KEY_TARGET_SAVINGS_RATE_PCT: &str = "assumptions.target_savings_rate_pct";
pub const KEY_EMERGENCY_FUND_TARGET_MONTHS: &str = "assumptions.emergency_fund_target_months";
pub const KEY_EXPECTED_ANNUAL_RETURN_PCT: &str = "assumptions.expected_annual_return_pct";

/// Read the user's assumptions, falling back to framework defaults for any that
/// aren't set (or if the settings read fails — assumptions are never critical
/// enough to fail a whole request over).
pub fn assumptions(conn: &Connection) -> Assumptions {
    let d = Assumptions::default();
    Assumptions {
        target_savings_rate_pct: crate::settings::get(conn, KEY_TARGET_SAVINGS_RATE_PCT)
            .ok()
            .flatten()
            .unwrap_or(d.target_savings_rate_pct),
        emergency_fund_target_months: crate::settings::get(conn, KEY_EMERGENCY_FUND_TARGET_MONTHS)
            .ok()
            .flatten()
            .unwrap_or(d.emergency_fund_target_months),
        expected_annual_return_pct: crate::settings::get(conn, KEY_EXPECTED_ANNUAL_RETURN_PCT)
            .ok()
            .flatten()
            .unwrap_or(d.expected_annual_return_pct),
    }
}

/// Persist the user's assumptions.
pub fn set_assumptions(conn: &Connection, a: &Assumptions) -> CoreResult<()> {
    crate::settings::set(conn, KEY_TARGET_SAVINGS_RATE_PCT, &a.target_savings_rate_pct)?;
    crate::settings::set(conn, KEY_EMERGENCY_FUND_TARGET_MONTHS, &a.emergency_fund_target_months)?;
    crate::settings::set(conn, KEY_EXPECTED_ANNUAL_RETURN_PCT, &a.expected_annual_return_pct)?;
    Ok(())
}

// ── Cashflow over a window ──────────────────────────────────────────────────

/// Income and expense (both positive cents) since `start_inclusive`, transfers
/// and investment-account activity excluded.
pub fn income_expense_since(conn: &Connection, start_inclusive: &str) -> CoreResult<(i64, i64)> {
    let pred = non_investment_txn_predicate("t");
    conn.query_row(
        &format!(
            "SELECT
                COALESCE(SUM(CASE WHEN amount_cents > 0 AND settle_up = 0 THEN amount_cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN settle_up = 1 THEN -amount_cents
                                  WHEN amount_cents < 0 THEN -amount_cents
                                  ELSE 0 END), 0)
             FROM transactions t
             WHERE t.posted_at >= ?1 AND t.is_transfer = 0 AND {pred}"
        ),
        params![start_inclusive],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(Into::into)
}

/// Income and expense (both positive cents) over `[start_inclusive, end_exclusive)`,
/// transfers and investment-account activity excluded.
pub fn income_expense_between(
    conn: &Connection,
    start_inclusive: &str,
    end_exclusive: &str,
) -> CoreResult<(i64, i64)> {
    let pred = non_investment_txn_predicate("t");
    conn.query_row(
        &format!(
            "SELECT
                COALESCE(SUM(CASE WHEN amount_cents > 0 AND settle_up = 0 THEN amount_cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN settle_up = 1 THEN -amount_cents
                                  WHEN amount_cents < 0 THEN -amount_cents
                                  ELSE 0 END), 0)
             FROM transactions t
             WHERE t.posted_at >= ?1 AND t.posted_at < ?2 AND t.is_transfer = 0 \
               AND {pred}"
        ),
        params![start_inclusive, end_exclusive],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(Into::into)
}

/// Income, expense, net, and savings rate for a single window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cashflow {
    pub income_cents: i64,
    pub expense_cents: i64,
    pub net_cents: i64,
    pub savings_rate_pct: i64,
}

impl Cashflow {
    fn from_income_expense(income: i64, expense: i64) -> Self {
        Cashflow {
            income_cents: income,
            expense_cents: expense,
            net_cents: income - expense,
            savings_rate_pct: savings_rate_pct(income, expense),
        }
    }
}

/// Cashflow since `start_inclusive` (e.g. the first of the calendar month).
pub fn cashflow_since(conn: &Connection, start_inclusive: &str) -> CoreResult<Cashflow> {
    let (income, expense) = income_expense_since(conn, start_inclusive)?;
    Ok(Cashflow::from_income_expense(income, expense))
}

/// Cashflow over `[start_inclusive, end_exclusive)`.
pub fn cashflow_between(
    conn: &Connection,
    start_inclusive: &str,
    end_exclusive: &str,
) -> CoreResult<Cashflow> {
    let (income, expense) = income_expense_between(conn, start_inclusive, end_exclusive)?;
    Ok(Cashflow::from_income_expense(income, expense))
}

// ── Rolling averages ────────────────────────────────────────────────────────

/// Average monthly income/expense over a trailing window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RollingAverages {
    pub window_days: i64,
    /// Months actually averaged over — the span of real history inside the
    /// window, not the window's nominal length. A user with six weeks of data
    /// gets 1.5, not 3.
    pub months: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub net_monthly_cents: i64,
    pub savings_rate_pct: i64,
    /// Days from the earliest transaction in the window to today. Below ~30 any
    /// monthly figure is an extrapolation from a partial month, so consumers of
    /// the SAFETY metrics (runway, emergency-fund coverage) should hedge or
    /// withhold rather than state a number — being wrong high there tells
    /// someone they are safer than they are.
    pub data_span_days: i64,
}

/// Average monthly income and expense over the last `days`, transfers excluded.
///
/// Divides by the months of history that ACTUALLY EXIST in the window, not by
/// the window's nominal length. Dividing a new user's single month of activity
/// by a fixed three understated their burn threefold, which in turn overstated
/// every safety metric built on it — a $6,000 fund against $2,000/mo read as
/// nine months of cover instead of three. Overstating safety is the dangerous
/// direction, and it landed on brand-new users, who are least equipped to
/// notice.
///
/// Sub-month histories are floored at one month rather than extrapolated: a
/// single large purchase three days in would otherwise imply an absurd monthly
/// burn. That floor understates instead, so `data_span_days` is reported
/// alongside and safety-metric consumers are expected to hedge below ~30 days.
pub fn rolling_averages(conn: &Connection, days: i64) -> CoreResult<RollingAverages> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let (income_total, expense_total) = income_expense_since(conn, &cutoff)?;
    let (months_with_data, data_span_days) = data_coverage_since(conn, &cutoff)?;
    let months = months_in_span(months_with_data, days);
    let avg_income = income_total / months;
    let avg_expense = expense_total / months;
    Ok(RollingAverages {
        window_days: days,
        months,
        avg_monthly_income_cents: avg_income,
        avg_monthly_expense_cents: avg_expense,
        net_monthly_cents: avg_income - avg_expense,
        savings_rate_pct: savings_rate_pct(avg_income, avg_expense),
        data_span_days,
    })
}

/// How much history the window actually holds: the number of distinct calendar
/// months containing activity, and the days from the earliest transaction to
/// today. Returns `(0, 0)` for an empty window.
///
/// Counting distinct CALENDAR months — not elapsed days — is what makes a
/// monthly average mean what people expect it to. Three monthly paychecks span
/// only ~72 days from first to last, so an elapsed-days divisor would call that
/// 2.4 months and inflate monthly income by half. Three paychecks land in three
/// calendar months, which is also how a user would describe it: "I have three
/// months of data."
fn data_coverage_since(conn: &Connection, cutoff: &str) -> CoreResult<(i64, i64)> {
    let pred = non_investment_txn_predicate("t");
    let (months, earliest): (i64, Option<String>) = conn.query_row(
        &format!(
            "SELECT COUNT(DISTINCT strftime('%Y-%m', t.posted_at)), MIN(t.posted_at) \
             FROM transactions t \
             WHERE t.posted_at >= ?1 AND t.is_transfer = 0 AND {pred}"
        ),
        params![cutoff],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let span_days = earliest
        .and_then(|e| chrono::DateTime::parse_from_rfc3339(&e).ok())
        .map(|first| {
            (chrono::Utc::now() - first.with_timezone(&chrono::Utc))
                .num_days()
                .max(0)
        })
        .unwrap_or(0);
    Ok((months, span_days))
}

/// Months to divide by: the calendar months that actually hold activity, capped
/// at the window and floored at one. An empty window falls back to the window's
/// nominal length so a zero-activity account reports a $0 average rather than
/// dividing by zero.
fn months_in_span(months_with_data: i64, window_days: i64) -> i64 {
    let window_months = (window_days / 30).max(1);
    if months_with_data <= 0 {
        return window_months;
    }
    months_with_data.clamp(1, window_months)
}

// ── Balance breakdown ───────────────────────────────────────────────────────

/// Liquid / invested / debt / emergency-fund splits plus net worth. Computed
/// only from accounts with a confirmed balance (`balance_known`); accounts whose
/// balance is unknown are surfaced via `accounts_with_unknown_balance` rather
/// than counted as a phantom $0. Net worth is delegated to
/// [`net_worth::breakdown`] so there is exactly one net-worth definition too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BalanceBreakdown {
    /// Cash and near-cash (non-debt, non-investment), summed with sign.
    pub liquid_cents: i64,
    /// Investment/brokerage/retirement balances.
    pub invested_cents: i64,
    /// Magnitude of debt owed on Credit/Loan accounts (>= 0).
    pub debt_cents: i64,
    /// Balance of emergency-fund-eligible, non-debt accounts — the pool
    /// emergency-fund coverage is measured against.
    pub emergency_fund_cents: i64,
    /// Net worth (known account balances with debt as negatives + manual assets).
    pub net_worth_cents: i64,
    pub accounts_with_unknown_balance: i64,
}

pub fn balance_breakdown(conn: &mut Connection) -> CoreResult<BalanceBreakdown> {
    let summaries = accounts::list_summaries(conn)?;
    let net_worth_cents = net_worth::breakdown(conn)?.net_worth_cents;

    let mut out = BalanceBreakdown {
        net_worth_cents,
        ..Default::default()
    };
    for a in &summaries {
        if !a.balance_known {
            out.accounts_with_unknown_balance += 1;
            continue;
        }
        if is_debt_type(a.r#type) {
            // Debt is stored as a negative balance; report the magnitude owed.
            if a.balance_cents < 0 {
                out.debt_cents += -a.balance_cents;
            }
        } else if is_investment_type(a.r#type) {
            out.invested_cents += a.balance_cents;
        } else {
            out.liquid_cents += a.balance_cents;
        }
        if a.emergency_fund_eligible && !is_debt_type(a.r#type) {
            out.emergency_fund_cents += a.balance_cents;
        }
    }
    Ok(out)
}

// ── Per-member attribution ──────────────────────────────────────────────────
//
// A member's share of an account is `1 / owner_count`: a sole account is fully
// theirs (1.0), a 2-owner JOINT account splits equally (0.5 each), matching the
// V038 "shares are equal in v1" model. Accounts with NO owner belong to the
// household, not any member, so they carry zero member weight.
//
// The reconciliation contract this yields — and the one the tests pin — is:
//   Σ(every member's slice) + unassigned_residual == household total
// It is NOT `member_a + member_b == household` on its own; the ownerless
// accounts are a distinct residual bucket. And because joint shares are
// fractional cents, per-member cents reconcile to the household total only up to
// rounding (≤ 1 cent per joint account per aggregate) — round at the display
// edge, not mid-reconciliation.

/// The ONE definition of a member's per-account ownership weight, shared by
/// balance attribution ([`account_weights_for_member`]), flow attribution
/// ([`weighted_income_expense`]), and the Copilot's per-member breakdown, so they
/// can never drift. The weight is the member's explicit `share_bps` (basis
/// points, 10000 = 100%) when set, else an equal split (`1 / owner_count`) — so
/// accounts with no explicit share behave exactly as before. Selects
/// `(account_id, weight)` for the member bound to `?1` — callers must supply the
/// member id as the first parameter.
pub const MEMBER_WEIGHT_SUBQUERY: &str = "SELECT ao.account_id, \
       COALESCE(ao.share_bps / 10000.0, 1.0 / oc.n) AS weight \
     FROM account_owners ao \
     JOIN (SELECT account_id, COUNT(*) AS n FROM account_owners GROUP BY account_id) oc \
       ON oc.account_id = ao.account_id \
     WHERE ao.member_id = ?1";

/// Per-account ownership weight for `member_id` (`1 / owner_count`). Accounts the
/// member does not own are absent from the map.
fn account_weights_for_member(
    conn: &Connection,
    member_id: &str,
) -> CoreResult<std::collections::HashMap<String, f64>> {
    let mut stmt = conn.prepare(MEMBER_WEIGHT_SUBQUERY)?;
    let rows = stmt.query_map(params![member_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
    })?;
    let mut out = std::collections::HashMap::new();
    for r in rows {
        let (account_id, weight) = r?;
        out.insert(account_id, weight);
    }
    Ok(out)
}

/// The manual-asset analogue of [`MEMBER_WEIGHT_SUBQUERY`]: a member's share of a
/// jointly-owned asset — explicit `share_bps` when set, else an equal split.
/// Selects `(asset_id, weight)` for the member bound to `?1`.
pub const MEMBER_ASSET_WEIGHT_SUBQUERY: &str = "SELECT ao.asset_id, \
       COALESCE(ao.share_bps / 10000.0, 1.0 / oc.n) AS weight \
     FROM asset_owners ao \
     JOIN (SELECT asset_id, COUNT(*) AS n FROM asset_owners GROUP BY asset_id) oc \
       ON oc.asset_id = ao.asset_id \
     WHERE ao.member_id = ?1";

/// Per-asset ownership weight for `member_id`. Assets the member does not own are
/// absent from the map.
fn asset_weights_for_member(
    conn: &Connection,
    member_id: &str,
) -> CoreResult<std::collections::HashMap<String, f64>> {
    let mut stmt = conn.prepare(MEMBER_ASSET_WEIGHT_SUBQUERY)?;
    let rows = stmt.query_map(params![member_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
    })?;
    let mut out = std::collections::HashMap::new();
    for r in rows {
        let (asset_id, weight) = r?;
        out.insert(asset_id, weight);
    }
    Ok(out)
}

/// Income and expense (both positive cents), transfers excluded, attributed to
/// one member by ownership weight over `[start, end)` (end optional = open).
fn weighted_income_expense(
    conn: &Connection,
    start_inclusive: &str,
    end_exclusive: Option<&str>,
    member_id: &str,
) -> CoreResult<(i64, i64)> {
    // Per-transaction weight for this member: an explicit per-transaction owner
    // (`owner_member_id`) attributes the WHOLE transaction to that one member —
    // overriding the account's ownership share for that row (a personal purchase
    // on a joint card) — otherwise the account share applies. A LEFT JOIN (not
    // INNER) so an overridden transaction on an account the member doesn't own by
    // share is still counted 100% for them. The member id binds to ?1 in both the
    // weight subquery and the override.
    let sql = format!(
        "SELECT \
            COALESCE(SUM(CASE WHEN t.amount_cents > 0 AND t.settle_up = 0 THEN t.amount_cents * t.mw ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents * t.mw \
                              WHEN t.amount_cents < 0 THEN -t.amount_cents * t.mw \
                              ELSE 0 END), 0) \
         FROM ( \
             SELECT t.amount_cents AS amount_cents, t.settle_up AS settle_up, \
                    CASE WHEN t.owner_member_id IS NOT NULL \
                         THEN (CASE WHEN t.owner_member_id = ?1 THEN 1.0 ELSE 0.0 END) \
                         ELSE COALESCE(w.weight, 0.0) END AS mw \
             FROM transactions t \
             LEFT JOIN ({MEMBER_WEIGHT_SUBQUERY}) w ON w.account_id = t.account_id \
             WHERE t.posted_at >= ?2 AND t.is_transfer = 0 \
               AND {pred}{end} \
         ) t",
        pred = non_investment_txn_predicate("t"),
        end = if end_exclusive.is_some() {
            " AND t.posted_at < ?3"
        } else {
            ""
        }
    );
    let (inc, exp): (f64, f64) = if let Some(end) = end_exclusive {
        conn.query_row(&sql, params![member_id, start_inclusive, end], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
    } else {
        conn.query_row(&sql, params![member_id, start_inclusive], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
    };
    Ok((inc.round() as i64, exp.round() as i64))
}

/// [`income_expense_since`] optionally scoped to one member (`None` = household,
/// running the existing unweighted query verbatim).
pub fn income_expense_since_for(
    conn: &Connection,
    start_inclusive: &str,
    member_id: Option<&str>,
) -> CoreResult<(i64, i64)> {
    match member_id {
        None => income_expense_since(conn, start_inclusive),
        Some(m) => weighted_income_expense(conn, start_inclusive, None, m),
    }
}

/// [`income_expense_between`] optionally scoped to one member.
pub fn income_expense_between_for(
    conn: &Connection,
    start_inclusive: &str,
    end_exclusive: &str,
    member_id: Option<&str>,
) -> CoreResult<(i64, i64)> {
    match member_id {
        None => income_expense_between(conn, start_inclusive, end_exclusive),
        Some(m) => weighted_income_expense(conn, start_inclusive, Some(end_exclusive), m),
    }
}

/// [`cashflow_since`] optionally scoped to one member.
pub fn cashflow_since_for(
    conn: &Connection,
    start_inclusive: &str,
    member_id: Option<&str>,
) -> CoreResult<Cashflow> {
    let (income, expense) = income_expense_since_for(conn, start_inclusive, member_id)?;
    Ok(Cashflow::from_income_expense(income, expense))
}

/// [`cashflow_between`] optionally scoped to one member.
pub fn cashflow_between_for(
    conn: &Connection,
    start_inclusive: &str,
    end_exclusive: &str,
    member_id: Option<&str>,
) -> CoreResult<Cashflow> {
    let (income, expense) = income_expense_between_for(conn, start_inclusive, end_exclusive, member_id)?;
    Ok(Cashflow::from_income_expense(income, expense))
}

/// [`rolling_averages`] optionally scoped to one member.
pub fn rolling_averages_for(
    conn: &Connection,
    days: i64,
    member_id: Option<&str>,
) -> CoreResult<RollingAverages> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let (income_total, expense_total) = income_expense_since_for(conn, &cutoff, member_id)?;
    // Household-wide span deliberately, not the member's own first transaction:
    // a member who joined recently should not have their averages inflated by a
    // short personal span when the household has months of history behind it.
    let (months_with_data, data_span_days) = data_coverage_since(conn, &cutoff)?;
    let months = months_in_span(months_with_data, days);
    let avg_income = income_total / months;
    let avg_expense = expense_total / months;
    Ok(RollingAverages {
        window_days: days,
        months,
        avg_monthly_income_cents: avg_income,
        avg_monthly_expense_cents: avg_expense,
        net_monthly_cents: avg_income - avg_expense,
        savings_rate_pct: savings_rate_pct(avg_income, avg_expense),
        data_span_days,
    })
}

/// [`balance_breakdown`] optionally scoped to one member. Each account balance
/// AND each jointly-owned manual asset is weighted by the member's ownership
/// share; ownerless accounts and ownerless assets stay in the household residual
/// (never attributed to a member). So per-member `net_worth_cents` is the
/// member's owned share of accounts + assets, and the members' slices plus the
/// residual reconcile to the household total.
pub fn balance_breakdown_for(
    conn: &mut Connection,
    member_id: Option<&str>,
) -> CoreResult<BalanceBreakdown> {
    let Some(member) = member_id else {
        return balance_breakdown(conn);
    };
    let weights = account_weights_for_member(conn, member)?;
    let summaries = accounts::list_summaries(conn)?;
    let (mut liquid, mut invested, mut debt, mut ef, mut net) = (0f64, 0f64, 0f64, 0f64, 0f64);
    let mut unknown = 0i64;
    for a in &summaries {
        let Some(&weight) = weights.get(&a.id) else {
            continue; // not owned by this member
        };
        if !a.balance_known {
            unknown += 1;
            continue;
        }
        let bal = a.balance_cents as f64 * weight;
        if is_debt_type(a.r#type) {
            if a.balance_cents < 0 {
                debt += -bal;
            }
            net += bal;
        } else if is_investment_type(a.r#type) {
            invested += bal;
            net += bal;
        } else {
            liquid += bal;
            net += bal;
        }
        if a.emergency_fund_eligible && !is_debt_type(a.r#type) {
            ef += bal;
        }
    }
    // Fold in the member's share of jointly-owned manual assets. Assets aren't
    // liquid/invested/debt — they only move net worth — matching how the
    // household breakdown folds in manual_asset_cents. An asset with no owner
    // stays in the household residual, exactly like an ownerless account.
    let asset_weights = asset_weights_for_member(conn, member)?;
    if !asset_weights.is_empty() {
        for asset in crate::repos::manual_assets::list(conn)? {
            if let Some(&w) = asset_weights.get(&asset.id) {
                net += asset.value_cents as f64 * w;
            }
        }
    }
    Ok(BalanceBreakdown {
        liquid_cents: liquid.round() as i64,
        invested_cents: invested.round() as i64,
        debt_cents: debt.round() as i64,
        emergency_fund_cents: ef.round() as i64,
        net_worth_cents: net.round() as i64,
        accounts_with_unknown_balance: unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, models::NewAccount, repos::accounts, Db};
    use chrono::{Duration, Utc};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("metrics.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn account(name: &str, ty: AccountType, opening: i64, ef_eligible: bool) -> NewAccount {
        NewAccount {
            owner: "me".into(),
            bank: "Bank".into(),
            r#type: ty,
            name: name.into(),
            last4: None,
            currency: "USD".into(),
            color: "#3B82F6".into(),
            opening_balance_cents: opening,
            source: "manual".into(),
            liquidity_type: "liquid".into(),
            emergency_fund_eligible: ef_eligible,
            goal_earmark: None,
            apy_pct: None,
            simplefin_account_id: None,
            nickname: None,
            connection_id: None,
            institution_id: None,
            external_account_id: None,
            official_name: None,
            mask: None,
            subtype: None,
            account_group: "cash".into(),
            available_balance_cents: None,
            balance_date: None,
            extra_json: None,
            raw_json: None,
            import_pending: false,
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        }
    }

    fn insert_txn(conn: &mut Connection, acct: &str, amount: i64, days_ago: i64, transfer: bool) {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at) \
             VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', 0, ?5, ?3)",
            params![
                id,
                acct,
                (Utc::now() - Duration::days(days_ago)).to_rfc3339(),
                amount,
                if transfer { 1 } else { 0 },
            ],
        )
        .unwrap();
    }

    #[test]
    fn savings_rate_is_signed_and_guards_zero_income() {
        assert_eq!(savings_rate_pct(0, 500), 0, "no income → 0, not a divide by zero");
        assert_eq!(savings_rate_pct(1000, 200), 80);
        assert_eq!(savings_rate_pct(1000, 1500), -50, "deficit is negative, not clamped");
    }

    #[test]
    fn emergency_fund_months_caps_and_guards() {
        assert_eq!(emergency_fund_months(100_000, 0), 0.0);
        assert_eq!(emergency_fund_months(300_000, 100_000), 3.0);
        assert_eq!(emergency_fund_months(100_000_000, 100_000), EMERGENCY_FUND_MONTHS_CAP);
    }

    #[test]
    fn type_classifiers_split_correctly() {
        assert!(is_liquid_type(AccountType::Checking));
        assert!(is_liquid_type(AccountType::Savings));
        assert!(is_investment_type(AccountType::Investment));
        assert!(is_debt_type(AccountType::Credit));
        assert!(is_debt_type(AccountType::Loan));
        assert!(!is_liquid_type(AccountType::Loan));
    }

    #[test]
    fn income_expense_excludes_transfers() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acct = accounts::insert(&mut conn, account("Checking", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        insert_txn(&mut conn, &acct, 300_000, 5, false); // income
        insert_txn(&mut conn, &acct, -100_000, 5, false); // expense
        insert_txn(&mut conn, &acct, -500_000, 5, true); // transfer out — must be ignored
        insert_txn(&mut conn, &acct, 500_000, 5, true); // transfer in — must be ignored

        let (income, expense) = income_expense_since(&conn, "1970-01-01T00:00:00Z").unwrap();
        assert_eq!(income, 300_000);
        assert_eq!(expense, 100_000);
        assert_eq!(savings_rate_pct(income, expense), 66);
    }

    #[test]
    fn settle_up_inflow_nets_against_expense_never_income() {
        // Settle-up (person-to-person reimbursement) rows never count as income;
        // an inflow nets back against expense instead. Joe: we paid out $11,475
        // on his behalf, he paid back $3,000 — net expense $8,475, income $0.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let chk = accounts::insert(&mut conn, account("Checking", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        let insert_settle_up = |conn: &Connection, amount: i64| {
            conn.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                          status, is_anomaly, is_transfer, created_at, settle_up) \
                 VALUES(?1, ?2, '2026-05-15T00:00:00Z', ?3, 'e-transfer joe', 'cleared', 0, 0, \
                        '2026-05-15T00:00:00Z', 1)",
                params![uuid::Uuid::new_v4().to_string(), chk, amount],
            )
            .unwrap();
        };
        insert_settle_up(&conn, -1_147_500); // 'o' — we paid out on Joe's behalf
        insert_settle_up(&conn, 300_000); // 'i' — Joe paid us back

        let (income, expense) = income_expense_since(&conn, "2026-05-01T00:00:00Z").unwrap();
        assert_eq!(income, 0, "settle-up inflow is never income");
        assert_eq!(expense, 847_500, "settle-up nets: 11,475 - 3,000 = 8,475");
    }

    #[test]
    fn income_expense_excludes_investment_account_activity() {
        // A TFSA/brokerage import brings BUY/SELL trades and contribution
        // inflows; none of it is living income or spending, and a single trade
        // must not swing the savings rate.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let chk = accounts::insert(&mut conn, account("Chq", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        let tfsa = accounts::insert(&mut conn, account("TFSA", AccountType::Investment, 0, false))
            .unwrap()
            .id;
        insert_txn(&mut conn, &chk, 300_000, 5, false); // real income
        insert_txn(&mut conn, &chk, -100_000, 5, false); // real expense
        insert_txn(&mut conn, &tfsa, 20_000, 5, false); // contribution arriving
        insert_txn(&mut conn, &tfsa, -12_260, 5, false); // BUY trade

        let (income, expense) = income_expense_since(&conn, "1970-01-01T00:00:00Z").unwrap();
        assert_eq!(income, 300_000, "TFSA inflows are not income");
        assert_eq!(expense, 100_000, "trades are not spending");

        // The per-member weighted path applies the same exclusion.
        use crate::repos::household;
        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        household::set_account_owners(&mut conn, &chk, &[alice.id.clone()]).unwrap();
        household::set_account_owners(&mut conn, &tfsa, &[alice.id.clone()]).unwrap();
        let (m_inc, m_exp) =
            income_expense_since_for(&conn, "1970-01-01T00:00:00Z", Some(&alice.id)).unwrap();
        assert_eq!(m_inc, 300_000);
        assert_eq!(m_exp, 100_000);
    }

    #[test]
    fn income_expense_includes_dividend_interest_tax_in_investment_accounts() {
        // The blanket investment-account exclusion above must not swallow a
        // dividend/interest payment (real income the moment it lands) or its
        // withholding tax (a real expense) — only Trade/MoneyMovement rows are
        // wealth-shuffling. An untyped row (no activity_type set — a manual
        // entry) stays excluded, matching the account-level rule.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let tfsa = accounts::insert(&mut conn, account("TFSA", AccountType::Investment, 0, false))
            .unwrap()
            .id;
        let insert_activity = |conn: &Connection, amount: i64, activity_type: &str| {
            conn.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                          status, is_anomaly, is_transfer, created_at, activity_type) \
                 VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', 0, 0, ?3, ?5)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    tfsa,
                    Utc::now().to_rfc3339(),
                    amount,
                    activity_type,
                ],
            )
            .unwrap();
        };
        insert_activity(&conn, 500, "Dividend");
        insert_activity(&conn, 10, "Interest");
        insert_activity(&conn, -20, "Tax");
        insert_txn(&mut conn, &tfsa, 100, 0, false); // untyped — stays excluded

        let (income, expense) = income_expense_since(&conn, "1970-01-01T00:00:00Z").unwrap();
        assert_eq!(income, 510, "dividends + interest count as real income");
        assert_eq!(expense, 20, "withholding tax counts as a real expense");
    }

    #[test]
    fn non_investment_predicate_stays_in_lockstep_with_is_investment_type() {
        // The SQL predicate names the DB type strings directly; if AccountType
        // gains a new investment-classified variant, this test must fail until
        // the predicate learns it too.
        for ty in [
            AccountType::Checking,
            AccountType::Savings,
            AccountType::Credit,
            AccountType::Investment,
            AccountType::Cash,
            AccountType::Loan,
            AccountType::Other,
        ] {
            let in_sql = non_investment_txn_predicate("t").contains(&format!("'{}'", ty.as_db()));
            assert_eq!(
                in_sql,
                is_investment_type(ty),
                "predicate and is_investment_type disagree on {ty:?}"
            );
        }
    }

    #[test]
    fn per_member_flows_reconcile_to_household_with_joint_split() {
        use crate::repos::household;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        let bob = household::create_member(&mut conn, "Bob", None).unwrap();

        let a_sole = accounts::insert(&mut conn, account("A", AccountType::Checking, 0, true)).unwrap().id;
        let b_sole = accounts::insert(&mut conn, account("B", AccountType::Checking, 0, true)).unwrap().id;
        let joint = accounts::insert(&mut conn, account("J", AccountType::Savings, 0, true)).unwrap().id;
        let shared = accounts::insert(&mut conn, account("U", AccountType::Checking, 0, true)).unwrap().id;

        household::set_account_owners(&mut conn, &a_sole, &[alice.id.clone()]).unwrap();
        household::set_account_owners(&mut conn, &b_sole, &[bob.id.clone()]).unwrap();
        household::set_account_owners(&mut conn, &joint, &[alice.id.clone(), bob.id.clone()]).unwrap();
        // `shared` is left unassigned (0 owners) → the household residual.

        insert_txn(&mut conn, &a_sole, 300_000, 5, false);
        insert_txn(&mut conn, &a_sole, -100_000, 5, false);
        insert_txn(&mut conn, &b_sole, 200_000, 5, false);
        insert_txn(&mut conn, &b_sole, -50_000, 5, false);
        insert_txn(&mut conn, &joint, 100_000, 5, false);
        insert_txn(&mut conn, &joint, -40_000, 5, false);
        insert_txn(&mut conn, &shared, 70_000, 5, false);
        insert_txn(&mut conn, &shared, -30_000, 5, false);
        // A transfer in the joint account must be ignored on every slice.
        insert_txn(&mut conn, &joint, 999_999, 5, true);

        let start = "1970-01-01T00:00:00Z";
        // None path is the existing unweighted query verbatim.
        let (h_inc, h_exp) = income_expense_since_for(&conn, start, None).unwrap();
        assert_eq!(
            (h_inc, h_exp),
            income_expense_since(&conn, start).unwrap(),
            "None path == household verbatim"
        );
        assert_eq!(h_inc, 670_000);
        assert_eq!(h_exp, 220_000);

        let (a_inc, a_exp) = income_expense_since_for(&conn, start, Some(&alice.id)).unwrap();
        let (b_inc, b_exp) = income_expense_since_for(&conn, start, Some(&bob.id)).unwrap();
        // Joint account split equally (50k income / 20k expense each).
        assert_eq!(a_inc, 350_000, "alice: 300k sole + 50k half-joint");
        assert_eq!(a_exp, 120_000);
        assert_eq!(b_inc, 250_000, "bob: 200k sole + 50k half-joint");
        assert_eq!(b_exp, 70_000);

        // Reconciliation contract: members + unassigned residual == household.
        let (u_inc, u_exp) = (70_000, 30_000); // ownerless `shared` account
        assert_eq!(a_inc + b_inc + u_inc, h_inc, "income reconciles with residual");
        assert_eq!(a_exp + b_exp + u_exp, h_exp, "expense reconciles with residual");

        // rolling_averages_for threads the same filter. This fixture's activity
        // sits in ONE calendar month, so Alice's $3,500 is $3,500/month — the
        // old `/ 3` here was the fixed-divisor bug baked into an assertion,
        // which is how it survived: it reported her month as a third of itself.
        let r_alice = rolling_averages_for(&conn, 90, Some(&alice.id)).unwrap();
        assert_eq!(r_alice.months, 1, "the fixture spans a single calendar month");
        assert_eq!(r_alice.avg_monthly_income_cents, 350_000);
    }

    #[test]
    fn per_member_balances_split_within_fractional_cent_tolerance() {
        use crate::repos::household;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        let bob = household::create_member(&mut conn, "Bob", None).unwrap();

        let a_sole = accounts::insert(&mut conn, account("A", AccountType::Checking, 40_000, true)).unwrap().id;
        // Odd-cent joint balance: halves are 50_000.5 → round away from zero → 50_001
        // each, so the two slices exceed the whole by 1 cent (per joint account).
        let joint = accounts::insert(&mut conn, account("J", AccountType::Savings, 100_001, true)).unwrap().id;
        household::set_account_owners(&mut conn, &a_sole, &[alice.id.clone()]).unwrap();
        household::set_account_owners(&mut conn, &joint, &[alice.id.clone(), bob.id.clone()]).unwrap();

        // None path == existing household breakdown verbatim.
        let household_bd = balance_breakdown_for(&mut conn, None).unwrap();
        assert_eq!(household_bd, balance_breakdown(&mut conn).unwrap());
        assert_eq!(household_bd.liquid_cents, 140_001);

        let a = balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap();
        let b = balance_breakdown_for(&mut conn, Some(&bob.id)).unwrap();
        assert_eq!(b.liquid_cents, 50_001, "bob: half of the odd joint balance");
        assert_eq!(a.liquid_cents, 90_001, "alice: 40k sole + half joint");
        // Reconciles to the household total up to ≤ 1 cent per joint account.
        let joint_accounts = 1;
        assert!(
            (a.liquid_cents + b.liquid_cents - household_bd.liquid_cents).abs() <= joint_accounts,
            "member balances reconcile to household within fractional-cent tolerance"
        );
        // A member's net worth is their owned-account net worth (no ownerless assets).
        assert_eq!(a.net_worth_cents, 90_001);
    }

    #[test]
    fn explicit_share_bps_attributes_balances_by_share() {
        use crate::repos::household;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        let bob = household::create_member(&mut conn, "Bob", None).unwrap();
        let joint =
            accounts::insert(&mut conn, account("J", AccountType::Checking, 100_000, true)).unwrap().id;
        household::set_account_owners(&mut conn, &joint, &[alice.id.clone(), bob.id.clone()]).unwrap();
        let share = |conn: &Connection, m: &str, bps: i64| {
            conn.execute(
                "UPDATE account_owners SET share_bps = ?3 WHERE account_id = ?1 AND member_id = ?2",
                rusqlite::params![joint, m, bps],
            )
            .unwrap();
        };

        // NULL share_bps ⇒ equal split, exactly as before this feature existed.
        assert_eq!(
            balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap().liquid_cents,
            50_000,
            "NULL share_bps ⇒ equal split (backward compatible)"
        );

        // Explicit 70/30 attributes 70/30 and still reconciles to household.
        share(&conn, &alice.id, 7000);
        share(&conn, &bob.id, 3000);
        let a = balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap();
        let b = balance_breakdown_for(&mut conn, Some(&bob.id)).unwrap();
        let h = balance_breakdown_for(&mut conn, None).unwrap();
        assert_eq!(a.liquid_cents, 70_000, "alice owns 70%");
        assert_eq!(b.liquid_cents, 30_000, "bob owns 30%");
        assert_eq!(a.liquid_cents + b.liquid_cents, h.liquid_cents, "70 + 30 == household");

        // Cross-app: an operator can own <100% recorded here — the rest is the
        // residual (owned by people who run their own separate app). Drop bob and
        // give alice a 30% share: her slice is 30%, the other 70% is never
        // attributed here and so is never double-counted across apps.
        conn.execute(
            "DELETE FROM account_owners WHERE account_id = ?1 AND member_id = ?2",
            rusqlite::params![joint, bob.id],
        )
        .unwrap();
        share(&conn, &alice.id, 3000);
        assert_eq!(
            balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap().liquid_cents,
            30_000,
            "sole-recorded owner with a 30% share attributes only 30%; 70% is the cross-app residual"
        );
    }

    #[test]
    fn explicit_share_bps_attributes_flows_by_share() {
        use crate::repos::household;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        let bob = household::create_member(&mut conn, "Bob", None).unwrap();
        let joint =
            accounts::insert(&mut conn, account("J", AccountType::Checking, 0, true)).unwrap().id;
        household::set_account_owners(&mut conn, &joint, &[alice.id.clone(), bob.id.clone()]).unwrap();
        conn.execute(
            "UPDATE account_owners SET share_bps = 7000 WHERE account_id = ?1 AND member_id = ?2",
            rusqlite::params![joint, alice.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE account_owners SET share_bps = 3000 WHERE account_id = ?1 AND member_id = ?2",
            rusqlite::params![joint, bob.id],
        )
        .unwrap();

        insert_txn(&mut conn, &joint, 100_000, 5, false);
        insert_txn(&mut conn, &joint, -50_000, 5, false);
        // Shares weight flows too (documented decision): alice gets 70% of both.
        let (a_inc, a_exp) =
            income_expense_since_for(&conn, "1970-01-01T00:00:00Z", Some(&alice.id)).unwrap();
        let (b_inc, b_exp) =
            income_expense_since_for(&conn, "1970-01-01T00:00:00Z", Some(&bob.id)).unwrap();
        assert_eq!((a_inc, a_exp), (70_000, 35_000), "alice: 70% of joint flows");
        assert_eq!((b_inc, b_exp), (30_000, 15_000), "bob: 30% of joint flows");
        // Reconciles to the household total.
        let (h_inc, h_exp) = income_expense_since(&conn, "1970-01-01T00:00:00Z").unwrap();
        assert_eq!(a_inc + b_inc, h_inc);
        assert_eq!(a_exp + b_exp, h_exp);
    }

    #[test]
    fn member_net_worth_folds_in_owned_manual_asset_shares() {
        use crate::repos::household;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        let bob = household::create_member(&mut conn, "Bob", None).unwrap();
        // A $500,000 house jointly owned 60/40 — the "shared assets" case.
        conn.execute(
            "INSERT INTO manual_assets(id,name,asset_type,value_cents,currency,created_at,updated_at) \
             VALUES('house','House','Real Estate',50000000,'CAD','2024-01-01','2024-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO asset_owners(asset_id,member_id,share_bps) VALUES('house',?1,6000)",
            rusqlite::params![alice.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO asset_owners(asset_id,member_id,share_bps) VALUES('house',?1,4000)",
            rusqlite::params![bob.id],
        )
        .unwrap();

        let a = balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap();
        let b = balance_breakdown_for(&mut conn, Some(&bob.id)).unwrap();
        assert_eq!(a.net_worth_cents, 30_000_000, "alice: 60% of the house");
        assert_eq!(b.net_worth_cents, 20_000_000, "bob: 40% of the house");
        // Reconciles to the household net worth (which folds in the whole asset).
        let h = balance_breakdown(&mut conn).unwrap();
        assert_eq!(
            a.net_worth_cents + b.net_worth_cents,
            h.net_worth_cents,
            "60 + 40 == the household's whole-asset net worth"
        );

        // NULL share ⇒ equal split, and an ownerless asset stays in the residual.
        conn.execute("UPDATE asset_owners SET share_bps = NULL", []).unwrap();
        assert_eq!(
            balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap().net_worth_cents,
            25_000_000,
            "NULL share ⇒ 50/50"
        );
        conn.execute("DELETE FROM asset_owners", []).unwrap();
        assert_eq!(
            balance_breakdown_for(&mut conn, Some(&alice.id)).unwrap().net_worth_cents,
            0,
            "ownerless asset attributes to no member (household residual)"
        );
    }

    #[test]
    fn per_transaction_owner_override_attributes_the_whole_txn_to_one_member() {
        use crate::repos::household;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let alice = household::create_member(&mut conn, "Alice", None).unwrap();
        let bob = household::create_member(&mut conn, "Bob", None).unwrap();
        let joint =
            accounts::insert(&mut conn, account("J", AccountType::Checking, 0, true)).unwrap().id;
        household::set_account_owners(&mut conn, &joint, &[alice.id.clone(), bob.id.clone()]).unwrap();

        // A shared $1,000 expense (no override) splits 50/50 by account share.
        insert_txn(&mut conn, &joint, -100_000, 5, false);
        // Alice's personal $400 purchase on the joint card → 100% hers (override),
        // even though the account is jointly owned 50/50.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,is_transfer,created_at,owner_member_id) \
             VALUES('t_alice',?1,'2025-01-01T00:00:00Z',-40000,'M','cleared',0,0,'2025-01-01T00:00:00Z',?2)",
            rusqlite::params![joint, alice.id],
        )
        .unwrap();

        let start = "1970-01-01T00:00:00Z";
        let (_a_inc, a_exp) = income_expense_since_for(&conn, start, Some(&alice.id)).unwrap();
        let (_b_inc, b_exp) = income_expense_since_for(&conn, start, Some(&bob.id)).unwrap();
        assert_eq!(a_exp, 90_000, "alice: half the shared $1,000 + all of her own $400");
        assert_eq!(b_exp, 50_000, "bob: only half the shared (0 of alice's override)");
        // Still reconciles to the household total ($1,400).
        let (_h_inc, h_exp) = income_expense_since(&conn, start).unwrap();
        assert_eq!(a_exp + b_exp, h_exp, "member expenses reconcile to the household");
    }

    #[test]
    fn balance_breakdown_classifies_by_type_not_sign() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // Overdrawn checking (negative but still liquid).
        accounts::insert(&mut conn, account("Checking", AccountType::Checking, -5_000, true)).unwrap();
        accounts::insert(&mut conn, account("HISA", AccountType::Savings, 500_000, true)).unwrap();
        accounts::insert(&mut conn, account("Brokerage", AccountType::Investment, 1_000_000, false)).unwrap();
        // Credit-card debt (negative) — debt, not "liquid negative".
        accounts::insert(&mut conn, account("Card", AccountType::Credit, -120_000, false)).unwrap();

        let b = balance_breakdown(&mut conn).unwrap();
        assert_eq!(b.liquid_cents, 495_000, "overdrawn checking reduces liquid, HISA adds");
        assert_eq!(b.invested_cents, 1_000_000);
        assert_eq!(b.debt_cents, 120_000, "credit-card magnitude owed");
        assert_eq!(
            b.emergency_fund_cents, 495_000,
            "only ef-eligible non-debt accounts (checking + HISA)"
        );
        // Net worth folds debt in as a negative: -5,000 + 500,000 + 1,000,000 - 120,000.
        assert_eq!(b.net_worth_cents, 1_375_000);
    }

    #[test]
    fn rolling_averages_divide_window_into_months() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acct = accounts::insert(&mut conn, account("Checking", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        // Three months of $3,000 income and $1,000 expense.
        for m in 0..3 {
            insert_txn(&mut conn, &acct, 300_000, 10 + m * 30, false);
            insert_txn(&mut conn, &acct, -100_000, 12 + m * 30, false);
        }
        let avg = rolling_averages(&conn, 90).unwrap();
        assert_eq!(avg.months, 3);
        assert_eq!(avg.avg_monthly_income_cents, 300_000);
        assert_eq!(avg.avg_monthly_expense_cents, 100_000);
        assert_eq!(avg.net_monthly_cents, 200_000);
        assert!(avg.savings_rate_pct >= 66);
    }

    /// A brand-new user has less history than the window. Dividing by a fixed
    /// 3 months understates their burn, which OVERSTATES every safety metric
    /// derived from it — runway and emergency-fund coverage both come out ~3x
    /// too generous. This is the dangerous direction: the app tells someone
    /// they are safer than they are, on day one, when they are least able to
    /// spot it.
    #[test]
    fn rolling_averages_do_not_dilute_a_short_history() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acct = accounts::insert(&mut conn, account("Checking", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        // ONE month of history: $3,000 in, $2,000 out. The user's real monthly
        // burn is $2,000 — not $666.
        insert_txn(&mut conn, &acct, 300_000, 10, false);
        insert_txn(&mut conn, &acct, -200_000, 12, false);

        let avg = rolling_averages(&conn, 90).unwrap();

        assert_eq!(
            avg.avg_monthly_expense_cents, 200_000,
            "one month of data must read as one month of burn, not a third of it"
        );
        // A $6,000 emergency fund is 3 months at $2,000/mo — not 9.
        let months = emergency_fund_months(600_000, avg.avg_monthly_expense_cents);
        assert!(
            (months - 3.0).abs() < 0.01,
            "EF coverage came out {months} months; diluted burn overstates safety"
        );
    }

    /// Monthly cadence is why the divisor counts calendar months rather than
    /// elapsed days: N monthly paychecks span only ~(N-1)x30 days, so an
    /// elapsed-days divisor would inflate monthly income by roughly half.
    #[test]
    fn rolling_averages_count_calendar_months_not_elapsed_days() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acct = accounts::insert(&mut conn, account("Checking", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        // Three monthly paychecks: ~72 days from first to last, but three
        // calendar months of activity.
        for m in 0..3 {
            insert_txn(&mut conn, &acct, 300_000, 10 + m * 30, false);
        }

        let avg = rolling_averages(&conn, 90).unwrap();

        assert_eq!(avg.months, 3, "three monthly paychecks are three months");
        assert_eq!(
            avg.avg_monthly_income_cents, 300_000,
            "a $3,000 monthly paycheck must not read as $4,500"
        );
    }

    /// Under a month there is no honest monthly figure. Extrapolating a single
    /// early purchase would imply an absurd burn, so the divisor floors at one
    /// month — which understates. `data_span_days` is what lets the safety
    /// metrics hedge instead of stating a number they cannot support.
    #[test]
    fn rolling_averages_report_a_thin_span_rather_than_extrapolating() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acct = accounts::insert(&mut conn, account("Checking", AccountType::Checking, 0, true))
            .unwrap()
            .id;
        insert_txn(&mut conn, &acct, -50_000, 5, false);

        let avg = rolling_averages(&conn, 90).unwrap();

        assert_eq!(avg.months, 1, "a 5-day span must not divide by a fraction");
        assert_eq!(avg.avg_monthly_expense_cents, 50_000);
        assert!(
            avg.data_span_days <= 6,
            "the thin span must be visible to callers, got {}",
            avg.data_span_days
        );
    }

    /// An empty window must not claim a zero-day span — there is no history to
    /// be thin about, and callers keying on `data_span_days` would misread it.
    #[test]
    fn rolling_averages_on_an_empty_window_fall_back_to_the_window() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();

        let avg = rolling_averages(&conn, 90).unwrap();

        assert_eq!(avg.months, 3);
        assert_eq!(avg.avg_monthly_expense_cents, 0);
        assert_eq!(avg.data_span_days, 0);
    }

    #[test]
    fn assumptions_default_then_round_trip() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        // Defaults from the Financial Freedom Framework when nothing is stored.
        let d = assumptions(&conn);
        assert_eq!(d.target_savings_rate_pct, 20);
        assert_eq!(d.emergency_fund_target_months, 6.0);
        assert_eq!(d.expected_annual_return_pct, 7.0);

        set_assumptions(
            &conn,
            &Assumptions {
                target_savings_rate_pct: 15,
                emergency_fund_target_months: 3.0,
                expected_annual_return_pct: 5.5,
            },
        )
        .unwrap();
        let got = assumptions(&conn);
        assert_eq!(got.target_savings_rate_pct, 15);
        assert_eq!(got.emergency_fund_target_months, 3.0);
        assert_eq!(got.expected_annual_return_pct, 5.5);
    }

    #[test]
    fn runway_uses_liquid_over_average_burn() {
        // 300,000 liquid at 100,000/month → ~90 days.
        assert_eq!(runway_days(300_000, 100_000), 90);
        // No burn → capped, not infinite.
        assert_eq!(runway_days(300_000, 0), forecast::RUNWAY_CAP_DAYS);
        // Empty pocket → 0 regardless of burn.
        assert_eq!(runway_days(0, 100_000), 0);
    }
}

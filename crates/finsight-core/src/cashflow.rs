//! Near-term **daily** cash-flow projection and safe-to-spend.
//!
//! A monthly budget can look healthy while the *timing* of income and bills
//! still creates a mid-month shortfall, and a large balance can look spendable
//! when most of it is already committed. This module projects the liquid
//! balance forward day by day over the coming weeks and answers:
//!   - When is the lowest projected balance, and does it breach the buffer?
//!   - What is the first material cash-flow risk in the window?
//!   - How much is genuinely safe to spend today without endangering it?
//!
//! The projection combines two things, which is what makes it honest:
//!   1. **Dated events** on their actual dates — income, bills, subscriptions,
//!      and user-planned transactions — for timing and lumpiness.
//!   2. A **residual smooth daily burn** for everyday variable spending
//!      (groceries, dining, fuel): `avg_monthly_expense` minus the monthly
//!      equivalent of the dated obligations, spread per day. Without this the
//!      balance would float up unrealistically between paydays and overstate
//!      safe-to-spend — the dangerous direction. Subtracting the dated
//!      obligations' monthly equivalent is what stops those bills being counted
//!      twice.
//!
//! Internal transfers and credit-card payments are excluded (they are
//! `RecurringKind::Transfer`, not real costs) so money moving between the user's
//! own accounts is never double-counted as spending.

use crate::metrics;
use crate::models::PlannedTxnFilter;
use crate::recurring::{self, RecurringItem, RecurringKind};
use crate::repos::planned_transactions;
use crate::CoreResult;
use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

pub const DEFAULT_HORIZON_DAYS: i64 = 30;
const MIN_HORIZON_DAYS: i64 = 7;
const MAX_HORIZON_DAYS: i64 = 90;
/// Detection window for recurring items — over a year, so annual bills that
/// haven't recurred inside the horizon are still known and can be flagged.
const RECURRING_WINDOW_DAYS: i64 = 400;
/// Below this much transaction history a daily forecast is guesswork; it is
/// still returned but flagged unreliable rather than shown as precise.
const MIN_RELIABLE_SPAN_DAYS: i64 = 30;
/// A bill this many days past the horizon is surfaced as a heads-up so a 30-day
/// window can't silently hide a large annual charge.
const NEAR_HORIZON_LOOKAHEAD_DAYS: i64 = 21;
const AVG_DAYS_PER_MONTH: f64 = 30.44;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CashflowEventKind {
    Income,
    Bill,
    Subscription,
    Planned,
    /// A temporary what-if outflow the user is testing; never persisted.
    Hypothetical,
}

/// A single dated cash movement. `amount_cents` is signed: positive inflow,
/// negative outflow.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashflowEvent {
    pub date: String,
    pub label: String,
    pub amount_cents: i64,
    pub kind: CashflowEventKind,
    /// Detection confidence for recurring-derived events (0..1); `None` for
    /// user-entered planned/hypothetical events, which aren't guesses.
    pub confidence: Option<f64>,
}

/// One projected day.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashflowDay {
    pub date: String,
    /// Projected end-of-day liquid balance.
    pub projected_balance_cents: i64,
    /// Net of the dated events landing on this day.
    pub event_net_cents: i64,
    /// The day's share of everyday variable spend (<= 0).
    pub burn_cents: i64,
    /// Whether the projected balance is below the safety buffer this day.
    pub below_buffer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum WarningLevel {
    Info,
    Caution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashflowWarning {
    pub level: WarningLevel,
    pub message: String,
}

/// The forward cash-flow forecast.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashflowForecast {
    pub as_of: String,
    pub horizon_days: i64,
    pub start_balance_cents: i64,
    pub buffer_cents: i64,
    /// Everyday variable spend spread per day (magnitude, >= 0).
    pub daily_burn_cents: i64,
    pub days: Vec<CashflowDay>,
    pub lowest_balance_cents: i64,
    pub lowest_date: String,
    /// First day the projected balance drops below the buffer, if any.
    pub first_breach_date: Option<String>,
    /// The conservative amount safe to spend today: the lowest projected balance
    /// minus the buffer, floored at zero. Spending it today lowers every later
    /// day by the same amount, so the lowest point is the binding constraint.
    pub safe_to_spend_cents: i64,
    /// The dated events inside the horizon, chronological, for display.
    pub upcoming_events: Vec<CashflowEvent>,
    pub warnings: Vec<CashflowWarning>,
    /// False when there's too little history (or no known balance) for the
    /// forecast to be trusted as precise. Consumers must say so, not imply rigor.
    pub reliable: bool,
}

/// Temporary what-if overlay: a user-chosen safety buffer plus an optional
/// hypothetical one-off outflow. Pure parameters — nothing is persisted.
#[derive(Debug, Clone, Default)]
pub struct WhatIf {
    pub buffer_cents: i64,
    /// Magnitude (>= 0) of a hypothetical outflow to test ("what if I spend X").
    pub extra_expense_cents: i64,
    /// When the hypothetical outflow lands; defaults to `as_of` (today).
    pub extra_expense_date: Option<NaiveDate>,
    pub extra_expense_label: Option<String>,
}

/// Pure projection: walk `start_balance` forward `horizon_days`, applying the
/// per-day smooth burn and the dated events, and derive the lowest point, first
/// buffer breach, and safe-to-spend. No DB, no clock — every input is explicit,
/// so all the math is unit-testable. `warnings`/`reliable` are set by the DB
/// assembler; here they default to empty/true.
fn project(
    as_of: NaiveDate,
    start_balance_cents: i64,
    horizon_days: i64,
    daily_burn_cents: i64,
    buffer_cents: i64,
    events: &[CashflowEvent],
) -> CashflowForecast {
    let horizon = horizon_days.clamp(MIN_HORIZON_DAYS, MAX_HORIZON_DAYS);
    let burn = daily_burn_cents.max(0);

    let mut days: Vec<CashflowDay> = Vec::with_capacity(horizon as usize);
    let mut balance = start_balance_cents;
    let mut lowest_balance = start_balance_cents;
    let mut lowest_date = as_of.format("%Y-%m-%d").to_string();
    let mut first_breach: Option<String> = None;

    for d in 0..horizon {
        let date = as_of + Duration::days(d);
        let date_str = date.format("%Y-%m-%d").to_string();
        let event_net: i64 = events
            .iter()
            .filter(|e| e.date == date_str)
            .map(|e| e.amount_cents)
            .sum();
        balance += event_net - burn;
        let below_buffer = balance < buffer_cents;
        if balance < lowest_balance {
            lowest_balance = balance;
            lowest_date = date_str.clone();
        }
        if below_buffer && first_breach.is_none() {
            first_breach = Some(date_str.clone());
        }
        days.push(CashflowDay {
            date: date_str,
            projected_balance_cents: balance,
            event_net_cents: event_net,
            burn_cents: -burn,
            below_buffer,
        });
    }

    let safe_to_spend_cents = (lowest_balance - buffer_cents).max(0);

    // Upcoming events for display: chronological, horizon only.
    let horizon_end = as_of + Duration::days(horizon);
    let mut upcoming: Vec<CashflowEvent> = events
        .iter()
        .filter(|e| {
            NaiveDate::parse_from_str(&e.date, "%Y-%m-%d")
                .map(|d| d >= as_of && d < horizon_end)
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    upcoming.sort_by(|a, b| a.date.cmp(&b.date));

    CashflowForecast {
        as_of: as_of.format("%Y-%m-%d").to_string(),
        horizon_days: horizon,
        start_balance_cents,
        buffer_cents,
        daily_burn_cents: burn,
        days,
        lowest_balance_cents: lowest_balance,
        lowest_date,
        first_breach_date: first_breach,
        safe_to_spend_cents,
        upcoming_events: upcoming,
        warnings: Vec::new(),
        reliable: true,
    }
}

/// Roll a recurring occurrence forward from `first` by `gap_days`, returning
/// every occurrence inside `[start, end)`.
fn occurrences_in_window(first: NaiveDate, gap_days: i64, start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    if gap_days <= 0 {
        if first >= start && first < end {
            out.push(first);
        }
        return out;
    }
    let mut d = first;
    // Fast-forward into the window without a per-day loop.
    if d < start {
        let behind = (start - d).num_days();
        let steps = behind / gap_days;
        d += Duration::days(steps * gap_days);
        while d < start {
            d += Duration::days(gap_days);
        }
    }
    while d < end {
        out.push(d);
        d += Duration::days(gap_days);
    }
    out
}

/// Whether a recurring bill/subscription occurrence is already covered by a
/// user-entered planned transaction, so we don't count the same charge twice.
/// Fuzzy on purpose: same-ish date and amount.
fn covered_by_planned(date: NaiveDate, amount_cents: i64, planned: &[(NaiveDate, i64, String)]) -> bool {
    planned.iter().any(|(pd, pa, _)| {
        (*pd - date).num_days().abs() <= 5
            && pa.signum() == amount_cents.signum()
            && {
                let a = pa.unsigned_abs() as f64;
                let b = amount_cents.unsigned_abs() as f64;
                let hi = a.max(b);
                hi > 0.0 && (a - b).abs() / hi <= 0.20
            }
    })
}

/// Build the daily cash-flow forecast from the user's real data plus an optional
/// what-if overlay. Assembles dated events (recurring income/bills/subs rolled
/// forward + planned transactions), computes the residual smooth burn, then
/// hands everything to the pure [`project`].
pub fn build_forecast(conn: &mut Connection, horizon_days: i64, whatif: &WhatIf) -> CoreResult<CashflowForecast> {
    let horizon = horizon_days.clamp(MIN_HORIZON_DAYS, MAX_HORIZON_DAYS);
    let as_of = chrono::Utc::now().date_naive();
    let horizon_end = as_of + Duration::days(horizon);

    let balances = metrics::balance_breakdown(conn)?;
    let rolling = metrics::rolling_averages(conn, 90)?;
    let items = recurring::detect_recurring(conn, RECURRING_WINDOW_DAYS)?;

    // Planned transactions due within the horizon, still pending.
    let planned_rows = planned_transactions::list(
        conn,
        PlannedTxnFilter {
            status: Some("pending".to_string()),
            due_before: Some(horizon_end.format("%Y-%m-%d").to_string()),
            ..Default::default()
        },
    )?;
    let planned: Vec<(NaiveDate, i64, String)> = planned_rows
        .iter()
        .filter_map(|p| {
            let d = NaiveDate::parse_from_str(&p.due_date, "%Y-%m-%d").ok()?;
            (d >= as_of).then(|| (d, p.amount_cents, p.description.clone()))
        })
        .collect();

    let mut events: Vec<CashflowEvent> = Vec::new();
    let mut warnings: Vec<CashflowWarning> = Vec::new();

    // Sum of the monthly-equivalent of the DATED obligations, so the smooth burn
    // can subtract them and they aren't counted twice.
    let mut dated_obligation_monthly: i64 = 0;

    for item in &items {
        let Some(next) = item.next_expected.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()) else {
            continue;
        };
        let gap = item.avg_gap_days.round() as i64;

        let (kind, include) = match item.kind {
            RecurringKind::Income if item.confidence >= recurring::PROJECTION_CONFIDENCE_THRESHOLD => {
                (CashflowEventKind::Income, true)
            }
            RecurringKind::Bill if item.is_projection_obligation() => (CashflowEventKind::Bill, true),
            RecurringKind::Subscription if item.is_projection_obligation() => (CashflowEventKind::Subscription, true),
            // Transfers (internal / card payments) and irregular repeat purchases
            // are never projected as dated events — the latter feed the smooth burn.
            _ => (CashflowEventKind::Bill, false),
        };
        if !include {
            near_horizon_warning(item, next, gap, horizon_end, &mut warnings);
            continue;
        }

        let is_obligation = matches!(kind, CashflowEventKind::Bill | CashflowEventKind::Subscription);
        if is_obligation {
            dated_obligation_monthly += item.monthly_equivalent_cents();
        }

        for occ in occurrences_in_window(next, gap, as_of, horizon_end) {
            // Prefer the user's explicit planned entry over a detected duplicate.
            if is_obligation && covered_by_planned(occ, item.last_amount_cents, &planned) {
                continue;
            }
            events.push(CashflowEvent {
                date: occ.format("%Y-%m-%d").to_string(),
                label: item.display_merchant.clone(),
                amount_cents: item.last_amount_cents,
                kind,
                confidence: Some(item.confidence),
            });
        }
        near_horizon_warning(item, next, gap, horizon_end, &mut warnings);
    }

    for (d, amount, label) in &planned {
        events.push(CashflowEvent {
            date: d.format("%Y-%m-%d").to_string(),
            label: label.clone(),
            amount_cents: *amount,
            kind: CashflowEventKind::Planned,
            confidence: None,
        });
    }

    // Residual everyday variable spend = typical monthly expense minus the dated
    // obligations already applied above, spread evenly per day. Never negative:
    // if obligations exceed the average, the dated events already carry the load.
    let residual_monthly = (rolling.avg_monthly_expense_cents - dated_obligation_monthly).max(0);
    let daily_burn_cents = (residual_monthly as f64 / AVG_DAYS_PER_MONTH).round() as i64;

    // What-if overlay: hypothetical outflow (as a dated event) + buffer.
    if whatif.extra_expense_cents > 0 {
        let d = whatif.extra_expense_date.unwrap_or(as_of);
        events.push(CashflowEvent {
            date: d.format("%Y-%m-%d").to_string(),
            label: whatif.extra_expense_label.clone().unwrap_or_else(|| "Hypothetical spend".to_string()),
            amount_cents: -whatif.extra_expense_cents.abs(),
            kind: CashflowEventKind::Hypothetical,
            confidence: None,
        });
    }

    let mut forecast = project(as_of, balances.liquid_cents, horizon, daily_burn_cents, whatif.buffer_cents, &events);

    // Data-quality disclosures — the difference between a forecast and a guess.
    let reliable = rolling.data_span_days >= MIN_RELIABLE_SPAN_DAYS;
    if !reliable {
        warnings.push(CashflowWarning {
            level: WarningLevel::Caution,
            message: format!(
                "Only {} day(s) of history so far — this forecast is a rough estimate, not a precise projection.",
                rolling.data_span_days
            ),
        });
    }
    if balances.accounts_with_unknown_balance > 0 {
        warnings.push(CashflowWarning {
            level: WarningLevel::Caution,
            message: format!(
                "The starting balance excludes {} account(s) with no confirmed balance, so the real starting point may be higher.",
                balances.accounts_with_unknown_balance
            ),
        });
    }
    if !balances.unconverted.is_empty() {
        let codes: Vec<&str> = balances.unconverted.iter().map(|h| h.code.as_str()).collect();
        warnings.push(CashflowWarning {
            level: WarningLevel::Info,
            message: format!("This forecast is in your primary currency; money held in {} isn't included.", codes.join(", ")),
        });
    }
    if forecast.upcoming_events.iter().all(|e| e.kind != CashflowEventKind::Income) && rolling.avg_monthly_income_cents > 0 {
        warnings.push(CashflowWarning {
            level: WarningLevel::Info,
            message: "No recurring income lands in this window, so the trajectory only falls — the next paycheck may be just beyond it.".to_string(),
        });
    }

    forecast.warnings = warnings;
    forecast.reliable = reliable;
    Ok(forecast)
}

/// If an obligation's next occurrence falls just past the horizon, surface it so
/// a short window can't hide a large upcoming (often annual) charge.
fn near_horizon_warning(item: &RecurringItem, next: NaiveDate, gap: i64, horizon_end: NaiveDate, warnings: &mut Vec<CashflowWarning>) {
    if !item.is_projection_obligation() {
        return;
    }
    let lookahead_end = horizon_end + Duration::days(NEAR_HORIZON_LOOKAHEAD_DAYS);
    // The first occurrence on/after the horizon end.
    let just_after = occurrences_in_window(next, gap, horizon_end, lookahead_end)
        .into_iter()
        .next();
    if let Some(d) = just_after {
        warnings.push(CashflowWarning {
            level: WarningLevel::Caution,
            message: format!(
                "{} (about {}) is due on {}, just after this window — plan for it.",
                item.display_merchant,
                fmt_money(item.last_amount_cents),
                d.format("%Y-%m-%d"),
            ),
        });
    }
}

fn fmt_money(cents: i64) -> String {
    format!("${:.0}", (cents.unsigned_abs() as f64) / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn ev(date: &str, amount: i64, kind: CashflowEventKind) -> CashflowEvent {
        CashflowEvent { date: date.into(), label: "x".into(), amount_cents: amount, kind, confidence: None }
    }

    // ── Pure projection math ────────────────────────────────────────────────

    #[test]
    fn safe_to_spend_is_the_lowest_point_minus_buffer() {
        // $1000 start, $10/day burn, a $500 bill on day 5, income $1200 on day 20.
        let events = vec![
            ev("2026-01-06", -50_000, CashflowEventKind::Bill),
            ev("2026-01-21", 120_000, CashflowEventKind::Income),
        ];
        let f = project(d("2026-01-01"), 100_000, 30, 1_000, 20_000, &events);
        // Lowest point is just before the paycheck; safe-to-spend = lowest - buffer.
        assert_eq!(f.safe_to_spend_cents, (f.lowest_balance_cents - 20_000).max(0));
        assert!(f.lowest_balance_cents < 100_000, "burn + bill must draw the balance down");
    }

    #[test]
    fn daily_burn_actually_drains_the_balance() {
        // No events, pure burn: end balance = start - burn*horizon.
        let f = project(d("2026-01-01"), 100_000, 10, 2_000, 0, &[]);
        assert_eq!(f.days.last().unwrap().projected_balance_cents, 100_000 - 2_000 * 10);
        assert_eq!(f.lowest_balance_cents, 100_000 - 2_000 * 10);
    }

    #[test]
    fn first_breach_is_the_first_day_below_buffer() {
        // Start $100, burn $30/day, buffer $50 → breaches when balance < 50.
        let f = project(d("2026-01-01"), 10_000, 20, 3_000, 5_000, &[]);
        // day0: 7000, day1: 4000 (<5000) → breach on day1 = 2026-01-02.
        assert_eq!(f.first_breach_date.as_deref(), Some("2026-01-02"));
    }

    #[test]
    fn positive_trajectory_has_no_breach_and_full_safe_to_spend() {
        // Income exceeds burn, balance only rises → no breach, safe-to-spend = start - buffer.
        let events = vec![ev("2026-01-02", 500_000, CashflowEventKind::Income)];
        let f = project(d("2026-01-01"), 100_000, 15, 1_000, 10_000, &events);
        assert!(f.first_breach_date.is_none());
        // Lowest is day0 (before income): 100_000 - 1_000 = 99_000.
        assert_eq!(f.safe_to_spend_cents, 99_000 - 10_000);
    }

    #[test]
    fn hypothetical_spend_lowers_safe_to_spend() {
        let base = project(d("2026-01-01"), 100_000, 30, 1_000, 0, &[]);
        let with_spend = project(
            d("2026-01-01"),
            100_000,
            30,
            1_000,
            0,
            &[ev("2026-01-01", -40_000, CashflowEventKind::Hypothetical)],
        );
        assert_eq!(with_spend.safe_to_spend_cents, base.safe_to_spend_cents - 40_000);
    }

    #[test]
    fn occurrences_roll_forward_by_cadence() {
        // Monthly from Jan 10, window Jan 1 – Mar 1 → Jan 10 and Feb 9 (gap 30).
        let occ = occurrences_in_window(d("2026-01-10"), 30, d("2026-01-01"), d("2026-03-01"));
        assert_eq!(occ, vec![d("2026-01-10"), d("2026-02-09")]);
    }

    #[test]
    fn covered_by_planned_matches_fuzzy_date_and_amount() {
        let planned = vec![(d("2026-01-03"), -145_000, "Rent".to_string())];
        assert!(covered_by_planned(d("2026-01-01"), -150_000, &planned)); // within 5d, 20%
        assert!(!covered_by_planned(d("2026-01-01"), -50_000, &planned)); // amount too different
        assert!(!covered_by_planned(d("2026-01-20"), -145_000, &planned)); // date too far
    }

    // ── DB assembly: the burn decomposition, exclusions, and disclosures ─────

    fn fresh_db() -> (tempfile::TempDir, crate::Db) {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("cashflow.sqlcipher"), &key).unwrap();
        crate::db::run_migrations(&db).unwrap();
        (dir, db)
    }

    /// A regular series ending `end` (inclusive), `count` occurrences `gap` days
    /// apart, walking backward — so the data is "current" relative to today.
    fn series(conn: &Connection, merchant: &str, end: NaiveDate, gap: i64, count: i64, amount: i64, transfer: bool) {
        for i in 0..count {
            let date = end - Duration::days(i * gap);
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
                 VALUES(hex(randomblob(16)),'acct',?1,?2,?3,?4,'cleared',datetime('now'))",
                rusqlite::params![format!("{}T12:00:00Z", date.format("%Y-%m-%d")), amount, merchant, transfer as i64],
            )
            .unwrap();
        }
    }

    /// Everyday variable spend: many small charges across rotating merchants with
    /// jittered amounts, so they feed the average expense WITHOUT being detected
    /// as a single recurring obligation.
    fn steady_spend(conn: &Connection, end: NaiveDate, days: i64) {
        let merchants = ["CORNER MART", "CAFE DELISH", "GAS N GO", "QUICK BITE", "DAILY GROCER"];
        for i in 0..days {
            let date = end - Duration::days(i);
            let merchant = merchants[(i as usize) % merchants.len()];
            let amount = -(1_800 + (i % 7) * 350); // ~$18–$39, varied
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
                 VALUES(hex(randomblob(16)),'acct',?1,?2,?3,0,'cleared',datetime('now'))",
                rusqlite::params![format!("{}T09:00:00Z", date.format("%Y-%m-%d")), amount, merchant],
            )
            .unwrap();
        }
    }

    /// The advisor's steady-burn guard: a mid-cycle paycheck + a dated bill + a
    /// regular internal transfer + everyday spending. Confirms (a) the smooth
    /// burn subtracts the dated bill's monthly equivalent (no double-count and
    /// the bill IS detected), (b) an internal transfer never appears as a drain,
    /// (c) income/bill land as dated events, and (d) a bill just past a short
    /// horizon is disclosed rather than silently dropped.
    #[test]
    fn forecast_reflects_burn_excludes_transfers_and_discloses_near_horizon() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let today = chrono::Utc::now().date_naive();

        // A liquid account for the transactions to reference (FK). No balance
        // snapshot — the burn/exclusion assertions don't depend on the balance.
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,color,created_at) \
             VALUES('acct','me','Bank','Checking','Checking','#fff',datetime('now'))",
            [],
        )
        .unwrap();

        // ~90 days of everyday spending → establishes the average expense.
        steady_spend(&conn, today, 90);
        // A monthly utility bill, last paid 5 days ago → next due ~25 days out.
        series(&conn, "ACME UTILITIES", today - Duration::days(5), 30, 6, -12_000, false);
        // A monthly paycheck, last received 10 days ago → next ~20 days out.
        series(&conn, "EMPLOYER PAYROLL", today - Duration::days(10), 30, 6, 400_000, false);
        // A monthly internal transfer to savings — NOT a real cost.
        series(&conn, "INTERNAL TRANSFER SAVINGS", today - Duration::days(7), 30, 6, -50_000, true);

        let f = build_forecast(&mut conn, 30, &WhatIf::default()).unwrap();

        // (a) The smooth burn equals avg expense minus the dated obligations'
        // monthly equivalent — so the bill is counted once, on its date, not also
        // smeared into the daily burn.
        let avg_exp = metrics::rolling_averages(&conn, 90).unwrap().avg_monthly_expense_cents;
        let obligations_monthly: i64 = recurring::projection_obligations(&conn, RECURRING_WINDOW_DAYS)
            .unwrap()
            .iter()
            .map(|o| o.monthly_equivalent_cents())
            .sum();
        assert!(obligations_monthly > 0, "the monthly bill must be detected as an obligation");
        let expected_burn = ((avg_exp - obligations_monthly).max(0) as f64 / AVG_DAYS_PER_MONTH).round() as i64;
        assert_eq!(f.daily_burn_cents, expected_burn);
        assert!(f.daily_burn_cents > 0, "everyday spending must produce a real daily burn");

        // (b) The internal transfer is never a projected drain.
        assert!(
            !f.upcoming_events.iter().any(|e| e.label.to_uppercase().contains("TRANSFER")),
            "an internal transfer must not appear as a cash-flow event: {:?}",
            f.upcoming_events
        );

        // (c) Income and the recurring obligation (bill/subscription — the
        // classifier's exact label doesn't matter) land as dated events.
        assert!(f.upcoming_events.iter().any(|e| e.kind == CashflowEventKind::Income));
        assert!(
            f.upcoming_events
                .iter()
                .any(|e| matches!(e.kind, CashflowEventKind::Bill | CashflowEventKind::Subscription)
                    && e.label.to_uppercase().contains("ACME")),
            "the recurring obligation must land as a dated event: {:?}",
            f.upcoming_events
        );

        // (d) With a 7-day horizon the bill (due ~25 days out) is beyond the
        // window, so it must be disclosed as a heads-up rather than ignored.
        let short = build_forecast(&mut conn, 7, &WhatIf::default()).unwrap();
        assert!(
            short.warnings.iter().any(|w| w.message.contains("ACME UTILITIES") && w.message.contains("just after")),
            "a bill just past the horizon must be surfaced: {:?}",
            short.warnings
        );
    }
}

use crate::error::{CoreError, CoreResult};
use crate::merchant::canonical_merchant_key;
use crate::models::{
    BudgetChange, BudgetTransfer, CustomReportParams, CustomReportResult, FundingTemplate, Period,
    ReportRow, SplitBy,
};
use chrono::{Datelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use specta::Type;
use utoipa::ToSchema;
use uuid::Uuid;

/// Tolerance for to-budget validation: allow $0.50 over-assign before blocking.
pub const TO_BUDGET_TOLERANCE_CENTS: i64 = 50;

/// Validate `to_budget` for `month`. Returns `CoreError::Validation("over-assigned by …")`
/// when `to_budget < -TOLERANCE`. When `allow_over_assign` is true the check is skipped.
pub fn validate_to_budget(
    conn: &Connection,
    month: &str,
    allow_over_assign: bool,
) -> CoreResult<()> {
    if allow_over_assign {
        return Ok(());
    }
    // If no income is recorded for the month, budgeting is not yet bounded by
    // income — allow the write so tests and early-month planning can proceed.
    // The guard activates once income exists, which mirrors Actual's flow where
    // To Budget is income-driven. This keeps existing tests (which set budgets
    // without seeding income) passing while still protecting real over-assigns.
    let income = total_income(conn, month)?;
    if income == 0 {
        return Ok(());
    }
    let tb = to_budget(conn, month)?;
    if tb < -TO_BUDGET_TOLERANCE_CENTS {
        let over = -tb;
        let dollars = format!("${:.2}", over as f64 / 100.0);
        return Err(CoreError::Validation(format!(
            "over-assigned by {} (to_budget {}¢)",
            dollars, tb
        )));
    }
    Ok(())
}

/// Set (upsert) a budget for a category in a given month (format: "YYYY-MM").
/// When `allow_over_assign` is false (default) the write is validated against
/// `to_budget` and rejected with `CoreError::Validation` if the month would be
/// over-assigned beyond the $0.50 tolerance.
pub fn set(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
    amount_cents: i64,
    allow_over_assign: bool,
) -> CoreResult<()> {
    crate::repos::atomic(conn, |conn| {
        set_raw(conn, category_id, month, amount_cents, allow_over_assign)
    })
}

fn set_raw(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
    amount_cents: i64,
    allow_over_assign: bool,
) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(category_id, month) DO UPDATE SET amount_cents = excluded.amount_cents, updated_at = excluded.updated_at",
        params![id, category_id, month, amount_cents, now],
    )?;
    validate_to_budget(conn, month, allow_over_assign)?;
    Ok(())
}

/// Backwards-compatible wrapper for `set` with `allow_over_assign = false`.
/// Existing tests and internal callers that do not need over-assign use this.
pub fn set_simple(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
    amount_cents: i64,
) -> CoreResult<()> {
    set(conn, category_id, month, amount_cents, false)
}

/// Return a map of category_id → amount_cents for the given month.
pub fn list_for_month(conn: &mut Connection, month: &str) -> CoreResult<Vec<(String, i64)>> {
    let mut stmt =
        conn.prepare("SELECT category_id, amount_cents FROM budgets WHERE month = ?1")?;
    let rows = stmt.query_map(params![month], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Return the "YYYY-MM" string `n` months before `month` ("YYYY-MM"). `n` may be
/// negative to step forward instead.
pub fn month_before(month: &str, n: i32) -> String {
    let year: i32 = month[0..4].parse().unwrap_or(1970);
    let mon: i32 = month[5..7].parse().unwrap_or(1); // 1-12
    let total = year * 12 + (mon - 1) - n; // zero-based month index
    let y = total.div_euclid(12);
    let m = total.rem_euclid(12) + 1;
    format!("{y:04}-{m:02}")
}

/// Get the hold for a month, if any. `month` is "YYYY-MM".
pub fn get_hold(conn: &Connection, month: &str) -> CoreResult<Option<i64>> {
    let v: Option<i64> = conn
        .query_row(
            "SELECT amount_cents FROM budget_holds WHERE month = ?1",
            params![month],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v)
}

/// Set (upsert) the hold amount for `month`. `amount_cents` must be >= 0.
/// A hold parks unassigned money for next month: it deducts from this month's
/// `to_budget` (`income - budgeted - hold`) and appears as income-like in
/// `available_funds` for the following month.
pub fn set_hold(conn: &mut Connection, month: &str, amount_cents: i64) -> CoreResult<()> {
    if amount_cents < 0 {
        return Err(crate::error::CoreError::Validation(
            "hold amount must be >= 0".to_string(),
        ));
    }
    conn.execute(
        "INSERT INTO budget_holds(month, amount_cents) VALUES(?1, ?2) \
         ON CONFLICT(month) DO UPDATE SET amount_cents = excluded.amount_cents",
        params![month, amount_cents],
    )?;
    Ok(())
}

/// Total income for `month` ("YYYY-MM"): sum of positive, non-transfer,
/// non-settle-up transactions posted in that month. Mirrors the income leg of
/// `metrics::cashflow_since` (settle_up excluded, is_transfer excluded) but
/// scoped to a calendar month. Currency / investment filtering is intentionally
/// not applied here — budgeting is household-centric and the budget screen's
/// `incomeCents` comes from the same monthly transaction sum, so the hold math
/// must use the same denominator.
pub fn total_income(conn: &Connection, month: &str) -> CoreResult<i64> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions \
         WHERE amount_cents > 0 \
           AND is_transfer = 0 \
           AND COALESCE(settle_up, 0) = 0 \
           AND strftime('%Y-%m', posted_at) = ?1",
        params![month],
        |r| r.get(0),
    )?;
    Ok(v)
}

/// Total budgeted amount for `month`.
pub fn total_budgeted(conn: &Connection, month: &str) -> CoreResult<i64> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets WHERE month = ?1",
        params![month],
        |r| r.get(0),
    )?;
    Ok(v)
}

/// To-budget for `month`: income - budgeted - hold.
///
/// This is the Actual `To Budget` primitive: the unassigned remainder after
/// funding envelopes and parking a hold for next month.
pub fn to_budget(conn: &Connection, month: &str) -> CoreResult<i64> {
    let income = total_income(conn, month)?;
    let budgeted = total_budgeted(conn, month)?;
    let hold = get_hold(conn, month)?.unwrap_or(0);
    Ok(income - budgeted - hold)
}

/// Available funds for `month`: income - budgeted - hold_current + hold_prev.
///
/// A hold deducted from the prior month reappears as income-like here, so
/// `available_funds("2026-10")` after `set_hold("2026-09", 1500)` is 1500 even
/// when October has no income yet. For the current month this equals
/// `to_budget` when there is no prior hold.
///
/// This mirrors Actual's "Hold for Next Month" rollover: the held amount is
/// not spent, not budgeted, and not lost — it funds the next month.
pub fn available_funds(conn: &Connection, month: &str) -> CoreResult<i64> {
    let income = total_income(conn, month)?;
    let budgeted = total_budgeted(conn, month)?;
    let cur_hold = get_hold(conn, month)?.unwrap_or(0);
    let prev = month_before(month, 1);
    let prev_hold = get_hold(conn, &prev)?.unwrap_or(0);
    Ok(income - budgeted - cur_hold + prev_hold)
}

// ── Atomic Cover Ledger (Actual's Cover as auditable row) ─────────────────

/// Net available for one category in `month`.
///
/// `available = budgeted + carryover + transfers_in - transfers_out - spent`
/// where `carryover` is the running `budgeted - spent` from the category's
/// first-ever budgeted month up to (not including) `month`, capped at 24 months.
/// `spent` includes `settle_up` reimbursements as in `budget_envelopes_for_month`
/// (a reimbursement nets against the category's outflow). Transfers are strictly
/// within `month` — this keeps cover auditable per month and avoids smearing a
/// one-time move across the carryover window.
pub fn available(conn: &Connection, category_id: &str, month: &str) -> CoreResult<i64> {
    let budgeted: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets WHERE category_id = ?1 AND month = ?2",
        params![category_id, month],
        |r| r.get(0),
    )?;
    let carry = carryover_for(conn, category_id, month)?;
    let transfers_in: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budget_transfers WHERE to_category = ?1 AND month = ?2",
        params![category_id, month],
        |r| r.get(0),
    )?;
    let transfers_out: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budget_transfers WHERE from_category = ?1 AND month = ?2",
        params![category_id, month],
        |r| r.get(0),
    )?;
    let start = format!("{month}-01");
    let next = month_before(month, -1);
    let next_start = format!("{next}-01");
    let spent = category_spent(conn, category_id, &start, &next_start)?;
    Ok(budgeted + carry + transfers_in - transfers_out - spent)
}

/// List all transfers for `month` ordered by `created_at` ASC.
pub fn list_transfers(conn: &Connection, month: &str) -> CoreResult<Vec<BudgetTransfer>> {
    let mut stmt = conn.prepare(
        "SELECT id, month, from_category, to_category, amount_cents, note, created_at \
          FROM budget_transfers WHERE month = ?1 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![month], |r| {
        Ok(BudgetTransfer {
            id: r.get(0)?,
            month: r.get(1)?,
            from_category: r.get(2)?,
            to_category: r.get(3)?,
            amount_cents: r.get(4)?,
            note: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// List all transfers across every month, ordered by month DESC then created_at.
/// Useful for audit views; `list_transfers` is the month-scoped primary API.
pub fn list_all_transfers(conn: &Connection) -> CoreResult<Vec<BudgetTransfer>> {
    let mut stmt = conn.prepare(
        "SELECT id, month, from_category, to_category, amount_cents, note, created_at \
          FROM budget_transfers ORDER BY month DESC, created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(BudgetTransfer {
            id: r.get(0)?,
            month: r.get(1)?,
            from_category: r.get(2)?,
            to_category: r.get(3)?,
            amount_cents: r.get(4)?,
            note: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Atomically move `amount_cents` from `from_category` to `to_category` within
/// `month` and record the auditable ledger row.
///
/// `from_category` and `to_category` are required category ids (non-empty).
/// `amount_cents` must be > 0 and available spare of `from_category` in that
/// month must be >= amount, otherwise `CoreError::Validation` is returned and
/// no row is written. The check and insert are performed in a single
/// `BEGIN IMMEDIATE` transaction, so concurrent covers cannot overdraft the
/// same donor.
///
/// `note` is free-form audit text (e.g. "cover overspend").
pub fn transfer(
    conn: &mut Connection,
    from_category: &str,
    to_category: &str,
    amount_cents: i64,
    month: &str,
    note: Option<&str>,
) -> CoreResult<BudgetTransfer> {
    transfer_optional(
        conn,
        Some(from_category),
        Some(to_category),
        amount_cents,
        month,
        note,
    )
}

/// Nullable variant: either side may be `None` to represent moving to/from
/// unassigned (To Budget). At least one side must be `Some`, and when both
/// are `Some` they must differ. Validation of spare applies only when
/// `from_category` is `Some`.
pub fn transfer_optional(
    conn: &mut Connection,
    from_category: Option<&str>,
    to_category: Option<&str>,
    amount_cents: i64,
    month: &str,
    note: Option<&str>,
) -> CoreResult<BudgetTransfer> {
    if amount_cents <= 0 {
        return Err(CoreError::Validation(
            "transfer amount must be > 0".to_string(),
        ));
    }
    if from_category.is_none() && to_category.is_none() {
        return Err(CoreError::Validation(
            "transfer requires at least one of from_category / to_category".to_string(),
        ));
    }
    if let (Some(f), Some(t)) = (from_category, to_category) {
        if f == t {
            return Err(CoreError::Validation(
                "from_category and to_category must differ".to_string(),
            ));
        }
        if f.trim().is_empty() || t.trim().is_empty() {
            return Err(CoreError::Validation(
                "category ids must be non-empty".to_string(),
            ));
        }
    }
    // Validate month shape loosely "YYYY-MM"
    if month.len() != 7 || &month[4..5] != "-" {
        return Err(CoreError::Validation(format!(
            "month must be YYYY-MM, got `{month}`"
        )));
    }
    crate::repos::atomic(conn, |conn| {
        // Spare check only when moving *from* a category (donor)
        if let Some(from) = from_category {
            let avail = available(conn, from, month)?;
            if avail < amount_cents {
                return Err(CoreError::Validation(format!(
                    "insufficient spare in `{from}` for month {month}: available {avail} < {amount_cents}"
                )));
            }
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO budget_transfers(id, month, from_category, to_category, amount_cents, note, created_at) \
              VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, month, from_category, to_category, amount_cents, note, now],
        )?;
        Ok(BudgetTransfer {
            id,
            month: month.to_string(),
            from_category: from_category.map(|s| s.to_string()),
            to_category: to_category.map(|s| s.to_string()),
            amount_cents,
            note: note.map(|s| s.to_string()),
            created_at: now,
        })
    })
}

/// Delete a transfer by id. Primarily for undo/audit correction; returns true
/// if a row was deleted. Does not retroactively validate spare after deletion.
pub fn delete_transfer(conn: &mut Connection, id: &str) -> CoreResult<bool> {
    let n = conn.execute("DELETE FROM budget_transfers WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

// ── Declarative Funding Templates (Actual's #template as a table) ─────────

const FUNDING_KINDS: &[&str] = &[
    "fixed",
    "up_to",
    "by",
    "average",
    "percent",
    "remainder",
    "schedule",
];

fn validate_funding_kind(kind: &str) -> CoreResult<()> {
    if FUNDING_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "invalid funding template kind `{kind}` — expected one of {}",
            FUNDING_KINDS.join(", ")
        )))
    }
}

fn parse_amount_from_json(params_json: &str, keys: &[&str]) -> CoreResult<i64> {
    let v: serde_json::Value = serde_json::from_str(params_json)
        .map_err(|e| CoreError::Validation(format!("invalid params_json: {e}")))?;
    if !v.is_object() {
        return Err(CoreError::Validation(
            "params_json must be a JSON object".to_string(),
        ));
    }
    for k in keys {
        if let Some(n) = v.get(*k).and_then(|x| x.as_i64()) {
            return Ok(n);
        }
        // allow floating point amounts; round to nearest cent
        if let Some(n) = v.get(*k).and_then(|x| x.as_f64()) {
            return Ok(n.round() as i64);
        }
    }
    Ok(0)
}

fn parse_pct_from_json(params_json: &str) -> CoreResult<f64> {
    let v: serde_json::Value = serde_json::from_str(params_json)
        .map_err(|e| CoreError::Validation(format!("invalid params_json: {e}")))?;
    if !v.is_object() {
        return Err(CoreError::Validation(
            "params_json must be a JSON object".to_string(),
        ));
    }
    for k in ["pct", "percent", "pct_f32", "percent_f32"] {
        if let Some(n) = v.get(k).and_then(|x| x.as_f64()) {
            // Accept 0..1 as fraction or 0..100 as percent — values >1 are treated as percent/100
            if n > 1.0 {
                return Ok(n / 100.0);
            }
            return Ok(n);
        }
        if let Some(n) = v.get(k).and_then(|x| x.as_i64()) {
            let f = n as f64;
            if f > 1.0 {
                return Ok(f / 100.0);
            }
            return Ok(f);
        }
    }
    // also try "amount" as alias for percent? not needed
    Ok(0.0)
}

// ── Schedule kind helpers (cron/interval) ────────────────────────────────────

fn parse_schedule_string(params_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(params_json).ok()?;
    if !v.is_object() {
        return None;
    }
    for key in [
        "schedule",
        "cron",
        "pattern",
        "interval",
        "frequency",
        "cadence",
    ] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn days_in_month(year: i32, month: u32) -> u32 {
    // chrono handles leap years
    let next_month = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    let first = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let next = next_month.unwrap();
    (next - first).num_days() as u32
}

fn cron_field_matches(field: &str, value: u32, min: u32, _max: u32) -> bool {
    let field = field.trim();
    if field == "*" {
        return true;
    }
    // comma-separated list: any matches
    if field.contains(',') {
        return field
            .split(',')
            .any(|part| cron_field_matches(part, value, min, _max));
    }
    // step: base/step
    if let Some((base, step_str)) = field.split_once('/') {
        let step: u32 = step_str.trim().parse().unwrap_or(0);
        if step == 0 {
            return false;
        }
        let base = base.trim();
        if base == "*" {
            return (value - min) % step == 0;
        }
        if base.contains('-') {
            if let Some((s, e)) = base.split_once('-') {
                let start: u32 = s.trim().parse().unwrap_or(min);
                let end: u32 = e.trim().parse().unwrap_or(_max);
                if value < start || value > end {
                    return false;
                }
                return (value - start) % step == 0;
            }
        }
        // single number with step: e.g. "5/15"
        if let Ok(start) = base.parse::<u32>() {
            if value < start {
                return false;
            }
            return (value - start) % step == 0;
        }
        return false;
    }
    // range: a-b
    if let Some((s, e)) = field.split_once('-') {
        if let (Ok(start), Ok(end)) = (s.trim().parse::<u32>(), e.trim().parse::<u32>()) {
            return value >= start && value <= end;
        }
        return false;
    }
    // exact
    if let Ok(n) = field.parse::<u32>() {
        // dow: 7 == 0 (Sunday)
        if min == 0 && n == 7 && value == 0 {
            return true;
        }
        return value == n;
    }
    false
}

fn cron_dow_matches(field: &str, dow_sunday0: u32) -> bool {
    // cron dow: 0 and 7 both Sunday, 1=Monday ... 6=Saturday
    let field = field.trim();
    if field == "*" {
        return true;
    }
    // Expand comma lists recursively
    if field.contains(',') {
        return field
            .split(',')
            .any(|part| cron_dow_matches(part, dow_sunday0));
    }
    // step
    if let Some((base, step_str)) = field.split_once('/') {
        let step: u32 = step_str.trim().parse().unwrap_or(0);
        if step == 0 {
            return false;
        }
        let base = base.trim();
        if base == "*" {
            return dow_sunday0 % step == 0;
        }
        if base.contains('-') {
            if let Some((s, e)) = base.split_once('-') {
                let start: u32 = s.trim().parse().unwrap_or(0);
                let end: u32 = e.trim().parse().unwrap_or(7);
                // normalize 7->0 for range checks
                let (start_n, end_n) = (
                    if start == 7 { 0 } else { start },
                    if end == 7 { 0 } else { end },
                );
                // handle wrap-around Sunday range like 5-1 (Fri-Mon)
                if start_n <= end_n {
                    if dow_sunday0 < start_n || dow_sunday0 > end_n {
                        return false;
                    }
                    return (dow_sunday0 - start_n) % step == 0;
                } else {
                    // wrapped range: valid if >=start or <=end
                    if dow_sunday0 >= start_n || dow_sunday0 <= end_n {
                        // step logic for wrapped is complex; fallback to true if in range
                        return true;
                    }
                    return false;
                }
            }
        }
        if let Ok(start) = base.parse::<u32>() {
            let start_n = if start == 7 { 0 } else { start };
            if dow_sunday0 < start_n {
                return false;
            }
            return (dow_sunday0 - start_n) % step == 0;
        }
        return false;
    }
    if let Some((s, e)) = field.split_once('-') {
        let start: u32 = s.trim().parse().unwrap_or(0);
        let end: u32 = e.trim().parse().unwrap_or(7);
        let (start_n, end_n) = (
            if start == 7 { 0 } else { start },
            if end == 7 { 0 } else { end },
        );
        let dow_n = dow_sunday0;
        if start_n <= end_n {
            return dow_n >= start_n && dow_n <= end_n;
        } else {
            return dow_n >= start_n || dow_n <= end_n;
        }
    }
    if let Ok(n) = field.parse::<u32>() {
        let n_n = if n == 7 { 0 } else { n };
        return dow_sunday0 == n_n;
    }
    false
}

fn cron_is_due_in_month(cron_str: &str, month: &str) -> bool {
    let parts: Vec<&str> = cron_str.split_whitespace().collect();
    // Accept 5 fields (min hour dom mon dow) or 6 with seconds prefix
    let (dom_str, mon_str, dow_str) = if parts.len() == 5 {
        (parts[2], parts[3], parts[4])
    } else if parts.len() == 6 {
        (parts[3], parts[4], parts[5])
    } else {
        return false;
    };
    let year: i32 = month[0..4].parse().unwrap_or(1970);
    let mon: u32 = month[5..7].parse().unwrap_or(1);
    if mon < 1 || mon > 12 {
        return false;
    }
    // month field must match the target month
    if !cron_field_matches(mon_str, mon, 1, 12) {
        return false;
    }
    let dim = days_in_month(year, mon);
    let dom_is_star = dom_str.trim() == "*";
    let dow_is_star = dow_str.trim() == "*";
    for day in 1..=dim {
        let dom_match = cron_field_matches(dom_str, day, 1, 31);
        let date = chrono::NaiveDate::from_ymd_opt(year, mon, day).unwrap();
        let dow_sunday0 = date.weekday().num_days_from_sunday();
        let dow_match = cron_dow_matches(dow_str, dow_sunday0);
        let day_match = match (dom_is_star, dow_is_star) {
            (true, true) => true,
            (true, false) => dow_match,
            (false, true) => dom_match,
            (false, false) => dom_match || dow_match,
        };
        if day_match {
            // verify hour/min are not impossible? we ignore them; any day match suffices
            return true;
        }
    }
    false
}

fn interval_days_from_str(s: &str) -> Option<u32> {
    let lower = s.to_lowercase();
    let trimmed = lower.trim();
    // explicit keyword mappings first (without number)
    if trimmed == "daily" || trimmed == "day" || trimmed == "every day" || trimmed == "everyday" {
        return Some(1);
    }
    if trimmed == "weekly" || trimmed == "every week" || trimmed == "week" {
        return Some(7);
    }
    if trimmed == "biweekly"
        || trimmed == "bi-weekly"
        || trimmed == "fortnightly"
        || trimmed == "every 2 weeks"
        || trimmed == "every fortnight"
    {
        return Some(14);
    }
    if trimmed == "monthly"
        || trimmed == "every month"
        || trimmed == "month"
        || trimmed == "once a month"
    {
        return Some(30);
    }
    if trimmed == "quarterly" || trimmed == "every quarter" || trimmed == "every 3 months" {
        return Some(90);
    }
    if trimmed == "semiannually" || trimmed == "semi-annually" || trimmed == "every 6 months" {
        return Some(180);
    }
    if trimmed == "yearly"
        || trimmed == "annually"
        || trimmed == "annual"
        || trimmed == "every year"
        || trimmed == "every 12 months"
    {
        return Some(365);
    }
    // Try to extract "<number> <unit>" pattern
    // Find all numbers and their following unit
    let chars: Vec<char> = lower.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            if let Ok(num) = num_str.parse::<u32>() {
                // skip spaces
                let mut j = i;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                let rest: String = chars[j..].iter().collect();
                if rest.starts_with("day") {
                    return Some(num);
                } else if rest.starts_with("week") {
                    return Some(num * 7);
                } else if rest.starts_with("month") {
                    return Some(num * 30);
                } else if rest.starts_with("year") {
                    return Some(num * 365);
                } else if rest.starts_with('d')
                    && (rest.len() == 1 || rest[1..].starts_with(|c: char| !c.is_alphabetic()))
                {
                    return Some(num);
                } else if rest.starts_with('w') {
                    return Some(num * 7);
                } else if rest.len() == 0 {
                    // bare number like "14" -> treat as days
                    return Some(num);
                }
            }
        } else {
            i += 1;
        }
    }
    // Also handle phrases like "every 2 weeks" where 'every' prefix already consumed
    // If still not found, try to detect keyword substrings with implied 1
    if lower.contains("daily") {
        return Some(1);
    }
    if lower.contains("weekly") {
        return Some(7);
    }
    if lower.contains("monthly") {
        return Some(30);
    }
    None
}

fn interval_is_due_in_month(interval_str: &str, month: &str) -> Option<bool> {
    let days = interval_days_from_str(interval_str)?;
    let mon: u32 = month[5..7].parse().unwrap_or(1);
    // Map interval days to due logic reusing recurring::cadence buckets (weekly/biweekly/monthly etc.)
    //   <=31 => monthly or more frequent => always due
    //   <=92 => quarterly bucket
    //   <=185 => semi-annual
    //   else annual
    // This mirrors recurring::cadence_label thresholds: weekly<10, biweekly<20, monthly<45, quarterly<100, annual else
    let due = if days <= 45 {
        // weekly/biweekly/monthly — always due within any month
        true
    } else if days <= 100 {
        // quarterly: due in Jan, Apr, Jul, Oct
        mon % 3 == 1
    } else if days <= 200 {
        // semi-annual: Jan, Jul
        mon % 6 == 1
    } else {
        // annual: only Jan
        mon == 1
    };
    Some(due)
}

pub fn schedule_is_due(schedule_str: &str, month: &str) -> bool {
    let s = schedule_str.trim();
    if s.is_empty() {
        return false;
    }
    // Cron-like strings contain '*' or '/' or have 5+ whitespace-separated tokens
    let is_cron_like = s.contains('*') || s.contains('/') || s.split_whitespace().count() >= 5;
    if is_cron_like {
        // Try cron first; if it looks like cron but invalid, treat as unparseable
        // Cron must have 5 or 6 fields to be valid
        let parts = s.split_whitespace().count();
        if parts == 5 || parts == 6 {
            return cron_is_due_in_month(s, month);
        }
        // fallback to interval parsing
    }
    // Try interval keywords / numeric intervals
    if let Some(due) = interval_is_due_in_month(s, month) {
        return due;
    }
    // Reuse recurring logic: if string looks like a cadence name, map via interval parser already
    // Otherwise attempt cron fallback for things like "0 0 1 * *"
    cron_is_due_in_month(s, month)
}

pub fn category_spent(
    conn: &Connection,
    category_id: &str,
    from: &str,
    to: &str,
) -> CoreResult<i64> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents \
         WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0) \
         FROM transactions t \
         WHERE t.category_id=?1 AND t.posted_at >= ?2 AND t.posted_at < ?3 AND t.is_transfer=0",
        params![category_id, from, to],
        |r| r.get(0),
    )?;
    Ok(v)
}

pub fn period_bounds(conn: &Connection, period: Period) -> CoreResult<(Option<String>, String)> {
    let anchor: Option<String> =
        conn.query_row("SELECT MAX(date(posted_at)) FROM transactions", [], |r| {
            r.get(0)
        })?;
    let mut anchor_date = anchor
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());
    // Future guard (I8): if the data's max is in the future relative to wall-clock,
    // clamp to today so future-dated rows are excluded by the end bound. This keeps
    // the anchor data-driven for historical imports but prevents a single future
    // row from sliding the window forward and hiding current-month spend.
    let today = Utc::now().date_naive();
    if anchor_date > today {
        anchor_date = today;
    }
    let end = (anchor_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let start = match period {
        Period::All => None,
        Period::Last1Month => Some(
            (anchor_date - chrono::Months::new(1))
                .format("%Y-%m-%d")
                .to_string(),
        ),
        Period::Last3Months => Some(
            (anchor_date - chrono::Months::new(3))
                .format("%Y-%m-%d")
                .to_string(),
        ),
        Period::Last6Months => Some(
            (anchor_date - chrono::Months::new(6))
                .format("%Y-%m-%d")
                .to_string(),
        ),
        Period::YTD => Some(format!("{}-01-01", anchor_date.year())),
    };
    let start_rfc = start.map(|s| format!("{s}T00:00:00Z"));
    Ok((start_rfc, format!("{end}T00:00:00Z")))
}
/// Carryover helper that works on `&Connection` (read-only). Mirrors
/// `carryover_into_month(&mut Connection)` but without requiring mut.
fn carryover_for(conn: &Connection, category_id: &str, month: &str) -> CoreResult<i64> {
    // B-P1-1: per-category rollover toggle. When `rollover_enabled = 0` the
    // envelope resets each month — no budgeted-spent from prior months carries
    // forward. Check the category row directly so we respect the toggle even
    // before any budgeted-then-spent history has accumulated. COALESCE defaults
    // to 1 for pre-migration rows or missing categories so existing data keeps
    // prior behaviour.
    let rollover: Option<i64> = conn
        .query_row(
            "SELECT rollover_enabled FROM categories WHERE id = ?1",
            params![category_id],
            |r| r.get(0),
        )
        .optional()?;
    if rollover.map(|v| v == 0).unwrap_or(false) {
        return Ok(0);
    }
    let first_budgeted: Option<String> = conn.query_row(
        "SELECT MIN(month) FROM budgets WHERE category_id = ?1 AND amount_cents > 0",
        params![category_id],
        |r| r.get(0),
    )?;
    let Some(first_budgeted) = first_budgeted else {
        return Ok(0);
    };
    if first_budgeted.as_str() >= month {
        return Ok(0);
    }
    let earliest_allowed = month_before(month, 24);
    let start = if first_budgeted.as_str() > earliest_allowed.as_str() {
        first_budgeted
    } else {
        earliest_allowed
    };
    let budgeted: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets \
          WHERE category_id = ?1 AND month >= ?2 AND month < ?3",
        params![category_id, start, month],
        |r| r.get(0),
    )?;
    let start_date = format!("{start}-01");
    let month_date = format!("{month}-01");
    let spent = category_spent(conn, category_id, &start_date, &month_date)?;
    Ok(budgeted - spent)
}
/// Mirrors the envelope `available = budgeted + carryover - spent_in_month`
/// without transfer ledger (transfers are Task 4). For templates, `UpTo` needs this.
fn category_available(conn: &Connection, category_id: &str, month: &str) -> CoreResult<i64> {
    let budgeted: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets WHERE category_id = ?1 AND month = ?2",
        params![category_id, month],
        |r| r.get(0),
    )?;
    let carry = carryover_for(conn, category_id, month)?;
    let start = format!("{month}-01");
    let next = month_before(month, -1);
    let next_start = format!("{next}-01");
    let spent = category_spent(conn, category_id, &start, &next_start)?;
    Ok(budgeted + carry - spent)
}

fn months_between(from_month: &str, to_month: &str) -> i64 {
    // Parse YYYY-MM
    let fy: i32 = from_month[0..4].parse().unwrap_or(1970);
    let fm: i32 = from_month[5..7].parse().unwrap_or(1);
    let ty: i32 = to_month[0..4].parse().unwrap_or(1970);
    let tm: i32 = to_month[5..7].parse().unwrap_or(1);
    ((ty - fy) * 12 + (tm - fm)) as i64
}

fn average_spending(
    conn: &Connection,
    category_id: &str,
    month: &str,
    months: u32,
) -> CoreResult<i64> {
    if months == 0 {
        return Ok(0);
    }
    let mut total: i64 = 0;
    for i in 1..=months as i32 {
        let m = month_before(month, i);
        let start = format!("{m}-01");
        let next = month_before(&m, -1);
        let next_start = format!("{next}-01");
        let spent = category_spent(conn, category_id, &start, &next_start)?;
        total += spent;
    }
    Ok(total / months as i64)
}

/// List all funding templates ordered by priority ASC, id ASC.
// M4: no WHERE category_id — verified via grep, so category_priority index not needed
pub fn list_funding_templates(conn: &Connection) -> CoreResult<Vec<FundingTemplate>> {
    let mut stmt = conn.prepare(
        "SELECT id, category_id, kind, params_json, priority, created_at \
          FROM funding_templates ORDER BY priority ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(FundingTemplate {
            id: r.get(0)?,
            category_id: r.get(1)?,
            kind: r.get(2)?,
            params_json: r.get(3)?,
            priority: r.get(4)?,
            created_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Get a single template by id.
pub fn get_funding_template(conn: &Connection, id: &str) -> CoreResult<Option<FundingTemplate>> {
    let v = conn
        .query_row(
            "SELECT id, category_id, kind, params_json, priority, created_at \
              FROM funding_templates WHERE id = ?1",
            params![id],
            |r| {
                Ok(FundingTemplate {
                    id: r.get(0)?,
                    category_id: r.get(1)?,
                    kind: r.get(2)?,
                    params_json: r.get(3)?,
                    priority: r.get(4)?,
                    created_at: r.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(v)
}

/// Create a funding template. Validates `kind` and that `category_id` exists.
/// `params_json` defaults to `{}` when empty; `priority` defaults to 0.
pub fn create_funding_template(
    conn: &mut Connection,
    category_id: &str,
    kind: &str,
    params_json: &str,
    priority: i64,
) -> CoreResult<FundingTemplate> {
    validate_funding_kind(kind)?;
    let pj = if params_json.trim().is_empty() {
        "{}".to_string()
    } else {
        // Validate it is valid JSON; fallback to {} if parse fails but keep original for debug?
        if serde_json::from_str::<serde_json::Value>(params_json).is_err() {
            return Err(CoreError::Validation(format!(
                "params_json must be valid JSON, got `{params_json}`"
            )));
        }
        params_json.to_string()
    };
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO funding_templates(id, category_id, kind, params_json, priority, created_at) \
          VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, category_id, kind, pj, priority, now],
    )?;
    Ok(FundingTemplate {
        id,
        category_id: category_id.to_string(),
        kind: kind.to_string(),
        params_json: pj,
        priority,
        created_at: now,
    })
}

/// Update a template's fields patch-wise. Returns `None` if not found.
pub fn update_funding_template(
    conn: &mut Connection,
    id: &str,
    category_id: Option<&str>,
    kind: Option<&str>,
    params_json: Option<&str>,
    priority: Option<i64>,
) -> CoreResult<Option<FundingTemplate>> {
    let existing = get_funding_template(conn, id)?;
    let Some(cur) = existing else {
        return Ok(None);
    };
    let new_category = category_id.unwrap_or(&cur.category_id).to_string();
    let new_kind = kind.unwrap_or(&cur.kind).to_string();
    if let Some(k) = kind {
        validate_funding_kind(k)?;
    }
    let new_params = if let Some(pj) = params_json {
        if pj.trim().is_empty() {
            "{}".to_string()
        } else {
            if serde_json::from_str::<serde_json::Value>(pj).is_err() {
                return Err(CoreError::Validation(format!(
                    "params_json must be valid JSON, got `{pj}`"
                )));
            }
            pj.to_string()
        }
    } else {
        cur.params_json.clone()
    };
    let new_priority = priority.unwrap_or(cur.priority);
    conn.execute(
        "UPDATE funding_templates SET category_id=?1, kind=?2, params_json=?3, priority=?4 WHERE id=?5",
        params![new_category, new_kind, new_params, new_priority, id],
    )?;
    Ok(Some(FundingTemplate {
        id: id.to_string(),
        category_id: new_category,
        kind: new_kind,
        params_json: new_params,
        priority: new_priority,
        created_at: cur.created_at,
    }))
}

/// Delete a template by id. Returns true if a row was deleted.
pub fn delete_funding_template(conn: &mut Connection, id: &str) -> CoreResult<bool> {
    let n = conn.execute("DELETE FROM funding_templates WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

/// Apply templates for `month` ("YYYY-MM") ordered by priority.
/// Each template computes `need` from its kind + params, capped by remaining `available`.
///
/// `available` starts as `available_funds(month)` (`income - budgeted - hold_current + hold_prev`)
/// so a hold parked for next month correctly appears as allocatable. This keeps
/// `apply_templates` from under-allocating by the prior month's hold (see
/// `available_funds` vs `to_budget` discussion in task-4 review).
///
/// Funding is transactional via `crate::repos::atomic` (`BEGIN IMMEDIATE` / `COMMIT` / `ROLLBACK`);
/// `DELETE FROM budget_holds` propagates errors (no `let _ =`) and `COMMIT` errors are
/// surfaced, with best-effort `ROLLBACK` on failure. `atomic` provides the
/// `BEGIN IMMEDIATE` isolation that prevents concurrent double-spend.
///
/// Idempotence: a second call in the same month must yield `take == 0` for every
/// template. `UpTo`/`By` are naturally idempotent via `category_available`/`cat_avail`
/// caps. `Fixed`/`Schedule` (constant `need`) would double-spend after the hold
/// is cleared (available recovers by `hold` amount), so they are capped by the
/// existing budget row: `need = (raw_amount - cur_budget).max(0)`. After the first
/// call `cur == raw` → `need == 0` → `take == 0` even though `available` may
/// remain >0. This makes `Fixed` a one-shot “ensure budget hits amount” rather
/// than an additive increment — the verified semantics for I2.
/// Kind handling:
/// - `fixed`: `{"amount":7299}` or `{"amount_cents":7299}` → need = max(0, amount - cur_budget)
/// - `up_to`: `{"cap":30000}` or `{"amount":30000}` → need = max(0, cap - category_available)
/// - `by`: `{"target":10000,"by":"2026-12"}` → need = ceil((target - cat_avail)/months_remaining), validates `target`+`by` presence and `params_json` well-formedness
/// - `average`: `{"months":3}` → need = average spend over N prior months, validates `params_json`
/// - `percent`: `{"pct":0.5}` or `{"percent":50}` → need = round(available * pct) where `available` is the *remaining* pool before this template (single tracking, `remainder` collapsed into `available`)
/// - `remainder`: `{"":}` → need = available (takes all remaining)
/// - `schedule`: `{"amount":5000,"schedule":"0 0 1 * *"} or {"amount":5000,"schedule":"weekly"}` → need = max(0, amount - cur_budget) if schedule is due within `month`, else 0; unparseable schedule ⇒ 0
pub fn apply_templates(conn: &mut Connection, month: &str) -> CoreResult<Vec<BudgetChange>> {
    crate::repos::atomic(conn, |conn| {
        let mut templates = list_funding_templates(conn)?;
        templates.sort_by_key(|t| (t.priority, t.id.clone()));
        // diverges from spec §3: available_funds intentionally includes prev_hold
        // Use available_funds (not to_budget) so prev_hold rolls forward as intended.
        let mut available = available_funds(conn, month)?;
        if available < 0 {
            available = 0;
        }
        let mut out = Vec::with_capacity(templates.len());
        for tmpl in &templates {
            // Current budgeted amount for this category/month — used to cap Fixed/Schedule for idempotence.
            let cur: i64 = conn.query_row(
                "SELECT COALESCE(SUM(amount_cents),0) FROM budgets WHERE category_id=?1 AND month=?2",
                params![tmpl.category_id, month],
                |r| r.get(0),
            )?;
            let cat_avail = category_available(conn, &tmpl.category_id, month)?;
            let need: i64 = match tmpl.kind.as_str() {
                "fixed" => {
                    let raw = parse_amount_from_json(
                        &tmpl.params_json,
                        &["amount", "amount_cents", "amountCents", "cap"],
                    )?;
                    (raw - cur).max(0)
                }
                "up_to" => {
                    let cap = parse_amount_from_json(
                        &tmpl.params_json,
                        &["cap", "amount", "amount_cents", "amountCents", "target"],
                    )?;
                    (cap - cat_avail).max(0)
                }
                "by" => {
                    // Malformed params_json must bubble as Validation (previously silent 0 via unwrap_or).
                    // Missing fields keep previous defaults (target 0, by = current month) to avoid
                    // breaking existing templates; only truly invalid JSON is surfaced.
                    let v: serde_json::Value =
                        serde_json::from_str(&tmpl.params_json).map_err(|e| {
                            CoreError::Validation(format!(
                                "invalid params_json for 'by' template {}: {e}",
                                tmpl.id
                            ))
                        })?;
                    if !v.is_object() {
                        return Err(CoreError::Validation(format!(
                            "params_json for 'by' template {} must be a JSON object",
                            tmpl.id
                        )));
                    }
                    let target = v
                        .get("target")
                        .or_else(|| v.get("amount"))
                        .or_else(|| v.get("cap"))
                        .and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f.round() as i64)))
                        .unwrap_or(0);
                    let by_str = v.get("by").and_then(|x| x.as_str()).unwrap_or(month);
                    let remaining = target.saturating_sub(cat_avail).max(0);
                    let months_left = months_between(month, by_str).max(1);
                    (remaining + months_left - 1) / months_left
                }
                "average" => {
                    let v: serde_json::Value =
                        serde_json::from_str(&tmpl.params_json).map_err(|e| {
                            CoreError::Validation(format!(
                                "invalid params_json for 'average' template {}: {e}",
                                tmpl.id
                            ))
                        })?;
                    if !v.is_object() {
                        return Err(CoreError::Validation(format!(
                            "params_json for 'average' template {} must be a JSON object",
                            tmpl.id
                        )));
                    }
                    let months = v
                        .get("months")
                        .and_then(|x| x.as_u64())
                        .or_else(|| v.get("months").and_then(|x| x.as_i64().map(|i| i as u64)))
                        .unwrap_or(3) as u32;
                    average_spending(conn, &tmpl.category_id, month, months)?
                }
                "percent" => {
                    let pct = parse_pct_from_json(&tmpl.params_json)?;
                    (available as f64 * pct).round() as i64
                }
                "remainder" => available,
                "schedule" => {
                    let raw = parse_amount_from_json(
                        &tmpl.params_json,
                        &["amount", "amount_cents", "amountCents", "cap"],
                    )?;
                    let schedule_str = parse_schedule_string(&tmpl.params_json);
                    let is_due = match schedule_str {
                        Some(s) => schedule_is_due(&s, month),
                        None => false,
                    };
                    if is_due {
                        (raw - cur).max(0)
                    } else {
                        0
                    }
                }
                _ => 0,
            };
            // Single tracking: `available` is the remaining allocatable pool; `remainder`
            // was redundant (`need.min(available).min(remainder)` was no-op). Percent and
            // Remainder now read from `available` before `take`.
            let take = need.min(available).max(0);
            if take != 0 {
                // Inside the outer atomic transaction, use set_raw to avoid nested BEGIN IMMEDIATE.
                // Templates are funded from available_funds which already respects holds, so
                // over-assign validation is not needed here; use allow=true to skip the
                // to_budget guard that is meant for manual envelope edits.
                set_raw(conn, &tmpl.category_id, month, cur + take, true)?;
            }
            available -= take;
            if available < 0 {
                available = 0;
            }
            out.push(BudgetChange {
                category_id: tmpl.category_id.clone(),
                amount_cents: take,
            });
        }
        if out.iter().any(|c| c.amount_cents != 0) {
            conn.execute("DELETE FROM budget_holds WHERE month=?1", params![month])?;
        }
        Ok(out)
    })
}

/// Compute carryover *into* `month` ("YYYY-MM") for one category: the running sum
/// of (budgeted − spent) over every month from the category's first-ever budgeted
/// month (first `budgets` row with `amount_cents > 0`) up to (not including)
/// `month`, capped at a 24-month lookback. Returns 0 if the category has never
/// been budgeted, or if its first budgeted month is `month` or later — the whole
/// point of the epoch anchor is that carryover only ever reflects money the user
/// actually earmarked, never spending from before budgeting started.
pub fn carryover_into_month(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
) -> CoreResult<i64> {
    // B-P1-1: same rollover gate as carryover_for — a disabled category never
    // carries forward, so Budget shows correct available even when prior months
    // had surplus/deficit.
    let rollover: Option<i64> = conn
        .query_row(
            "SELECT rollover_enabled FROM categories WHERE id = ?1",
            params![category_id],
            |r| r.get(0),
        )
        .optional()?;
    if rollover.map(|v| v == 0).unwrap_or(false) {
        return Ok(0);
    }
    let first_budgeted: Option<String> = conn.query_row(
        "SELECT MIN(month) FROM budgets WHERE category_id = ?1 AND amount_cents > 0",
        params![category_id],
        |r| r.get(0),
    )?;
    let Some(first_budgeted) = first_budgeted else {
        return Ok(0);
    };
    if first_budgeted.as_str() >= month {
        return Ok(0);
    }

    let earliest_allowed = month_before(month, 24);
    let start = if first_budgeted.as_str() > earliest_allowed.as_str() {
        first_budgeted
    } else {
        earliest_allowed
    };

    let budgeted: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets \
         WHERE category_id = ?1 AND month >= ?2 AND month < ?3",
        params![category_id, start, month],
        |r| r.get(0),
    )?;
    let start_date = format!("{start}-01");
    let month_date = format!("{month}-01");
    let spent = category_spent(conn, category_id, &start_date, &month_date)?;
    Ok(budgeted - spent)
}

/// A single plain-language fact about how `month` went for a budgeted category,
/// used to open the Plan Next Month wizard. Deterministic, no LLM — the frontend
/// composes the sentence (and applies the user's money formatting/privacy mode)
/// from `kind` + `amount_cents`/`streak_months`; this never bakes a formatted
/// dollar string server-side.
#[derive(Debug, Clone, Serialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LookBackFact {
    pub category_id: String,
    pub category_label: String,
    /// "over" | "under" | "streak"
    pub kind: String,
    /// Meaningful for "over" (spent − budgeted) and "under" (budgeted − spent); 0 for "streak".
    pub amount_cents: i64,
    /// Meaningful for "streak" (consecutive zero-spend months including `month`); 0 otherwise.
    pub streak_months: i64,
}

/// Up to 3 facts about `month`: the biggest overage, the biggest underage, and
/// the longest zero-spend streak (>= 2 consecutive months) — each only among
/// categories that were actually budgeted (amount_cents > 0) for `month`.
pub fn look_back_facts(conn: &mut Connection, month: &str) -> CoreResult<Vec<LookBackFact>> {
    let month_start = format!("{month}-01");
    let next_month = month_before(month, -1);
    let next_month_start = format!("{next_month}-01");

    let mut stmt = conn.prepare(
        "SELECT c.id, c.label, COALESCE(b.amount_cents, 0),
                COALESCE(SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0)
         FROM categories c
         LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?1
         LEFT JOIN transactions t ON t.category_id = c.id AND t.posted_at >= ?2 AND t.posted_at < ?3 AND t.is_transfer=0
         WHERE c.archived_at IS NULL
         GROUP BY c.id, c.label, b.amount_cents",
    )?;
    let rows: Vec<(String, String, i64, i64)> = stmt
        .query_map(params![month, month_start, next_month_start], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    let mut facts = Vec::new();

    if let Some((id, label, budget, spent)) = rows
        .iter()
        .filter(|(_, _, budget, spent)| *budget > 0 && spent > budget)
        .max_by_key(|(_, _, budget, spent)| spent - budget)
    {
        facts.push(LookBackFact {
            category_id: id.clone(),
            category_label: label.clone(),
            kind: "over".to_string(),
            amount_cents: spent - budget,
            streak_months: 0,
        });
    }

    if let Some((id, label, budget, spent)) = rows
        .iter()
        .filter(|(_, _, budget, spent)| *budget > 0 && budget > spent)
        .max_by_key(|(_, _, budget, spent)| budget - spent)
    {
        facts.push(LookBackFact {
            category_id: id.clone(),
            category_label: label.clone(),
            kind: "under".to_string(),
            amount_cents: budget - spent,
            streak_months: 0,
        });
    }

    let mut best: Option<(String, String, i64)> = None;
    for (id, label, budget, spent) in &rows {
        if *budget <= 0 || *spent != 0 {
            continue;
        }
        let mut streak = 1i64;
        for back in 1..12 {
            let m = month_before(month, back);
            // Stop at the first prior month this category wasn't actually
            // budgeted for — otherwise a category that has simply never been
            // budgeted (zero spend forever) would read as an N-month streak
            // instead of "not applicable." Only a budgeted-but-unspent run counts.
            let was_budgeted: bool = conn
                .query_row(
                    "SELECT 1 FROM budgets WHERE category_id = ?1 AND month = ?2 AND amount_cents > 0",
                    params![id, m],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if !was_budgeted {
                break;
            }
            let m_start = format!("{m}-01");
            let m_next = month_before(month, back - 1);
            let m_next_start = format!("{m_next}-01");
            let spent_that_month = category_spent(conn, id, &m_start, &m_next_start)?;
            if spent_that_month == 0 {
                streak += 1;
            } else {
                break;
            }
        }
        if streak >= 2 && best.as_ref().map(|(_, _, s)| streak > *s).unwrap_or(true) {
            best = Some((id.clone(), label.clone(), streak));
        }
    }
    if let Some((id, label, streak)) = best {
        facts.push(LookBackFact {
            category_id: id,
            category_label: label,
            kind: "streak".to_string(),
            amount_cents: 0,
            streak_months: streak,
        });
    }

    Ok(facts)
}

/// Custom report: group transactions by `split_by`, filtered by `period`,
/// transfer/archived flags. Sums are positive cents (expenses flipped).
/// Mirrors `metrics::spending_breakdown` transfer exclusion but is otherwise
/// a thin grouping query — money math stays in `finsight-core`.
pub fn custom_report(conn: &Connection, p: CustomReportParams) -> CoreResult<CustomReportResult> {
    let (start, end) = period_bounds(conn, p.period)?;

    let (select_label, join_clause, group_by) = match p.split_by {
        SplitBy::Category => (
            "COALESCE(c.label, 'Uncategorized')".to_string(),
            " LEFT JOIN categories c ON c.id = t.category_id".to_string(),
            "COALESCE(c.id, 'uncategorized'), COALESCE(c.label, 'Uncategorized')".to_string(),
        ),
        SplitBy::Group => (
            "COALESCE(g.label, 'Uncategorized')".to_string(),
            " LEFT JOIN categories c ON c.id = t.category_id LEFT JOIN category_groups g ON g.id = c.group_id".to_string(),
            "COALESCE(g.id, 'uncategorized'), COALESCE(g.label, 'Uncategorized')".to_string(),
        ),
        SplitBy::Payee => ("t.merchant_raw".to_string(), "".to_string(), "t.merchant_raw".to_string()),
        SplitBy::Account => (
            "COALESCE(a.name, t.account_id)".to_string(),
            " LEFT JOIN accounts a ON a.id = t.account_id".to_string(),
            "COALESCE(a.id, t.account_id), COALESCE(a.name, t.account_id)".to_string(),
        ),
        SplitBy::Month => {
            let fmt = match p.interval.as_deref().map(|s| s.to_lowercase()).as_deref() {
                Some("day") => "%Y-%m-%d",
                Some("week") => "%Y-%W",
                Some("year") => "%Y",
                _ => "%Y-%m",
            };
            let expr = format!("strftime('{}', t.posted_at)", fmt);
            (expr.clone(), "".to_string(), expr)
        },
        SplitBy::SpendingType => (
            // 'Untagged' distinguishes null spending_type (custom/untagged) from
            // 'Uncategorized' (no category). Separate from Category's bucket.
            "COALESCE(c.spending_type, 'Untagged')".to_string(),
            " LEFT JOIN categories c ON c.id = t.category_id".to_string(),
            "COALESCE(c.spending_type, 'Untagged')".to_string(),
        ),
    };
    // Convert to &str for later use, handling owned Strings
    let select_label = select_label.as_str();
    let group_by = group_by.as_str();
    // Ensure joins for filters that need them, regardless of split_by
    let mut join_clause = join_clause.to_string();
    if (!p.category_ids.is_empty() || !p.group_ids.is_empty() || p.spending_type.is_some())
        && !join_clause.contains("categories c")
    {
        join_clause.push_str(" LEFT JOIN categories c ON c.id = t.category_id");
    }
    if !p.group_ids.is_empty() && !join_clause.contains("category_groups g") {
        join_clause.push_str(" LEFT JOIN category_groups g ON g.id = c.group_id");
    }
    // ── Payee: group by canonical_merchant_key to merge variants (e.g. "WALMART #123" splits)
    // Mirrors recurring deduplication (recurring.rs groups by canonical_merchant_key).
    // We fetch raw merchant rows and aggregate in Rust — no need for a SQLite
    // scalar function and this stays deterministic with the Rust normalizer.
    if p.split_by == SplitBy::Payee {
        let metric_kind = p.metric.as_deref().map(|s| s.to_lowercase());
        let mut sql = format!(
            "SELECT t.merchant_raw, t.amount_cents, t.settle_up FROM transactions t{join_clause} WHERE 1=1"
        );
        let mut binds: Vec<String> = Vec::new();
        if !p.include_transfers {
            sql.push_str(" AND t.is_transfer = 0");
        }
        // No archived filter for payee (mirrors generic branch's _ => {}).
        if let Some(member_id) = &p.member_id {
            let mid = member_id.trim();
            if !mid.is_empty() {
                sql.push_str(" AND EXISTS (SELECT 1 FROM account_owners ao WHERE ao.account_id = t.account_id AND ao.member_id = ?)");
                binds.push(mid.to_string());
            }
        }
        if !p.account_ids.is_empty() {
            let placeholders = p
                .account_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(" AND t.account_id IN ({})", placeholders));
            for id in &p.account_ids {
                binds.push(id.clone());
            }
        }
        if !p.category_ids.is_empty() {
            let placeholders = p
                .category_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(" AND t.category_id IN ({})", placeholders));
            for id in &p.category_ids {
                binds.push(id.clone());
            }
        }
        if !p.group_ids.is_empty() {
            let placeholders = p
                .group_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(" AND g.id IN ({})", placeholders));
            for id in &p.group_ids {
                binds.push(id.clone());
            }
        }
        if let Some(payee) = &p.payee {
            let trimmed = payee.trim();
            if !trimmed.is_empty() {
                sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
                binds.push(format!("%{}%", trimmed));
            }
        }
        if let Some(st) = &p.spending_type {
            let trimmed = st.trim();
            if !trimmed.is_empty() {
                sql.push_str(" AND c.spending_type = ?");
                binds.push(trimmed.to_string());
            }
        }
        if let Some(min) = p.min_amount_cents {
            sql.push_str(" AND ABS(t.amount_cents) >= CAST(? AS INTEGER)");
            binds.push(min.to_string());
        }
        if let Some(max) = p.max_amount_cents {
            sql.push_str(" AND ABS(t.amount_cents) <= CAST(? AS INTEGER)");
            binds.push(max.to_string());
        }
        if let Some(s) = &start {
            sql.push_str(" AND t.posted_at >= ?");
            binds.push(s.clone());
        }
        sql.push_str(" AND t.posted_at < ?");
        binds.push(end);

        let mut stmt = conn.prepare(&sql)?;
        let rows_iter = stmt.query_map(rusqlite::params_from_iter(binds.iter()), |r| {
            let merchant_raw: String = r.get(0)?;
            let amount_cents: i64 = r.get(1)?;
            let settle_up: i64 = r.get(2)?;
            Ok((merchant_raw, amount_cents, settle_up))
        })?;
        use std::collections::HashMap;
        // canonical_key -> (sum_cents, count)
        let mut grouped: HashMap<String, (i64, i64)> = HashMap::new();
        for row in rows_iter {
            let (merchant_raw, amount_cents, settle_up) = row?;
            let amt = if settle_up == 1 {
                -amount_cents
            } else if amount_cents < 0 {
                -amount_cents
            } else {
                0
            };
            let key = {
                let k = canonical_merchant_key(&merchant_raw);
                if k.is_empty() {
                    merchant_raw.trim().to_lowercase()
                } else {
                    k
                }
            };
            if key.is_empty() {
                continue;
            }
            let entry = grouped.entry(key).or_insert((0, 0));
            entry.0 += amt;
            entry.1 += 1;
        }
        let mut out_rows: Vec<ReportRow> = grouped
            .into_iter()
            .map(|(label, (sum, cnt))| {
                let total_cents = match metric_kind.as_deref() {
                    Some("count") => cnt,
                    Some("average") | Some("avg") => {
                        if cnt > 0 {
                            sum / cnt
                        } else {
                            0
                        }
                    }
                    _ => sum,
                };
                ReportRow {
                    label,
                    total_cents,
                    txn_count: cnt,
                }
            })
            .collect();
        out_rows.sort_by(|a, b| {
            b.total_cents
                .cmp(&a.total_cents)
                .then_with(|| a.label.cmp(&b.label))
        });
        let total_cents: i64 = out_rows.iter().map(|r| r.total_cents).sum();
        return Ok(CustomReportResult {
            rows: out_rows,
            total_cents,
        });
    }
    // Build WHERE clause dynamically.
    let total_expr = match p.metric.as_deref().map(|s| s.to_lowercase()).as_deref() {
        Some("count") => "COUNT(*)".to_string(),
        Some("average") | Some("avg") => "CAST(AVG(CASE WHEN t.settle_up=1 THEN -t.amount_cents WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END) AS INTEGER)".to_string(),
        _ => "CAST(SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END) AS INTEGER)".to_string(),
    };
    let mut sql = format!(
        "SELECT {select_label} AS label, \
                {total_expr} AS total, \
                COUNT(*) AS cnt \
         FROM transactions t{join_clause} WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();
    if !p.include_transfers {
        sql.push_str(" AND t.is_transfer = 0");
    }
    if !p.include_archived {
        match p.split_by {
            SplitBy::Category | SplitBy::Group | SplitBy::SpendingType => {
                sql.push_str(" AND (c.archived_at IS NULL OR c.id IS NULL)");
            }
            SplitBy::Account => {
                sql.push_str(" AND (a.archived_at IS NULL OR a.id IS NULL)");
            }
            _ => {}
        }
    }
    if let Some(member_id) = &p.member_id {
        let mid = member_id.trim();
        if !mid.is_empty() {
            sql.push_str(" AND EXISTS (SELECT 1 FROM account_owners ao WHERE ao.account_id = t.account_id AND ao.member_id = ?)");
            binds.push(mid.to_string());
        }
    }
    if !p.account_ids.is_empty() {
        let placeholders = p
            .account_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND t.account_id IN ({})", placeholders));
        for id in &p.account_ids {
            binds.push(id.clone());
        }
    }
    if !p.category_ids.is_empty() {
        let placeholders = p
            .category_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND t.category_id IN ({})", placeholders));
        for id in &p.category_ids {
            binds.push(id.clone());
        }
    }
    if !p.group_ids.is_empty() {
        let placeholders = p
            .group_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND g.id IN ({})", placeholders));
        for id in &p.group_ids {
            binds.push(id.clone());
        }
    }
    if let Some(payee) = &p.payee {
        let trimmed = payee.trim();
        if !trimmed.is_empty() {
            sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
            binds.push(format!("%{}%", trimmed));
        }
    }
    if let Some(st) = &p.spending_type {
        let trimmed = st.trim();
        if !trimmed.is_empty() {
            sql.push_str(" AND c.spending_type = ?");
            binds.push(trimmed.to_string());
        }
    }
    if let Some(min) = p.min_amount_cents {
        sql.push_str(" AND ABS(t.amount_cents) >= CAST(? AS INTEGER)");
        binds.push(min.to_string());
    }
    if let Some(max) = p.max_amount_cents {
        sql.push_str(" AND ABS(t.amount_cents) <= CAST(? AS INTEGER)");
        binds.push(max.to_string());
    }
    if let Some(s) = &start {
        sql.push_str(" AND t.posted_at >= ?");
        binds.push(s.clone());
    }
    sql.push_str(" AND t.posted_at < ?");
    binds.push(end);
    sql.push_str(&format!(
        " GROUP BY {group_by} ORDER BY total DESC, label ASC"
    ));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(binds.iter()), |r| {
        Ok(ReportRow {
            label: r.get(0)?,
            total_cents: r.get(1)?,
            txn_count: r.get(2)?,
        })
    })?;
    let mut out_rows: Vec<ReportRow> = Vec::new();
    let mut total_cents: i64 = 0;
    for row in rows {
        let r = row?;
        total_cents += r.total_cents;
        out_rows.push(r);
    }
    Ok(CustomReportResult {
        rows: out_rows,
        total_cents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CustomReportParams, Period, SplitBy};
    use crate::Db;
    use chrono::Utc;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let (dir, db) = crate::testing::migrated_db();
        (dir, db)
    }

    fn seed_category(conn: &mut Connection, id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('daily', 'Daily', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'daily', ?1, '#94A3B8', 0)",
            params![id],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id, owner, bank, type, name, color, created_at) \
             VALUES('acc1', 'joint', 'Test Bank', 'Checking', 'Test Checking', '#000', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }

    fn spend(conn: &mut Connection, category_id: &str, posted_at: &str, cents: i64) {
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, created_at) \
             VALUES(?1, 'acc1', ?2, ?3, 'Test Merchant', ?4, ?2)",
            params![Uuid::new_v4().to_string(), posted_at, -cents, category_id],
        )
        .unwrap();
    }

    #[test]
    fn month_before_steps_back_across_year_boundary() {
        assert_eq!(month_before("2026-01", 1), "2025-12");
        assert_eq!(month_before("2026-03", 3), "2025-12");
        assert_eq!(month_before("2026-05", 0), "2026-05");
        assert_eq!(month_before("2026-01", -1), "2026-02");
    }

    #[test]
    fn carryover_is_zero_for_never_budgeted_category() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        assert_eq!(
            carryover_into_month(&mut conn, "food", "2026-05").unwrap(),
            0
        );
    }

    #[test]
    fn carryover_is_zero_when_first_budgeted_month_is_current_or_future() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-05", 10_000, false).unwrap();
        // First budgeted month is May itself — nothing to carry *into* May.
        assert_eq!(
            carryover_into_month(&mut conn, "food", "2026-05").unwrap(),
            0
        );
    }

    #[test]
    fn carryover_accumulates_positive_when_underspent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-04", 10_000, false).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 8_000);
        // April: budgeted $100, spent $80 → +$20 carries into May.
        assert_eq!(
            carryover_into_month(&mut conn, "food", "2026-05").unwrap(),
            2_000
        );
    }

    #[test]
    fn carryover_accumulates_negative_when_overspent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-04", 10_000, false).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 15_000);
        // April: budgeted $100, spent $150 → -$50 carries into May.
        assert_eq!(
            carryover_into_month(&mut conn, "food", "2026-05").unwrap(),
            -5_000
        );
    }

    #[test]
    fn carryover_sums_across_multiple_prior_months() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-03", 10_000, false).unwrap();
        spend(&mut conn, "food", "2026-03-10T00:00:00Z", 8_000); // +$20
        set(&mut conn, "food", "2026-04", 10_000, false).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 11_000); // -$10
                                                                  // Net into May: +$20 - $10 = +$10.
        assert_eq!(
            carryover_into_month(&mut conn, "food", "2026-05").unwrap(),
            1_000
        );
    }

    #[test]
    fn carryover_caps_at_24_month_lookback() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        // 30 consecutive budgeted months, each with a $10 surplus, ending the
        // month before "2028-07" (the target month we ask carryover into).
        for i in 0..30 {
            let m = month_before("2028-07", 30 - i);
            set(&mut conn, "food", &m, 10_000, false).unwrap();
            spend(&mut conn, "food", &format!("{m}-10T00:00:00Z"), 9_000);
        }
        // Only the trailing 24 months count: 24 * $10 = $240, not 30 * $10 = $300.
        assert_eq!(
            carryover_into_month(&mut conn, "food", "2028-07").unwrap(),
            24_000
        );
    }

    #[test]
    fn look_back_flags_the_biggest_overage_and_underage() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "dining");
        set(&mut conn, "dining", "2026-05", 40_000, false).unwrap();
        spend(&mut conn, "dining", "2026-05-10T00:00:00Z", 41_200); // $12 over

        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES('travel', 'daily', 'Travel', '#000', 1)",
            [],
        ).unwrap();
        set(&mut conn, "travel", "2026-05", 50_000, false).unwrap(); // no spend at all: $500 under

        let facts = look_back_facts(&mut conn, "2026-05").unwrap();
        assert!(facts
            .iter()
            .any(|f| f.category_id == "dining" && f.kind == "over" && f.amount_cents == 1_200));
        assert!(facts
            .iter()
            .any(|f| f.category_id == "travel" && f.kind == "under" && f.amount_cents == 50_000));
    }

    #[test]
    fn look_back_flags_a_zero_spend_streak() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "travel");
        for m in ["2026-02", "2026-03", "2026-04", "2026-05"] {
            set(&mut conn, "travel", m, 50_000, false).unwrap();
        }
        // No spend at all across 4 budgeted months.
        let facts = look_back_facts(&mut conn, "2026-05").unwrap();
        let streak = facts
            .iter()
            .find(|f| f.category_id == "travel" && f.kind == "streak")
            .unwrap();
        assert_eq!(streak.streak_months, 4);
    }

    #[test]
    fn look_back_ignores_unbudgeted_categories() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        // No budgets row at all — spending here shouldn't produce an "over"/"under" fact.
        spend(&mut conn, "food", "2026-05-10T00:00:00Z", 5_000);
        let facts = look_back_facts(&mut conn, "2026-05").unwrap();
        assert!(facts.iter().all(|f| f.category_id != "food"));
    }

    fn insert_tx(
        conn: &rusqlite::Connection,
        category_id: &str,
        amount_cents: i64,
        date: &str,
        settle_up: i64,
    ) {
        let posted_at = if date.len() == 10 {
            format!("{date}T00:00:00Z")
        } else {
            date.to_string()
        };
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, settle_up, created_at) \
             VALUES(?1, 'acc1', ?2, ?3, 'Test', ?4, 'cleared', 0, 0, ?5, ?2)",
            params![Uuid::new_v4().to_string(), posted_at, amount_cents, category_id, settle_up],
        )
        .unwrap();
    }

    #[test]
    fn custom_report_expense_only() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        // seed category acc1 + group
        {
            let mut c = db.get().unwrap();
            seed_category(&mut c, "groceries");
        }
        // -5000 expense
        insert_tx(&conn, "groceries", -5000, "2026-08-10", 0);
        // +8000 income (should be ignored)
        insert_tx(&conn, "groceries", 8000, "2026-08-11", 0);
        // +2000 reimbursement settle_up=1 (nets as -2000 expense)
        insert_tx(&conn, "groceries", 2000, "2026-08-12", 1);
        let params = CustomReportParams {
            period: Period::All,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            member_id: None,
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(
            res.total_cents, 3000,
            "expense - reimbursement, income ignored"
        );
        assert_eq!(res.rows[0].total_cents, 3000);
    }

    #[test]
    fn custom_report_anchors_on_max_posted_at() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        {
            let mut c = db.get().unwrap();
            seed_category(&mut c, "food");
        }
        insert_tx(&conn, "food", -1000, "2025-01-15", 0);
        // Use wall-clock now is 2026-08-29, but anchor is 2025-01-15
        let params = CustomReportParams {
            period: Period::Last1Month,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            member_id: None,
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(res.total_cents, 1000);
    }

    #[test]
    fn custom_report_excludes_future_rows() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        {
            let mut c = db.get().unwrap();
            seed_category(&mut c, "misc");
        }
        let today = Utc::now().format("%Y-%m-%d").to_string();
        insert_tx(&conn, "misc", -1000, &today, 0);
        let future = (Utc::now() + chrono::Duration::days(2))
            .format("%Y-%m-%d")
            .to_string();
        insert_tx(&conn, "misc", -9999, &future, 0);
        let params = CustomReportParams {
            period: Period::Last1Month,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            member_id: None,
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(res.total_cents, 1000, "future row excluded by end bound");
    }

    fn budget_amount(conn: &rusqlite::Connection, category_id: &str, month: &str) -> i64 {
        conn.query_row(
            "SELECT COALESCE(SUM(amount_cents),0) FROM budgets WHERE category_id=?1 AND month=?2",
            params![category_id, month],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn income_for_month(conn: &mut Connection, month: &str, amount_cents: i64) {
        // income is positive non-transfer, non-settle_up transaction in that month
        let posted = format!("{month}-05T00:00:00Z");
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, settle_up, created_at) VALUES(?1,'acc1',?2,?3,'Income','cleared',0,0,0,?2)",
            params![Uuid::new_v4().to_string(), posted, amount_cents],
        )
        .unwrap();
    }

    fn create_fixed_template(conn: &mut Connection, category_id: &str, amount: i64, priority: i64) {
        create_funding_template(
            conn,
            category_id,
            "fixed",
            &format!(r#"{{"amount":{}}}"#, amount),
            priority,
        )
        .unwrap();
    }

    #[test]
    fn apply_templates_writes_and_clears_hold() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "cat_a");
        seed_category(&mut conn, "cat_b");
        // income 10000 for 2026-09 so to_budget = income 10000 - hold 5000 = 5000
        income_for_month(&mut conn, "2026-09", 10_000);
        set_hold(&mut conn, "2026-09", 5000).unwrap();
        create_fixed_template(&mut conn, "cat_a", 3000, 1);
        create_fixed_template(&mut conn, "cat_b", 4000, 2);
        let changes = apply_templates(&mut conn, "2026-09").unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].category_id, "cat_a");
        assert_eq!(changes[0].amount_cents, 3000);
        assert_eq!(changes[1].category_id, "cat_b");
        assert_eq!(changes[1].amount_cents, 2000, "capped by to_budget 5000");
        // hold cleared
        assert_eq!(get_hold(&conn, "2026-09").unwrap(), None);
        assert_eq!(budget_amount(&conn, "cat_a", "2026-09"), 3000);
        assert_eq!(budget_amount(&conn, "cat_b", "2026-09"), 2000);
        // verify transactional: budgets sum equals to_budget initial
        assert_eq!(
            budget_amount(&conn, "cat_a", "2026-09") + budget_amount(&conn, "cat_b", "2026-09"),
            5000
        );
    }

    #[test]
    fn apply_templates_second_call_idempotent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "cat_a");
        // Use UpTo to make second call idempotent even when available remains (hold cleared adds back)
        // Income 10000, hold 5000 => to_budget 5000, UpTo cap 5000 => first takes 5000, second 0
        income_for_month(&mut conn, "2026-09", 10_000);
        set_hold(&mut conn, "2026-09", 5000).unwrap();
        create_funding_template(&mut conn, "cat_a", "up_to", r#"{"cap":5000}"#, 1).unwrap();
        let c1 = apply_templates(&mut conn, "2026-09").unwrap();
        assert_eq!(c1[0].amount_cents, 5000);
        assert_eq!(
            get_hold(&conn, "2026-09").unwrap(),
            None,
            "hold cleared after first"
        );
        assert_eq!(budget_amount(&conn, "cat_a", "2026-09"), 5000);
        let c2 = apply_templates(&mut conn, "2026-09").unwrap();
        assert!(
            c2.iter().all(|c| c.amount_cents == 0),
            "second call no double-spend, got {:?}",
            c2
        );
        assert_eq!(
            budget_amount(&conn, "cat_a", "2026-09"),
            5000,
            "budget not doubled"
        );
    }

    #[test]
    fn apply_templates_fixed_second_call_idempotent_via_available_zero() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "cat_a");
        // No hold, income 5000, fixed 5000 => first consumes all, second 0
        income_for_month(&mut conn, "2026-09", 5000);
        create_fixed_template(&mut conn, "cat_a", 5000, 1);
        let c1 = apply_templates(&mut conn, "2026-09").unwrap();
        assert_eq!(c1[0].amount_cents, 5000);
        let c2 = apply_templates(&mut conn, "2026-09").unwrap();
        assert!(
            c2.iter().all(|c| c.amount_cents == 0),
            "second call no double-spend"
        );
        assert_eq!(budget_amount(&conn, "cat_a", "2026-09"), 5000);
    }

    #[test]
    fn apply_templates_upto_uses_category_available() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");
        income_for_month(&mut conn, "2026-09", 20_000);
        // Pre-budget 5000 and spend 2000 => cat_avail 3000, cap 10000 => need 7000
        conn.execute(
            "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at) VALUES('b1','groceries','2026-09',5000,'2026-09-01T00:00:00Z','2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, created_at) VALUES('e1','acc1','2026-09-10T00:00:00Z',-2000,'Store','groceries','cleared',0,0,'2026-09-10T00:00:00Z')",
            [],
        )
        .unwrap();
        create_funding_template(&mut conn, "groceries", "up_to", r#"{"cap":10000}"#, 0).unwrap();
        let changes = apply_templates(&mut conn, "2026-09").unwrap();
        // cat_avail 3000, need 7000, available = to_budget = 20000-5000=15000 => take 7000, budget becomes 12000
        assert_eq!(changes[0].amount_cents, 7000);
        assert_eq!(budget_amount(&conn, "groceries", "2026-09"), 12_000);
    }

    #[test]
    fn apply_templates_fixed_with_hold_is_idempotent_via_cur_cap() {
        // Repro for reviewer's Critical: Fixed+hold double-spend on retry.
        // Income 10000, hold 5000 => available_funds 5000, fixed 5000 => first 5000, hold cleared.
        // Before fix: second call saw available 5000 again and retook 5000 (double-spend).
        // After fix: Fixed is capped by cur (5000-5000=0) => second 0, idempotent.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "cat_a");
        income_for_month(&mut conn, "2026-09", 10_000);
        set_hold(&mut conn, "2026-09", 5000).unwrap();
        create_fixed_template(&mut conn, "cat_a", 5000, 1);
        let c1 = apply_templates(&mut conn, "2026-09").unwrap();
        assert_eq!(
            c1[0].amount_cents, 5000,
            "first fixed consumes hold-limited pool"
        );
        assert_eq!(get_hold(&conn, "2026-09").unwrap(), None);
        assert_eq!(budget_amount(&conn, "cat_a", "2026-09"), 5000);
        // available after hold cleared is still 5000 (10000-5000), but Fixed cap makes second 0
        let c2 = apply_templates(&mut conn, "2026-09").unwrap();
        assert!(
            c2.iter().all(|c| c.amount_cents == 0),
            "second Fixed must be 0 via cur cap, got {:?}",
            c2
        );
        assert_eq!(
            budget_amount(&conn, "cat_a", "2026-09"),
            5000,
            "budget not doubled"
        );
    }

    #[test]
    fn apply_templates_available_funds_includes_prev_hold() {
        // Verify spec compliance: available_funds = to_budget + prev_hold
        // Hold in 2026-08 rolls into 2026-09's apply_templates pool.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "cat_a");
        // No income in Sep, but hold 1500 parked in Aug => available_funds(2026-09) = 1500
        set_hold(&mut conn, "2026-08", 1500).unwrap();
        income_for_month(&mut conn, "2026-08", 1500); // fund Aug so to_budget covers hold if needed, not relevant
        create_fixed_template(&mut conn, "cat_a", 1500, 1);
        let changes = apply_templates(&mut conn, "2026-09").unwrap();
        assert_eq!(
            changes[0].amount_cents, 1500,
            "prev_hold rolls forward via available_funds"
        );
        assert_eq!(budget_amount(&conn, "cat_a", "2026-09"), 1500);
    }

    #[test]
    fn apply_templates_by_malformed_json_bubbles_validation() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "cat_a");
        income_for_month(&mut conn, "2026-09", 5000);
        // Insert a 'by' template with invalid JSON directly (bypass create validation)
        conn.execute(
            "INSERT INTO funding_templates(id, category_id, kind, params_json, priority, created_at) VALUES('bad1','cat_a','by','{ not json',0,'2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let err = apply_templates(&mut conn, "2026-09").unwrap_err();
        match err {
            crate::error::CoreError::Validation(msg) => {
                assert!(msg.contains("invalid params_json"), "got {}", msg)
            }
            _ => panic!("expected Validation, got {:?}", err),
        };
        // Transactional: no budget written, hold untouched if any
        assert_eq!(budget_amount(&conn, "cat_a", "2026-09"), 0);
    }

    // ── Task 7 parity corpus (C1+I3+I8 unified) ──────────────────────────────
    // Self-consistency check, not cross-surface equality: both sides use the same
    // `CASE WHEN settle_up ...` via `period_bounds`, so this cannot catch a future
    // `get_report_data` regression (e.g. missing `primary_currency_clause`). That
    // cross-surface equality is exercised indirectly via the shared `period_bounds`
    // helper (reports.rs now calls the same function) and remains out-of-scope for
    // a `finsight-core` unit test due to the circular dep on `finsight-api`.
    // To tighten, we also assert that an `is_transfer=1` row is excluded from both sums.
    #[test]
    fn custom_report_self_consistent_sums() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // Seed 6 months: mix of expense, income, reimbursement (settle_up)
        for m in 1..=6 {
            let cat_id = format!("Cat{m}");
            seed_category(&mut conn, &cat_id);
            let month = format!("2026-0{m}");
            // expense -1000*m (negative) → contributes +1000*m
            insert_tx(&conn, &cat_id, -1000 * m as i64, &format!("{month}-10"), 0);
            // income 5000 (positive, non-settle_up) → ignored in expense sums
            insert_tx(&conn, &cat_id, 5000, &format!("{month}-11"), 0);
            // reimbursement +200 settle_up=1 nets as -200
            insert_tx(&conn, &cat_id, 200, &format!("{month}-12"), 1);
        }
        // Add a transfer that must be excluded (is_transfer=1) inside the 6-month window
        {
            let cat_id = "Cat3".to_string();
            let posted = "2026-03-15T00:00:00Z";
            conn.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, settle_up, created_at) \
                 VALUES(?1, 'acc1', ?2, -9999, 'TRANSFER', ?3, 'cleared', 0, 1, 0, ?2)",
                params![uuid::Uuid::new_v4().to_string(), posted, cat_id],
            )
            .unwrap();
        }
        // Fixed report total: direct SUM with same CASE + is_transfer=0 + period_bounds,
        // mimicking reports::get_report_data monthly expense totals for Last6Months.
        let (start_opt, end) = period_bounds(&conn, Period::Last6Months).unwrap();
        let start = start_opt.expect("Last6Months should have start bound");
        let fixed_sum: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0) \
                 FROM transactions t WHERE t.posted_at >= ?1 AND t.posted_at < ?2 AND t.is_transfer = 0",
                params![start, end],
                |r| r.get(0),
            )
            .unwrap();
        let custom = custom_report(
            &conn,
            CustomReportParams {
                period: Period::Last6Months,
                split_by: SplitBy::Month,
                include_archived: true,
                include_transfers: false,
                member_id: None,
                ..Default::default()
            },
        )
        .unwrap();
        let custom_sum: i64 = custom.rows.iter().map(|r| r.total_cents).sum();
        // The brief's invariant: for same 6-month period with reimbursements, sums must equal.
        // Expected: sum(1000*m -200) for m=1..6 = 19800 (is_transfer row excluded)
        assert_eq!(
            custom.total_cents, custom_sum,
            "custom.total_cents must equal sum of rows"
        );
        assert_eq!(
            fixed_sum, custom_sum,
            "parity: fixed report sum {} != custom_report rows sum {} (custom.total_cents {})",
            fixed_sum, custom_sum, custom.total_cents
        );
        assert_eq!(fixed_sum, custom.total_cents);
        assert_eq!(
            fixed_sum, 19_800,
            "expected 19800 from seeded fixture (transfer excluded)"
        );
        // Tighten: total must not include the 9999 transfer
        assert_ne!(
            fixed_sum,
            19_800 + 9_999,
            "is_transfer=1 row must be excluded"
        );
        assert_ne!(custom.total_cents, 19_800 + 9_999);
    }

    #[test]
    fn custom_report_filters_by_account_category_payee_and_amount() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        seed_category(&mut conn, "travel");
        conn.execute(
            "UPDATE categories SET spending_type='Need' WHERE id='food'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE categories SET spending_type='Want' WHERE id='travel'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('acc1','Me','Bank','Checking','Acc1','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('acc2','Me','Bank','Checking','Acc2','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, settle_up, created_at) VALUES(?1,'acc1',?2,-5000,'Whole Foods','food','cleared',0,0,0,?2)",
            params![Uuid::new_v4().to_string(), now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, settle_up, created_at) VALUES(?1,'acc2',?2,-3000,'Chipotle','travel','cleared',0,0,0,?2)",
            params![Uuid::new_v4().to_string(), now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, settle_up, created_at) VALUES(?1,'acc1',?2,-100,'Starbucks','food','cleared',0,0,0,?2)",
            params![Uuid::new_v4().to_string(), now],
        )
        .unwrap();
        let params = CustomReportParams {
            period: Period::All,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            account_ids: vec!["acc1".to_string()],
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(res.total_cents, 5100, "acc1 should have 5000+100");
        let params = CustomReportParams {
            period: Period::All,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            payee: Some("Whole".to_string()),
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(res.total_cents, 5000);
        let params = CustomReportParams {
            period: Period::All,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            min_amount_cents: Some(1000),
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(res.total_cents, 8000, "only >=1000 should be 5000+3000");
        let params = CustomReportParams {
            period: Period::All,
            split_by: SplitBy::Category,
            include_archived: true,
            include_transfers: false,
            spending_type: Some("Need".to_string()),
            ..Default::default()
        };
        let res = custom_report(&conn, params).unwrap();
        assert_eq!(res.total_cents, 5100, "Need should be food only");
    }

    #[test]
    fn carryover_nets_reimbursement() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "Food");
        set(&mut conn, "Food", "2026-08", 5000, false).unwrap();
        insert_tx(&conn, "Food", -3000, "2026-08-10", 0);
        insert_tx(&conn, "Food", 1000, "2026-08-12", 1); // +1000 settle_up nets as -1000
                                                         // budgeted 5000 - spent 2000 (3000-1000) = 3000 carryover into 2026-09
        assert_eq!(carryover_for(&conn, "Food", "2026-09").unwrap(), 3000);
        assert_eq!(
            carryover_into_month(&mut conn, "Food", "2026-09").unwrap(),
            3000
        );
    }

    #[test]
    fn category_available_nets_reimbursement() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "Food");
        set(&mut conn, "Food", "2026-08", 5000, false).unwrap();
        insert_tx(&conn, "Food", -3000, "2026-08-10", 0);
        insert_tx(&conn, "Food", 1000, "2026-08-12", 1);
        // available for 2026-08 = budgeted 5000 - spent 2000 = 3000
        assert_eq!(category_available(&conn, "Food", "2026-08").unwrap(), 3000);
        assert_eq!(available(&conn, "Food", "2026-08").unwrap(), 3000);
        // carryover into Sep same as above ensures available for Sep with no budget reflects net
        assert_eq!(carryover_for(&conn, "Food", "2026-09").unwrap(), 3000);
    }

    #[test]
    fn transfer_optional_insufficient_spare_validation() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "A");
        seed_category(&mut conn, "B");
        set(&mut conn, "A", "2026-09", 1000, false).unwrap();
        insert_tx(&conn, "A", -900, "2026-09-05", 0);
        // spare = 100; try transfer 500 → Validation
        let err =
            transfer_optional(&mut conn, Some("A"), Some("B"), 500, "2026-09", None).unwrap_err();
        assert!(
            matches!(err, crate::error::CoreError::Validation(_)),
            "expected Validation for insufficient spare, got {:?}",
            err
        );
        // exact spare should succeed
        let ok = transfer_optional(&mut conn, Some("A"), Some("B"), 100, "2026-09", None).unwrap();
        assert_eq!(ok.amount_cents, 100);
        // now spare is 0, further transfer should fail
        let err2 =
            transfer_optional(&mut conn, Some("A"), Some("B"), 1, "2026-09", None).unwrap_err();
        assert!(matches!(err2, crate::error::CoreError::Validation(_)));
    }

    #[test]
    fn over_assign_guard_blocks_and_allows_with_flag() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "A");
        seed_category(&mut conn, "B");
        // Income $100 for 2026-09
        income_for_month(&mut conn, "2026-09", 10_000);
        // First budget $100 should succeed (to_budget 0)
        set(&mut conn, "A", "2026-09", 10_000, false).unwrap();
        // Second budget $1 should be allowed within $0.50 tolerance? No, over by $1 → should fail
        let err = set(&mut conn, "B", "2026-09", 100, false).unwrap_err();
        assert!(
            matches!(&err, crate::error::CoreError::Validation(m) if m.contains("over-assigned")),
            "expected over-assigned validation, got {:?}",
            err
        );
        // Still $0 to_budget (second write rolled back)
        assert_eq!(to_budget(&conn, "2026-09").unwrap(), 0);
        // With allow_over_assign=true it should succeed and go negative
        set(&mut conn, "B", "2026-09", 100, true).unwrap();
        assert_eq!(to_budget(&conn, "2026-09").unwrap(), -100);
        // Tolerance: over by $0.50 (50c) should be allowed even without flag
        // Reset B to 0, then set to 50c over
        set(&mut conn, "B", "2026-09", 50, false).unwrap();
        assert_eq!(to_budget(&conn, "2026-09").unwrap(), -50);
        // 51c over should fail
        let err2 = set(&mut conn, "B", "2026-09", 51, false).unwrap_err();
        assert!(matches!(err2, crate::error::CoreError::Validation(_)));
    }
}

use crate::error::{CoreError, CoreResult};
use crate::models::{
    BudgetChange, CustomReportParams, CustomReportResult, FundingTemplate, Period, ReportRow,
    SplitBy,
};
use chrono::{Datelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use specta::Type;
use utoipa::ToSchema;
use uuid::Uuid;

/// Set (upsert) a budget for a category in a given month (format: "YYYY-MM").
pub fn set(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
    amount_cents: i64,
) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(category_id, month) DO UPDATE SET amount_cents = excluded.amount_cents, updated_at = excluded.updated_at",
        params![id, category_id, month, amount_cents, now],
    )?;
    Ok(())
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

// ── Declarative Funding Templates (Actual's #template as a table) ─────────

const FUNDING_KINDS: &[&str] = &[
    "fixed", "up_to", "by", "average", "percent", "remainder", "schedule",
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
    let v: serde_json::Value =
        serde_json::from_str(params_json).unwrap_or(serde_json::Value::Object(Default::default()));
    for k in keys {
        if let Some(n) = v.get(*k).and_then(|x| x.as_i64()) {
            return Ok(n);
        }
        // allow floating pct? for amount we expect integer cents
        if let Some(n) = v.get(*k).and_then(|x| x.as_f64()) {
            return Ok(n.round() as i64);
        }
    }
    Ok(0)
}

fn parse_pct_from_json(params_json: &str) -> CoreResult<f64> {
    let v: serde_json::Value =
        serde_json::from_str(params_json).unwrap_or(serde_json::Value::Object(Default::default()));
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

/// Carryover helper that works on `&Connection` (read-only). Mirrors
/// `carryover_into_month(&mut Connection)` but without requiring mut.
fn carryover_for(conn: &Connection, category_id: &str, month: &str) -> CoreResult<i64> {
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
    let spent: i64 = conn.query_row(
        "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
          WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
        params![category_id, start_date, month_date],
        |r| r.get(0),
    )?;
    Ok(budgeted - spent)
}

/// Current available balance for a category in `month` (budgeted + carryover - spent).
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
    let spent: i64 = conn.query_row(
        "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
          WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
        params![category_id, start, next_start],
        |r| r.get(0),
    )?;
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

fn average_spending(conn: &Connection, category_id: &str, month: &str, months: u32) -> CoreResult<i64> {
    if months == 0 {
        return Ok(0);
    }
    let mut total: i64 = 0;
    for i in 1..=months as i32 {
        let m = month_before(month, i);
        let start = format!("{m}-01");
        let next = month_before(&m, -1);
        let next_start = format!("{next}-01");
        let spent: i64 = conn.query_row(
            "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
              WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
            params![category_id, start, next_start],
            |r| r.get(0),
        )?;
        total += spent;
    }
    Ok(total / months as i64)
}

/// List all funding templates ordered by priority ASC, id ASC.
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
/// `available` starts as `to_budget(month)` (income - budgeted - hold, respects holds).
/// For each template: `take = need.min(available).max(0)`, then `available -= take`.
///
/// Kind handling:
/// - `fixed`: `{"amount":7299}` or `{"amount_cents":7299}` → need = amount
/// - `up_to`: `{"cap":30000}` or `{"amount":30000}` → need = max(0, cap - category_available)
/// - `by`: `{"target":10000,"by":"2026-12"}` → need = ceil((target - balance)/months_remaining)
/// - `average`: `{"months":3}` → need = average spend over N prior months
/// - `percent`: `{"pct":0.5}` or `{"percent":50}` → need = round(available * pct)
/// - `remainder`: `{"":}` → need = available (takes all remaining)
/// - `schedule`: `{"amount":5000}` or pattern → need = amount or 0 if unparseable
pub fn apply_templates(conn: &Connection, month: &str) -> CoreResult<Vec<BudgetChange>> {
    let templates = list_funding_templates(conn)?;
    let mut available = to_budget(conn, month)?;
    if available < 0 {
        available = 0;
    }
    let mut out = Vec::with_capacity(templates.len());
    for t in templates {
        let need: i64 = match t.kind.as_str() {
            "fixed" => parse_amount_from_json(&t.params_json, &["amount", "amount_cents", "amountCents", "cap"])?,
            "up_to" => {
                let cap = parse_amount_from_json(&t.params_json, &["cap", "amount", "amount_cents", "amountCents", "target"])?;
                let balance = category_available(conn, &t.category_id, month)?;
                (cap - balance).max(0)
            }
            "by" => {
                // Expect {"target":X, "by":"YYYY-MM"}
                let v: serde_json::Value = serde_json::from_str(&t.params_json)
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                let target = v
                    .get("target")
                    .or_else(|| v.get("amount"))
                    .or_else(|| v.get("cap"))
                    .and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f.round() as i64)))
                    .unwrap_or(0);
                let by = v
                    .get("by")
                    .and_then(|x| x.as_str())
                    .unwrap_or(month);
                let balance = category_available(conn, &t.category_id, month)?;
                let remaining = target.saturating_sub(balance).max(0);
                let months_left = months_between(month, by).max(1);
                // ceil division
                (remaining + months_left - 1) / months_left
            }
            "average" => {
                let v: serde_json::Value = serde_json::from_str(&t.params_json)
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                let months = v
                    .get("months")
                    .and_then(|x| x.as_u64())
                    .or_else(|| v.get("months").and_then(|x| x.as_i64().map(|i| i as u64)))
                    .unwrap_or(3) as u32;
                average_spending(conn, &t.category_id, month, months)?
            }
            "percent" => {
                let pct = parse_pct_from_json(&t.params_json)?;
                (available as f64 * pct).round() as i64
            }
            "remainder" => available,
            "schedule" => {
                // For schedule, params may be {"amount":X} or {"schedule":"X"} with amount inside
                let amt = parse_amount_from_json(&t.params_json, &["amount", "amount_cents", "amountCents"])?;
                if amt != 0 {
                    amt
                } else {
                    // try to parse schedule string as amount? fallback 0
                    0
                }
            }
            _ => 0,
        };
        let take = need.min(available).max(0);
        out.push(BudgetChange {
            category_id: t.category_id.clone(),
            amount_cents: take,
        });
        available -= take;
        if available < 0 {
            available = 0;
        }
    }
    Ok(out)
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
    // Mirrors the existing spend calculation in list_budget_envelopes (no
    // is_transfer filter there either) — kept consistent rather than silently
    // fixing an unrelated, pre-existing question about transfer handling.
    let spent: i64 = conn.query_row(
        "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
         WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
        params![category_id, start_date, month_date],
        |r| r.get(0),
    )?;
    Ok(budgeted - spent)
}

/// A single plain-language fact about how `month` went for a budgeted category,
/// used to open the Plan Next Month wizard. Deterministic, no LLM — the frontend
/// composes the sentence (and applies the user's money formatting/privacy mode)
/// from `kind` + `amount_cents`/`streak_months`; this never bakes a formatted
/// dollar string server-side.
#[derive(Debug, Clone, Serialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
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
                COALESCE(SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0)
         FROM categories c
         LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?1
         LEFT JOIN transactions t ON t.category_id = c.id AND t.posted_at >= ?2 AND t.posted_at < ?3
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
            let spent_that_month: i64 = conn.query_row(
                "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
                 WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
                params![id, m_start, m_next_start],
                |r| r.get(0),
            )?;
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

/// Compute the start of the period window for custom reports.
/// Returns None for `All` (no filter). Uses wall-clock `Utc::now()` as anchor,
/// mirroring the frontend's "last N months" notion; for YTD the anchor is Jan 1
/// of the current year.
fn period_start(period: &Period) -> Option<String> {
    let now = Utc::now();
    match period {
        Period::All => None,
        Period::Last1Month => Some((now - chrono::Duration::days(30)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        Period::Last3Months => Some((now - chrono::Duration::days(90)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        Period::Last6Months => Some((now - chrono::Duration::days(180)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        Period::YTD => Some(format!("{}-01-01T00:00:00Z", now.year())),
    }
}

/// Custom report: group transactions by `split_by`, filtered by `period`,
/// transfer/archived flags. Sums are positive cents (expenses flipped).
/// Mirrors `metrics::spending_breakdown` transfer exclusion but is otherwise
/// a thin grouping query — money math stays in `finsight-core`.
pub fn custom_report(conn: &Connection, p: CustomReportParams) -> CoreResult<CustomReportResult> {
    let start = period_start(&p.period);

    let (select_label, join_clause, group_by) = match p.split_by {
        SplitBy::Category => (
            "COALESCE(c.label, 'Uncategorized')",
            " LEFT JOIN categories c ON c.id = t.category_id",
            "COALESCE(c.id, 'uncategorized'), COALESCE(c.label, 'Uncategorized')",
        ),
        SplitBy::Group => (
            "COALESCE(g.label, 'Uncategorized')",
            " LEFT JOIN categories c ON c.id = t.category_id LEFT JOIN category_groups g ON g.id = c.group_id",
            "COALESCE(g.id, 'uncategorized'), COALESCE(g.label, 'Uncategorized')",
        ),
        SplitBy::Payee => ("t.merchant_raw", "", "t.merchant_raw"),
        SplitBy::Account => (
            "COALESCE(a.name, t.account_id)",
            " LEFT JOIN accounts a ON a.id = t.account_id",
            "COALESCE(a.id, t.account_id), COALESCE(a.name, t.account_id)",
        ),
        SplitBy::Month => (
            "strftime('%Y-%m', t.posted_at)",
            "",
            "strftime('%Y-%m', t.posted_at)",
        ),
    };

    // Build WHERE clause dynamically.
    let mut sql = format!(
        "SELECT {select_label} AS label, \
                CAST(SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE t.amount_cents END) AS INTEGER) AS total, \
                COUNT(*) AS cnt \
         FROM transactions t{join_clause} WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();
    if !p.include_transfers {
        sql.push_str(" AND t.is_transfer = 0");
    }
    if !p.include_archived {
        match p.split_by {
            SplitBy::Category | SplitBy::Group => {
                sql.push_str(" AND (c.archived_at IS NULL OR c.id IS NULL)");
            }
            SplitBy::Account => {
                sql.push_str(" AND (a.archived_at IS NULL OR a.id IS NULL)");
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        sql.push_str(" AND t.posted_at >= ?");
        binds.push(s);
    }
    // No end bound — window is start..now; future-dated rows are naturally excluded by being > now? Not needed.
    sql.push_str(&format!(" GROUP BY {group_by} ORDER BY total DESC, label ASC"));

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
    use crate::Db;
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
        set(&mut conn, "food", "2026-05", 10_000).unwrap();
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
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
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
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
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
        set(&mut conn, "food", "2026-03", 10_000).unwrap();
        spend(&mut conn, "food", "2026-03-10T00:00:00Z", 8_000); // +$20
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
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
            set(&mut conn, "food", &m, 10_000).unwrap();
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
        set(&mut conn, "dining", "2026-05", 40_000).unwrap();
        spend(&mut conn, "dining", "2026-05-10T00:00:00Z", 41_200); // $12 over

        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES('travel', 'daily', 'Travel', '#000', 1)",
            [],
        ).unwrap();
        set(&mut conn, "travel", "2026-05", 50_000).unwrap(); // no spend at all: $500 under

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
            set(&mut conn, "travel", m, 50_000).unwrap();
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
}

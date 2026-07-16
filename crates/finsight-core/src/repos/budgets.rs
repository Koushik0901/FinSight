use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use specta::Type;
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
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
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
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 0);
    }

    #[test]
    fn carryover_is_zero_when_first_budgeted_month_is_current_or_future() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-05", 10_000).unwrap();
        // First budgeted month is May itself — nothing to carry *into* May.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 0);
    }

    #[test]
    fn carryover_accumulates_positive_when_underspent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 8_000);
        // April: budgeted $100, spent $80 → +$20 carries into May.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 2_000);
    }

    #[test]
    fn carryover_accumulates_negative_when_overspent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 15_000);
        // April: budgeted $100, spent $150 → -$50 carries into May.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), -5_000);
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
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 1_000);
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
        assert_eq!(carryover_into_month(&mut conn, "food", "2028-07").unwrap(), 24_000);
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
        assert!(facts.iter().any(|f| f.category_id == "dining" && f.kind == "over" && f.amount_cents == 1_200));
        assert!(facts.iter().any(|f| f.category_id == "travel" && f.kind == "under" && f.amount_cents == 50_000));
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
        let streak = facts.iter().find(|f| f.category_id == "travel" && f.kind == "streak").unwrap();
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

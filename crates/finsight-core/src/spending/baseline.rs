//! "Your normal": robust per-merchant monthly baselines + the grand monthly
//! median, computed on read from the ledger. Clusters on `canonical_merchant_key`
//! (the same normalizer categorization/recurring use) so a merchant's variants
//! collapse to one stream. Honors the metrics-layer exclusions (transfers and
//! investment activity are never spending).

use crate::error::CoreResult;
use crate::merchant::canonical_merchant_key;
use crate::spending::{months_between, stats};
use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MerchantBaseline {
    pub display: String,
    pub category: Option<String>,
    /// Mean monthly spend for this merchant over the baseline (total ÷ months).
    pub monthly_cents: i64,
    /// Mean transactions per month over the baseline.
    pub txns_per_month: f64,
    /// Distinct calendar months this merchant had any spend in.
    pub active_months: i64,
}

#[derive(Debug, Clone)]
pub struct Baseline {
    /// Whole calendar months the baseline spans.
    pub months: i64,
    /// Robust "normal" monthly spend: median of the per-month grand totals.
    pub grand_monthly_median_cents: i64,
    /// Robust spread (MAD) of the per-month grand totals — the volatility band
    /// classify uses to tell an episodic spike from a new regime.
    pub grand_monthly_mad_cents: i64,
    /// Keyed by `canonical_merchant_key`.
    pub per_merchant: HashMap<String, MerchantBaseline>,
    /// Dominant account currency in the window (v1 analyzes one currency).
    pub currency: String,
    /// True when more than one currency appeared (drives a caller warning).
    pub mixed_currency: bool,
}

struct Row {
    key: String,
    display: String,
    ym: String,
    amount_abs: i64,
    category: Option<String>,
    currency: String,
}

/// Load expense rows in `[start, end)` (YYYY-MM-DD), normalized + clustered.
fn load_rows(conn: &Connection, start: &str, end: &str) -> CoreResult<Vec<Row>> {
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT t.merchant_raw, substr(t.posted_at,1,7) AS ym, t.amount_cents, \
                (SELECT label FROM categories c WHERE c.id = t.category_id), \
                COALESCE(a.currency, 'USD') \
         FROM transactions t JOIN accounts a ON a.id = t.account_id \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred} \
           AND substr(t.posted_at,1,10) >= ?1 AND substr(t.posted_at,1,10) < ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![start, end], |r| {
            let raw: String = r.get(0)?;
            Ok(Row {
                key: canonical_merchant_key(&raw),
                display: crate::merchant::split_display(&raw),
                ym: r.get(1)?,
                amount_abs: r.get::<_, i64>(2)?.unsigned_abs() as i64,
                category: r.get(3)?,
                currency: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Compute the baseline over `[start_ym, end_ym)` (both `YYYY-MM`).
pub fn compute(conn: &Connection, start_ym: &str, end_ym: &str) -> CoreResult<Baseline> {
    let (sy, sm) = crate::spending::parse_ym(start_ym);
    let (ey, em) = crate::spending::parse_ym(end_ym);
    let start = format!("{sy:04}-{sm:02}-01");
    let end = format!("{ey:04}-{em:02}-01");
    let rows = load_rows(conn, &start, &end)?;

    // Dominant currency.
    let mut cur_tot: HashMap<String, i64> = HashMap::new();
    for r in &rows {
        *cur_tot.entry(r.currency.clone()).or_default() += r.amount_abs;
    }
    let mixed_currency = cur_tot.len() > 1;
    let currency = cur_tot
        .iter()
        .max_by_key(|(_, v)| **v)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "USD".to_string());

    // Per (merchant, month) and per-month grand totals — dominant currency only.
    let mut m_month: HashMap<String, HashMap<String, (i64, i64)>> = HashMap::new(); // key -> ym -> (sum, count)
    let mut m_display: HashMap<String, String> = HashMap::new();
    let mut m_cat: HashMap<String, Option<String>> = HashMap::new();
    let mut grand: HashMap<String, i64> = HashMap::new(); // ym -> sum
    for r in rows.into_iter().filter(|r| r.currency == currency) {
        let e = m_month.entry(r.key.clone()).or_default().entry(r.ym.clone()).or_insert((0, 0));
        e.0 += r.amount_abs;
        e.1 += 1;
        m_display.entry(r.key.clone()).or_insert(r.display);
        m_cat.entry(r.key.clone()).or_insert(r.category);
        *grand.entry(r.ym).or_default() += r.amount_abs;
    }

    // Effective history DEPTH: from the first month that actually has spend to
    // the window end. Empty PRE-history months (before the user's first
    // transaction) must not deflate the per-merchant divisor or the grand
    // median, and must not mask a genuinely thin baseline — otherwise a
    // brand-new user gets a fabricated "$0 normal, everything is new" with no
    // low-confidence caveat. Interior/trailing quiet months are real zeros and
    // still count. So `months` is history depth, not window span.
    let first_active_ym = grand.keys().min().cloned();
    let months = match &first_active_ym {
        Some(fa) => months_between(fa, end_ym).max(1),
        None => 0,
    };
    let div = months.max(1);

    let per_merchant = m_month
        .into_iter()
        .map(|(key, by_month)| {
            let total: i64 = by_month.values().map(|(s, _)| *s).sum();
            let count: i64 = by_month.values().map(|(_, c)| *c).sum();
            let mb = MerchantBaseline {
                display: m_display.remove(&key).unwrap_or_else(|| key.clone()),
                category: m_cat.remove(&key).flatten(),
                monthly_cents: total / div,
                txns_per_month: count as f64 / div as f64,
                active_months: by_month.len() as i64,
            };
            (key, mb)
        })
        .collect();

    // Robust grand monthly: median over the EFFECTIVE months only (first active
    // month .. window end), zero-filling interior quiet months (real zeros).
    let mut monthly_totals: Vec<f64> = Vec::new();
    if let Some(fa) = &first_active_ym {
        let (fy, fm) = crate::spending::parse_ym(fa);
        for i in 0..months {
            let idx = fy * 12 + (fm as i32 - 1) + i as i32;
            let ym = format!("{:04}-{:02}", idx.div_euclid(12), idx.rem_euclid(12) + 1);
            monthly_totals.push(*grand.get(&ym).unwrap_or(&0) as f64);
        }
    }
    let med = stats::median(&monthly_totals);
    let grand_monthly_median_cents = med.round() as i64;
    let grand_monthly_mad_cents = stats::mad(&monthly_totals, med).round() as i64;

    Ok(Baseline {
        months,
        grand_monthly_median_cents,
        grand_monthly_mad_cents,
        per_merchant,
        currency,
        mixed_currency,
    })
}

/// The trailing `months`-month baseline ending the month BEFORE `period_ym`
/// (so the target month is never inside its own baseline). This is the
/// canonical "your normal" window; the agent tools and classify all use it.
pub fn trailing(conn: &Connection, period_ym: &str, months: i64) -> CoreResult<Baseline> {
    let (py, pm) = crate::spending::parse_ym(period_ym);
    let end = format!("{py:04}-{pm:02}"); // exclusive end = the period month itself
    let start_idx = py * 12 + (pm as i32 - 1) - months as i32;
    let start = format!("{:04}-{:02}", start_idx.div_euclid(12), start_idx.rem_euclid(12) + 1);
    compute(conn, &start, &end)
}

/// Total expense (positive cents) in one calendar month `ym` (`YYYY-MM`),
/// applying the same exclusions as the baseline (transfers + investment out).
pub fn month_total(conn: &Connection, ym: &str) -> CoreResult<i64> {
    let (y, m) = crate::spending::parse_ym(ym);
    let start = format!("{y:04}-{m:02}-01");
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end = format!("{ny:04}-{nm:02}-01");
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT COALESCE(SUM(-t.amount_cents), 0) FROM transactions t \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred} \
           AND substr(t.posted_at,1,10) >= ?1 AND substr(t.posted_at,1,10) < ?2"
    );
    let total: i64 = conn.query_row(&sql, rusqlite::params![start, end], |r| r.get(0))?;
    Ok(total)
}

/// A single category's outflow within a month (positive cents).
#[derive(Debug, Clone)]
pub struct CategorySpend {
    pub label: String,
    pub amount_cents: i64,
}

/// Top `k` spending categories for one calendar month (`YYYY-MM`). Uses the SAME
/// transfer/investment exclusion as `month_total`, so a review's category rows
/// and its month total are computed on one grounded basis (never diverge).
pub fn month_category_breakdown(
    conn: &Connection,
    ym: &str,
    k: usize,
) -> CoreResult<Vec<CategorySpend>> {
    let (y, m) = crate::spending::parse_ym(ym);
    let start = format!("{y:04}-{m:02}-01");
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end = format!("{ny:04}-{nm:02}-01");
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT COALESCE(c.label, 'Uncategorized') AS label, SUM(-t.amount_cents) AS spent \
         FROM transactions t LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred} \
           AND substr(t.posted_at,1,10) >= ?1 AND substr(t.posted_at,1,10) < ?2 \
         GROUP BY label ORDER BY spent DESC LIMIT ?3"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![start, end, k as i64], |r| {
        Ok(CategorySpend {
            label: r.get(0)?,
            amount_cents: r.get(1)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// The most recent calendar month (`YYYY-MM`) with any spending activity, or
/// None if the ledger has none. Lets a caller default "the current period".
pub fn latest_activity_month(conn: &Connection) -> CoreResult<Option<String>> {
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT MAX(substr(t.posted_at,1,7)) FROM transactions t \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred}"
    );
    let ym: Option<String> = conn.query_row(&sql, [], |r| r.get(0))?;
    Ok(ym)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("b.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        {
            let conn = db.get().unwrap();
            conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Credit','Card','USD','#fff',datetime('now'))", []).unwrap();
        }
        (dir, db)
    }

    fn ins(conn: &Connection, ym: &str, cents: i64, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,0,'cleared',datetime('now'))",
            rusqlite::params![format!("{ym}-15T12:00:00Z"), cents, merchant],
        ).unwrap();
    }

    #[test]
    fn baseline_is_robust_and_per_merchant() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        // 12 months: $2,000 groceries each month, plus one $7,000 spike month.
        for i in 0..12 {
            let ym = format!("2025-{:02}", i + 1);
            ins(&conn, &ym, -200_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2025-06", -700_000, "FLAIR AIRLINES  BURNABY, BC"); // spike in June

        let b = compute(&conn, "2025-01", "2026-01").unwrap();
        assert_eq!(b.months, 12);
        // Grand monthly median stays ~ the groceries level, not dragged up by the spike.
        assert!(b.grand_monthly_median_cents <= 220_000, "median resists the spike: {}", b.grand_monthly_median_cents);
        let groceries = b.per_merchant.get(&canonical_merchant_key("SAVE ON FOODS  EDMONTON, AB")).unwrap();
        assert_eq!(groceries.monthly_cents, 200_000);
        assert_eq!(groceries.active_months, 12);
    }

    #[test]
    fn thin_history_uses_real_depth_not_window_span() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        // Only 2 months of spend inside a 12-month-wide window.
        ins(&conn, "2026-03", -50_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-04", -50_000, "SAVE ON FOODS  EDMONTON, AB");
        let b = compute(&conn, "2025-05", "2026-05").unwrap();
        assert_eq!(b.months, 2, "history depth is 2 months, not the 12-month span");
        assert!(b.grand_monthly_median_cents > 0, "median must not be zero-deflated by empty pre-history");
    }

    #[test]
    fn trailing_excludes_the_target_month_and_month_total_sums_it() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-01", -99_000, "FLAIR AIRLINES  BURNABY, BC"); // the target month

        let base = trailing(&conn, "2026-01", 12).unwrap(); // [2025-01, 2026-01)
        assert_eq!(base.months, 12);
        assert!(base.per_merchant.get(&canonical_merchant_key("FLAIR AIRLINES  BURNABY, BC")).is_none());
        assert!(base.grand_monthly_mad_cents >= 0);

        assert_eq!(month_total(&conn, "2026-01").unwrap(), 99_000);
    }

    #[test]
    fn latest_activity_month_finds_the_newest_month() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        assert_eq!(latest_activity_month(&conn).unwrap(), None);
        ins(&conn, "2025-03", -1000, "A  X, BC");
        ins(&conn, "2026-02", -1000, "B  Y, BC");
        assert_eq!(latest_activity_month(&conn).unwrap().as_deref(), Some("2026-02"));
    }
}

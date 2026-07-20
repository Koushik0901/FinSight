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
    /// The currency these figures are denominated in — the primary from
    /// [`crate::currency`], so it agrees with every other aggregate.
    pub currency: String,
    /// True when the user holds money in other currencies. Those rows are
    /// EXCLUDED from the totals above (never converted, never mixed in), so
    /// this flags an incomplete view rather than an unreliable one.
    pub mixed_currency: bool,
}

struct Row {
    key: String,
    display: String,
    ym: String,
    /// Net expense contribution in cents (always >= 0 for an ordinary outflow;
    /// a settle-up inflow nets as `-amount_cents`, so a reimbursement-heavy
    /// merchant/month can legitimately go negative — mirrors metrics.rs).
    net_cents: i64,
    category: Option<String>,
    currency: String,
}

/// Load expense rows in `[start, end)` (YYYY-MM-DD), normalized + clustered.
/// A `settle_up = 1` row is netted (contributes `-amount_cents`) the same way
/// metrics.rs cashflow does, instead of being silently dropped by the
/// `amount_cents < 0` filter.
fn load_rows(conn: &Connection, start: &str, end: &str) -> CoreResult<Vec<Row>> {
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT t.merchant_raw, substr(t.posted_at,1,7) AS ym, \
                CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                     WHEN t.amount_cents < 0 THEN -t.amount_cents \
                     ELSE 0 END AS net_cents, \
                (SELECT label FROM categories c WHERE c.id = t.category_id), \
                COALESCE(a.currency, 'USD') \
         FROM transactions t JOIN accounts a ON a.id = t.account_id \
         WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND {pred} \
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
                net_cents: r.get(2)?,
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

    // Which currency to analyse is decided by `crate::currency`, the same rule
    // every other aggregate uses. This module used to pick its own "dominant"
    // currency by spend volume, which could name a DIFFERENT currency than the
    // metrics layer for the same user — two screens, two answers. Falls back to
    // dominant-by-spend only when there are no accounts to derive from (rows
    // can outlive the account that produced them).
    let profile = crate::currency::currency_profile(conn)?;
    let mut cur_tot: HashMap<String, i64> = HashMap::new();
    for r in &rows {
        *cur_tot
            .entry(crate::currency::normalize_code(&r.currency))
            .or_default() += r.net_cents;
    }
    let currency = profile
        .primary()
        .map(str::to_string)
        .or_else(|| {
            cur_tot
                .iter()
                .max_by_key(|(_, v)| **v)
                .map(|(k, _)| k.clone())
        })
        .unwrap_or_else(|| crate::currency::SCHEMA_DEFAULT_CURRENCY.to_string());
    // Mixed means "money exists that these totals EXCLUDE", so it must consider
    // the user's accounts, not only the currencies that happen to appear in
    // this window's spending.
    let mixed_currency = profile.is_mixed() || cur_tot.len() > 1;

    // Per (merchant, month) and per-month grand totals — primary currency only.
    let mut m_month: HashMap<String, HashMap<String, (i64, i64)>> = HashMap::new(); // key -> ym -> (sum, count)
    let mut m_display: HashMap<String, String> = HashMap::new();
    let mut m_cat: HashMap<String, Option<String>> = HashMap::new();
    let mut grand: HashMap<String, i64> = HashMap::new(); // ym -> sum
    for r in rows
        .into_iter()
        .filter(|r| crate::currency::normalize_code(&r.currency) == currency)
    {
        let e = m_month.entry(r.key.clone()).or_default().entry(r.ym.clone()).or_insert((0, 0));
        e.0 += r.net_cents;
        e.1 += 1;
        m_display.entry(r.key.clone()).or_insert(r.display);
        m_cat.entry(r.key.clone()).or_insert(r.category);
        *grand.entry(r.ym).or_default() += r.net_cents;
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
    // Must match `compute`'s scoping exactly: `classify` subtracts this month
    // total from that baseline, so scoping one and not the other compares a
    // single-currency band against an all-currency total.
    let cur = crate::metrics::primary_currency_clause(conn, "t");
    let sql = format!(
        "SELECT COALESCE(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                   WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                   ELSE 0 END), 0) FROM transactions t \
         WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND {pred}{cur} \
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
    // Same scope as `month_total` — the doc comment above promises these two
    // "never diverge", which only holds if they narrow identically.
    let cur = crate::metrics::primary_currency_clause(conn, "t");
    let sql = format!(
        "SELECT COALESCE(c.label, 'Uncategorized') AS label, \
                SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                         WHEN t.amount_cents < 0 THEN -t.amount_cents \
                         ELSE 0 END) AS spent \
         FROM transactions t LEFT JOIN categories c ON c.id = t.category_id \
         WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND {pred}{cur} \
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

    /// A settle-up (person-to-person reimbursement) row on a given merchant/month.
    fn ins_settle_up(conn: &Connection, ym: &str, cents: i64, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at,settle_up) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,0,'cleared',datetime('now'),1)",
            rusqlite::params![format!("{ym}-15T12:00:00Z"), cents, merchant],
        ).unwrap();
    }

    /// Seed a minimal category group + category so `category_id` FK inserts succeed.
    fn seed_category(conn: &Connection, id: &str, label: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
            rusqlite::params![id, label],
        )
        .unwrap();
    }

    /// An ordinary (non settle-up) expense row categorized to `category_id`.
    fn ins_categorized(conn: &Connection, ym: &str, cents: i64, merchant: &str, category_id: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,?4,0,'cleared',datetime('now'))",
            rusqlite::params![format!("{ym}-15T12:00:00Z"), cents, merchant, category_id],
        ).unwrap();
    }

    /// A settle-up row categorized to `category_id`.
    fn ins_settle_up_categorized(conn: &Connection, ym: &str, cents: i64, merchant: &str, category_id: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,is_transfer,status,created_at,settle_up) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,?4,0,'cleared',datetime('now'),1)",
            rusqlite::params![format!("{ym}-15T12:00:00Z"), cents, merchant, category_id],
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

    #[test]
    fn month_category_breakdown_nets_settle_up_inflow() {
        // A $500 expense and a $200 settle-up reimbursement in the same
        // category must net to $300 spend, not drop the reimbursement or
        // double count it as a separate outflow.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_category(&conn, "food", "Food");
        ins_categorized(&conn, "2026-05", -500_00, "GROCERY STORE", "food");
        ins_settle_up_categorized(&conn, "2026-05", 200_00, "GROCERY REFUND", "food");

        let rows = month_category_breakdown(&conn, "2026-05", 10).unwrap();
        let food = rows.iter().find(|r| r.label == "Food").expect("Food category present");
        assert_eq!(food.amount_cents, 300_00, "500 expense - 200 settle-up = 300 net spend");
    }

    #[test]
    fn month_total_nets_settle_up_inflow() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        ins(&conn, "2026-05", -500_00, "GROCERY STORE");
        ins_settle_up(&conn, "2026-05", 200_00, "GROCERY REFUND");

        assert_eq!(month_total(&conn, "2026-05").unwrap(), 300_00);
    }

    #[test]
    fn compute_nets_settle_up_inflow_per_merchant_and_grand_total() {
        // Same merchant, same month: a $500 expense plus a $200 settle-up
        // inflow must net to $300 in both the per-merchant baseline and the
        // grand monthly total feeding the median/MAD band.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        ins(&conn, "2026-05", -500_00, "SAVE ON FOODS  EDMONTON, AB");
        ins_settle_up(&conn, "2026-05", 200_00, "SAVE ON FOODS  EDMONTON, AB");

        let b = compute(&conn, "2026-05", "2026-06").unwrap();
        let groceries = b.per_merchant.get(&canonical_merchant_key("SAVE ON FOODS  EDMONTON, AB")).unwrap();
        assert_eq!(groceries.monthly_cents, 300_00, "500 expense - 200 settle-up = 300 net spend");
        assert_eq!(b.grand_monthly_median_cents, 300_00);
    }

    #[test]
    fn baseline_analyses_the_same_primary_currency_the_metrics_layer_reports() {
        // Two CAD accounts and one USD, but the USD account carries the larger
        // spend. The old rule picked the currency with the most SPEND, so this
        // module would have analysed USD while every other screen reported CAD
        // — the same user seeing two different "normal" figures.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        conn.execute("UPDATE accounts SET currency = 'CAD' WHERE id = 'a'", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('cad2','me','B','Checking','Chq','CAD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('usd','me','B','Credit','US Card','USD','#fff',datetime('now'))", []).unwrap();

        ins(&conn, "2026-05", -100_00, "CAD MERCHANT");
        // -90000 cents = $900. Written as a plain SQL integer: Rust's `_` digit
        // separators are not SQL syntax.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'usd','2026-05-15T12:00:00Z',-90000,'USD MERCHANT',0,'cleared',datetime('now'))",
            [],
        )
        .unwrap();

        let b = compute(&conn, "2026-05", "2026-06").unwrap();
        assert_eq!(b.currency, "CAD", "primary comes from accounts, not spend volume");
        assert!(b.mixed_currency, "the USD holding is flagged");
        assert_eq!(
            b.grand_monthly_median_cents, 100_00,
            "USD spending is excluded, not added to the CAD total"
        );
        assert!(
            b.per_merchant.get(&canonical_merchant_key("USD MERCHANT")).is_none(),
            "a foreign-currency merchant must not appear in a CAD baseline"
        );
    }

    #[test]
    fn month_total_and_category_rows_narrow_the_same_way_the_baseline_does() {
        // `classify` subtracts the month total from the baseline band. If one
        // is currency-scoped and the other is not, a user whose foreign-card
        // spending lands in the same month gets told their spending "stepped
        // up" purely because two figures counted different accounts.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        conn.execute("UPDATE accounts SET currency = 'CAD' WHERE id = 'a'", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('cad2','me','B','Checking','Chq','CAD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('usd','me','B','Credit','US Card','USD','#fff',datetime('now'))", []).unwrap();
        seed_category(&conn, "cat-travel", "Travel");

        ins(&conn, "2026-05", -100_00, "CAD MERCHANT");
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at,category_id) \
             VALUES(hex(randomblob(16)),'usd','2026-05-15T12:00:00Z',-90000,'USD MERCHANT',0,'cleared',datetime('now'),'cat-travel')",
            [],
        )
        .unwrap();

        assert_eq!(
            month_total(&conn, "2026-05").unwrap(),
            100_00,
            "the USD charge is excluded, matching the baseline's scope"
        );
        let cats = month_category_breakdown(&conn, "2026-05", 10).unwrap();
        assert!(
            !cats.iter().any(|c| c.label == "Travel"),
            "a USD-only category must not appear in a CAD breakdown: {cats:?}"
        );
        assert_eq!(
            cats.iter().map(|c| c.amount_cents).sum::<i64>(),
            month_total(&conn, "2026-05").unwrap(),
            "category rows still reconcile to the month total"
        );
    }

    #[test]
    fn currency_casing_does_not_split_one_currency_into_two() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        conn.execute("UPDATE accounts SET currency = 'usd' WHERE id = 'a'", []).unwrap();
        ins(&conn, "2026-05", -100_00, "MERCHANT");

        let b = compute(&conn, "2026-05", "2026-06").unwrap();
        assert_eq!(b.currency, "USD");
        assert!(!b.mixed_currency, "lowercase 'usd' is not a second currency");
        assert_eq!(b.grand_monthly_median_cents, 100_00, "the row still counts");
    }
}

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
    let months = months_between(start_ym, end_ym).max(1);
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

    let per_merchant = m_month
        .into_iter()
        .map(|(key, by_month)| {
            let total: i64 = by_month.values().map(|(s, _)| *s).sum();
            let count: i64 = by_month.values().map(|(_, c)| *c).sum();
            let mb = MerchantBaseline {
                display: m_display.remove(&key).unwrap_or_else(|| key.clone()),
                category: m_cat.remove(&key).flatten(),
                monthly_cents: total / months,
                txns_per_month: count as f64 / months as f64,
                active_months: by_month.len() as i64,
            };
            (key, mb)
        })
        .collect();

    // Robust grand monthly: median over ALL baseline months, counting months
    // with no spend as 0. Build the full month vector from the span, not just
    // months present.
    let mut monthly_totals: Vec<f64> = Vec::with_capacity(months as usize);
    for i in 0..months {
        let idx = sy * 12 + (sm as i32 - 1) + i as i32;
        let ym = format!("{:04}-{:02}", idx.div_euclid(12), idx.rem_euclid(12) + 1);
        monthly_totals.push(*grand.get(&ym).unwrap_or(&0) as f64);
    }
    let grand_monthly_median_cents = stats::median(&monthly_totals).round() as i64;

    Ok(Baseline {
        months,
        grand_monthly_median_cents,
        per_merchant,
        currency,
        mixed_currency,
    })
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
}

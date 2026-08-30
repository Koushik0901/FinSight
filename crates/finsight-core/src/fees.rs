//! Fee detection — bank/penalty/ATM/surcharge fees grouped and thresholded.
//!
//! Detects fee-like transactions in a calendar month and surfaces an alert when
//! the cost is material: **≥2 fees or >$25** in the month. This mirrors the
//! subscription/bill detection path: same canonical merchant key for grouping
//! stability and same primary-currency scoping so a mixed-currency ledger never
//! sums CAD + USD into a meaningless total.

use crate::error::CoreResult;
use crate::merchant::{canonical_merchant_key, fee_vendor_hint, normalize_merchant};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::collections::HashMap;

/// Thresholds for the "fees this month" alert.
pub const FEE_COUNT_THRESHOLD: i64 = 2;
pub const FEE_TOTAL_THRESHOLD_CENTS: i64 = 2500; // $25

/// One canonical fee vendor within the month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeDetail {
    pub merchant_key: String,
    pub display: String,
    pub count: i64,
    pub total_cents: i64,
}

/// Summary of fees in a calendar month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeMonthSummary {
    pub month: String,
    pub currency: String,
    pub count: i64,
    pub total_cents: i64,
    pub details: Vec<FeeDetail>,
    pub mixed_currency: bool,
}

impl FeeMonthSummary {
    pub fn should_alert(&self) -> bool {
        self.count >= FEE_COUNT_THRESHOLD || self.total_cents > FEE_TOTAL_THRESHOLD_CENTS
    }
}

fn month_bounds(ym: &str) -> (String, String) {
    let (y, m): (i32, u32) = crate::spending::parse_ym(ym);
    let start = format!("{y:04}-{m:02}-01");
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end = format!("{ny:04}-{nm:02}-01");
    (start, end)
}

fn primary_currency(conn: &Connection) -> (String, bool) {
    let profile = crate::currency::currency_profile(conn).unwrap_or_default();
    let mixed = profile.is_mixed();
    let code = profile.primary().unwrap_or("").to_string();
    (code, mixed)
}

pub fn fees_for_month(conn: &Connection, month_ym: &str) -> CoreResult<FeeMonthSummary> {
    let (month_start, month_end) = month_bounds(month_ym);
    let (currency, mixed) = primary_currency(conn);
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let cur_clause = crate::metrics::primary_currency_clause(conn, "t");
    let sql = format!(
        "SELECT t.merchant_raw, t.amount_cents FROM transactions t WHERE t.posted_at >= ?1 AND t.posted_at < ?2 AND t.is_transfer = 0 AND t.amount_cents < 0 AND {pred}{cur_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![month_start, month_end], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut grouped: HashMap<String, (String, i64, i64)> = HashMap::new();
    let mut total_cents: i64 = 0;
    let mut count: i64 = 0;
    for row in rows.flatten() {
        let (raw, amt) = row;
        let normalized = normalize_merchant(&raw);
        if fee_vendor_hint(&normalized).is_none() {
            continue;
        }
        let key = canonical_merchant_key(&raw);
        let display = crate::merchant::split_display(&raw);
        let amt_abs = amt.abs();
        total_cents += amt_abs;
        count += 1;
        let entry = grouped
            .entry(key.clone())
            .or_insert_with(|| (display.clone(), 0, 0));
        entry.1 += 1;
        entry.2 += amt_abs;
    }
    let mut details: Vec<FeeDetail> = grouped
        .into_iter()
        .map(|(k, (display, c, total))| FeeDetail {
            merchant_key: k,
            display,
            count: c,
            total_cents: total,
        })
        .collect();
    details.sort_by(|a, b| b.total_cents.cmp(&a.total_cents));
    Ok(FeeMonthSummary {
        month: month_ym.to_string(),
        currency,
        count,
        total_cents,
        details,
        mixed_currency: mixed,
    })
}

pub fn fees_this_month(
    conn: &Connection,
    now: DateTime<Utc>,
) -> CoreResult<Option<FeeMonthSummary>> {
    let ym = now.format("%Y-%m").to_string();
    let s = fees_for_month(conn, &ym)?;
    if s.should_alert() {
        Ok(Some(s))
    } else {
        Ok(None)
    }
}

pub fn has_fee_alert(conn: &Connection, now: DateTime<Utc>) -> CoreResult<bool> {
    let ym = now.format("%Y-%m").to_string();
    Ok(fees_for_month(conn, &ym)?.should_alert())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;
    use chrono::Utc;
    use tempfile::TempDir;
    fn fresh() -> (TempDir, Db) {
        let (d, db) = crate::testing::migrated_db();
        (d, db)
    }
    fn ensure_account(conn: &Connection) -> String {
        if let Some(id) = conn
            .query_row("SELECT id FROM accounts LIMIT 1", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
        {
            return id;
        }
        let id = "a1".to_string();
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) VALUES(?1,'Me','Bank','Checking','Checking','USD','#fff','manual',datetime('now'))", rusqlite::params![id]).unwrap();
        id
    }
    fn insert_fee(conn: &Connection, merchant: &str, cents: i64, posted_at: &str) {
        let account_id = ensure_account(conn);
        let txn_id = format!(
            "fee-{}-{}-{}-{}",
            merchant.replace(' ', "_"),
            posted_at.replace(':', "-").replace('T', "-"),
            cents.abs(),
            rand::random::<u16>()
        );
        conn.execute("INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, is_transfer, status, created_at) VALUES(?1,?2,?3,?4,?5,0,'cleared',datetime('now'))", rusqlite::params![txn_id, account_id, posted_at, cents, merchant]).unwrap();
    }
    #[test]
    fn fee_vendor_hint_word_boundary() {
        assert!(fee_vendor_hint(&normalize_merchant("STARBUCKS COFFEE")).is_none());
        assert!(fee_vendor_hint(&normalize_merchant("OVERDRAFT FEE")).is_some());
        assert!(fee_vendor_hint(&normalize_merchant("NSF FEE")).is_some());
        assert!(fee_vendor_hint(&normalize_merchant("ATM FEE")).is_some());
        assert!(fee_vendor_hint(&normalize_merchant("SURCHARGE FEE")).is_some());
        assert!(fee_vendor_hint(&normalize_merchant("LATE FEE")).is_some());
        assert!(fee_vendor_hint(&normalize_merchant("ANNUAL FEE")).is_some());
        assert!(fee_vendor_hint(&normalize_merchant("ANNUAL MEMBERSHIP")).is_some());
    }
    #[test]
    fn fees_threshold_count() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let now = Utc::now();
        let ym = now.format("%Y-%m").to_string();
        let d1 = format!("{}-05T00:00:00Z", ym);
        let d2 = format!("{}-10T00:00:00Z", ym);
        insert_fee(&conn, "OVERDRAFT FEE", -1200, &d1);
        insert_fee(&conn, "NSF FEE", -500, &d2);
        let s = fees_for_month(&conn, &ym).unwrap();
        assert_eq!(s.count, 2);
        assert!(s.should_alert());
        assert!(fees_this_month(&conn, now).unwrap().is_some());
    }
    #[test]
    fn fees_threshold_amount() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let now = Utc::now();
        let ym = now.format("%Y-%m").to_string();
        let d1 = format!("{}-05T00:00:00Z", ym);
        insert_fee(&conn, "OVERDRAFT FEE", -3000, &d1);
        let s = fees_for_month(&conn, &ym).unwrap();
        assert_eq!(s.count, 1);
        assert_eq!(s.total_cents, 3000);
        assert!(s.should_alert());
    }
    #[test]
    fn fees_below_threshold_no_alert() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let now = Utc::now();
        let ym = now.format("%Y-%m").to_string();
        let d1 = format!("{}-05T00:00:00Z", ym);
        insert_fee(&conn, "ATM FEE", -200, &d1);
        let s = fees_for_month(&conn, &ym).unwrap();
        assert_eq!(s.count, 1);
        assert!(!s.should_alert());
        assert!(fees_this_month(&conn, now).unwrap().is_none());
    }
    #[test]
    fn fees_currency_isolated() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        ensure_account(&conn);
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) VALUES('cad-acct','You','Bank','Checking','CAD Chequing','CAD','#fff','manual','2026-01-01T00:00:00Z')",[]).unwrap();
        conn.execute("INSERT INTO account_balances(account_id, balance_cents, as_of_date, source) VALUES('a1', 100000, '2026-08-01', 'manual')",[]).unwrap();
        let now = Utc::now();
        let ym = now.format("%Y-%m").to_string();
        let d1 = format!("{}-05T00:00:00Z", ym);
        conn.execute("INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, is_transfer, status, created_at) VALUES('cad-fee','cad-acct',?1,-5000,'OVERDRAFT FEE CAD',0,'cleared',datetime('now'))", rusqlite::params![d1]).unwrap();
        insert_fee(&conn, "ATM FEE", -200, &d1);
        let s = fees_for_month(&conn, &ym).unwrap();
        assert_eq!(s.count, 1);
        assert_eq!(s.total_cents, 200);
        assert!(!s.should_alert());
    }
    #[test]
    fn fees_canonical_grouping() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let now = Utc::now();
        let ym = now.format("%Y-%m").to_string();
        let d1 = format!("{}-05T00:00:00Z", ym);
        let d2 = format!("{}-06T00:00:00Z", ym);
        insert_fee(&conn, "ATM FEE  123 VANCOUVER", -300, &d1);
        insert_fee(&conn, "ATM FEE  456 TORONTO", -400, &d2);
        let s = fees_for_month(&conn, &ym).unwrap();
        assert_eq!(s.count, 2);
        assert_eq!(s.details.len(), 1);
        assert_eq!(s.details[0].count, 2);
    }
}

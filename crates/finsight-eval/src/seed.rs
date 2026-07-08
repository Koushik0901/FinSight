//! Deterministic synthetic household for Copilot evaluation.
//!
//! Fixed, round amounts so the benchmark's reference facts stay stable and
//! hand-verifiable. Monthly history is anchored to the six most-recent COMPLETE
//! months relative to the clock (so "last month" / "the past six months" always
//! have data) while the amounts, categories, accounts, rates, goals, and the
//! seeded anomaly never change. Benchmark questions therefore avoid depending on
//! the partial current month.
//!
//! Ground-truth summary (see `docs/eval-household.md` / the benchmark's
//! `reference_facts`):
//! - Checking +$2,000, Emergency Fund savings +$5,000 (4.0% APY),
//!   Visa −$1,200 (19.9% APR, $30 min), Auto Loan −$8,000 (6.5% APR, $250 min),
//!   Brokerage = UNKNOWN balance (no snapshot).
//! - Known net worth = $7,000 assets − $9,200 debt = −$2,200 (brokerage excluded).
//! - Income $4,000/mo payroll. Monthly expenses total $1,837 → surplus $2,163.
//! - Biggest category = Housing ($1,200/mo rent); then Groceries ($400/mo).
//! - Emergency fund $5,000 ≈ 2.7 months of expenses → below the 3–6mo target.
//! - Two debts: Visa (19.9% APR, smaller balance) and Auto Loan (6.5%).
//!   Visa is first under BOTH avalanche (highest APR) and snowball (smallest).
//! - Recurring: Rent, Netflix ($16), Spotify ($11), Gym ($40).
//! - One flagged anomaly: a $2,500 Apple Store charge (also uncategorized).

use chrono::{Datelike, NaiveDate, Utc};
use rusqlite::{params, Connection};

/// First day of the month that is `back` months before `anchor`'s month.
fn first_of_month_back(anchor: NaiveDate, back: u32) -> NaiveDate {
    let total = anchor.year() * 12 + (anchor.month0() as i32) - back as i32;
    let y = total.div_euclid(12);
    let m0 = total.rem_euclid(12);
    NaiveDate::from_ymd_opt(y, (m0 + 1) as u32, 1).unwrap()
}

/// A day within a given month, clamped to the last valid day (so day 31 in Feb
/// lands on the 28th/29th rather than panicking).
fn day_in(month_start: NaiveDate, day: u32) -> NaiveDate {
    let mut d = day;
    loop {
        if let Some(date) = NaiveDate::from_ymd_opt(month_start.year(), month_start.month(), d) {
            return date;
        }
        d -= 1;
    }
}

fn insert_txn(conn: &Connection, acct: &str, date: NaiveDate, cents: i64, merchant: &str, cat: Option<&str>) {
    conn.execute(
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, created_at) \
         VALUES(hex(randomblob(16)), ?1, ?2, ?3, ?4, ?5, 'cleared', datetime('now'))",
        params![acct, format!("{date}T12:00:00Z"), cents, merchant, cat],
    )
    .unwrap();
}

/// Seed the full evaluation household into a fresh, migrated DB connection.
pub fn seed(conn: &mut Connection) {
    // ── Accounts + confirmed balances (brokerage deliberately has none) ──────
    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apy_pct,created_at) VALUES('chk','You','Bank','Checking','Everyday Checking','USD','#3B82F6','manual','liquid',1,'cash',NULL,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('chk',date('now'),200000,'manual')", []).unwrap();

    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apy_pct,created_at) VALUES('sav','You','Bank','Savings','Emergency Fund','USD','#10B981','manual','liquid',1,'cash',4.0,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('sav',date('now'),500000,'manual')", []).unwrap();

    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,limit_cents,created_at) VALUES('cc','You','Bank','Credit','Visa Rewards','USD','#F97316','manual','restricted',0,'debt',19.9,3000,500000,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('cc',date('now'),-120000,'manual')", []).unwrap();

    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,created_at) VALUES('loan','You','Bank','Credit','Auto Loan','USD','#EF4444','manual','restricted',0,'debt',6.5,25000,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('loan',date('now'),-800000,'manual')", []).unwrap();

    // Brokerage: has activity but NO confirmed balance snapshot, so the app
    // reports it as UNKNOWN (not $0) and excludes it from net worth. The
    // "unknown" flag requires a transaction to exist WITHOUT a non-seed balance
    // row (see accounts::list_summaries), hence the dividend below.
    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,created_at) VALUES('inv','You','Broker','Investment','Brokerage','USD','#8B5CF6','manual','invested',0,'investments',datetime('now'))", []).unwrap();

    // ── Categories ───────────────────────────────────────────────────────────
    conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('core','Core',0)", []).unwrap();
    for (id, label) in [
        ("groceries", "Groceries"),
        ("dining", "Dining"),
        ("transport", "Transport"),
        ("shopping", "Shopping"),
        ("entertainment", "Entertainment"),
        ("utilities", "Utilities"),
        ("housing", "Housing"),
        ("health", "Health"),
    ] {
        conn.execute(
            "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES(?1,'core',?2,'#888',0)",
            params![id, label],
        )
        .unwrap();
    }

    // ── Twelve most-recent COMPLETE months of income + recurring + variable ──
    // A full 12 months so the tools' trailing-12-month income/expense AVERAGES
    // equal the intended $4,000 / $1,837 per-month figures. With only 6 months
    // seeded, those averages divide by 12 and halve to $2,000 / $163 surplus,
    // which is a real Copilot behavior (fixed 12-month divisor understates a
    // <12-month history) but made the benchmark facts unreachable.
    let anchor = Utc::now().date_naive();
    let most_recent_complete = first_of_month_back(anchor, 1); // last month's 1st
    for back in 0..12u32 {
        let m = first_of_month_back(most_recent_complete, back);

        // Income: $4,000 payroll on the 1st (NULL category — an inflow).
        insert_txn(conn, "chk", day_in(m, 1), 400000, "Acme Payroll", None);

        // Recurring commitments (stable amount + cadence → detectable).
        insert_txn(conn, "chk", day_in(m, 2), -120000, "Skyline Apartments", Some("housing"));
        insert_txn(conn, "chk", day_in(m, 5), -1600, "Netflix", Some("entertainment"));
        insert_txn(conn, "chk", day_in(m, 8), -1100, "Spotify", Some("entertainment"));
        insert_txn(conn, "chk", day_in(m, 6), -4000, "Anytime Fitness", Some("health"));

        // Variable but steady monthly spend.
        insert_txn(conn, "chk", day_in(m, 12), -25000, "Costco", Some("groceries"));
        insert_txn(conn, "chk", day_in(m, 22), -15000, "Trader Joe's", Some("groceries"));
        insert_txn(conn, "chk", day_in(m, 15), -5000, "Chipotle", Some("dining"));
        insert_txn(conn, "chk", day_in(m, 18), -12000, "Shell", Some("transport"));
    }

    // ── Recent one-off / uncategorized expenses (in the most-recent month) ───
    let last = most_recent_complete;
    insert_txn(conn, "cc", day_in(last, 10), -30000, "Best Buy", None); // uncategorized, over $60
    insert_txn(conn, "cc", day_in(last, 14), -45000, "Delta Airlines", None); // uncategorized, over $60
    insert_txn(conn, "chk", day_in(last, 24), -1800, "SQ *Blue Bottle", None); // uncategorized, under $60
    // Large, unusual, uncategorized charge — also the seeded anomaly.
    insert_txn(conn, "cc", day_in(last, 20), -250000, "Apple Store", None);
    conn.execute(
        "UPDATE transactions SET is_anomaly = 1, ai_explanation = 'Much larger than this account''s typical charge' WHERE merchant_raw = 'Apple Store'",
        [],
    )
    .unwrap();

    // Brokerage activity, before the 12-month analysis window. Positive (a
    // dividend inflow, like payroll) so it is NOT an uncategorized expense and
    // doesn't disturb the spending / uncategorized-count facts. Its only job is
    // to make the brokerage's balance genuinely "unknown".
    let old = first_of_month_back(most_recent_complete, 13);
    insert_txn(conn, "inv", day_in(old, 15), 10000, "Vanguard Dividend", None);

    // ── Goals ────────────────────────────────────────────────────────────────
    conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('ef','Emergency Fund','save',1100000,500000,50000,'#10B981',0,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('vac','Vacation','save-by-date',300000,60000,10000,'#3B82F6',1,datetime('now'))", []).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};

    fn seeded() -> (tempfile::TempDir, Db) {
        let dir = tempfile::TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("seed.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn);
        }
        (dir, db)
    }

    /// Locks the benchmark's ground-truth reference facts to the actual seed so
    /// they can't silently drift apart.
    #[test]
    fn seed_matches_documented_reference_facts() {
        let (_d, db) = seeded();
        let conn = db.get().unwrap();

        // Use the app's real net-worth computation so the reference facts match
        // exactly what the Copilot's tools return (not a hand-rolled SQL notion).
        let mut wconn = db.get().unwrap();
        let bd = finsight_core::repos::net_worth::breakdown(&mut wconn).unwrap();
        assert_eq!(bd.net_worth_cents, -220_000, "known net worth = -$2,200");
        assert_eq!(
            bd.accounts_with_unknown_balance, 1,
            "brokerage must read as UNKNOWN (has a txn, no confirmed balance)"
        );
        assert!(
            bd.unknown_balance_accounts.iter().any(|n| n == "Brokerage"),
            "the unknown account is the Brokerage"
        );

        let accounts: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).unwrap();
        assert_eq!(accounts, 5);

        let income: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(amount_cents),0) FROM transactions WHERE merchant_raw = 'Acme Payroll'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(income, 4_800_000, "12 × $4,000 payroll (the brokerage dividend is separate)");

        let housing: i64 = conn
            .query_row("SELECT COALESCE(SUM(-amount_cents),0) FROM transactions WHERE category_id='housing'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(housing, 1_440_000, "12 × $1,200 rent → biggest category");

        let groceries: i64 = conn
            .query_row("SELECT COALESCE(SUM(-amount_cents),0) FROM transactions WHERE category_id='groceries'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(groceries, 480_000, "12 × $400 → second category");

        let uncategorized: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND amount_cents < 0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uncategorized, 4, "Best Buy, Delta, Blue Bottle, Apple Store");

        let anomalies: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions WHERE is_anomaly = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(anomalies, 1, "the $2,500 Apple Store charge");

        let debts: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts WHERE account_group='debt'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(debts, 2, "Visa + Auto Loan");
    }
}

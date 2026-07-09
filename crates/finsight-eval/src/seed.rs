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
    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apy_pct,created_at) VALUES('chk','You','Bank','Checking','Everyday Checking','USD','#3B82F6','manual','liquid',0,'cash',NULL,datetime('now'))", []).unwrap();
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

    // ── 10 YEARS (120 months) of income + recurring + variable, ending at the
    // current state. Amounts EVOLVE over the decade (career raises, rent creep,
    // subscriptions that start at different times, delivery only in recent
    // years), but the most recent ~13 months exactly reproduce the current-state
    // facts (income $4,000, rent $1,200, expenses ~$1,900) so the tools' trailing
    // 90-day / 12-month windows still yield the documented "now" numbers. The
    // deep history is only visible to search_transactions (any date range) and
    // get_spending_breakdown (up to 60 months), which is what makes multi-year
    // trend / "which year" questions answerable without breaking current facts.
    let anchor = Utc::now().date_naive();
    let newest = first_of_month_back(anchor, 0); // current month's 1st
    for months_ago in 0..120u32 {
        let m = first_of_month_back(newest, months_ago);
        // Income tier: ~$2,400/mo a decade ago rising to $4,000/mo now (raises).
        let income = match months_ago {
            0..=17 => 400000,
            18..=41 => 370000,
            42..=65 => 330000,
            66..=89 => 290000,
            _ => 240000,
        };
        // Rent creep: $800 → $1,200 over the decade.
        let rent = match months_ago {
            0..=17 => 120000,
            18..=41 => 110000,
            42..=83 => 95000,
            _ => 80000,
        };
        insert_txn(conn, "chk", day_in(m, 1), income, "Acme Payroll", None);
        insert_txn(conn, "chk", day_in(m, 2), -rent, "Skyline Apartments", Some("housing"));

        // Subscriptions started at different points in the timeline.
        if months_ago <= 84 {
            insert_txn(conn, "chk", day_in(m, 5), -1600, "Netflix", Some("entertainment"));
        }
        if months_ago <= 60 {
            insert_txn(conn, "chk", day_in(m, 8), -1100, "Spotify", Some("entertainment"));
        }
        if months_ago <= 36 {
            insert_txn(conn, "chk", day_in(m, 6), -4000, "Anytime Fitness", Some("health"));
        }

        // Groceries + transport scale gently with income over the years.
        let (costco, tj, shell) = if months_ago <= 41 {
            (-25000, -15000, -12000)
        } else if months_ago <= 89 {
            (-20000, -12000, -10000)
        } else {
            (-16000, -9000, -8000)
        };
        insert_txn(conn, "chk", day_in(m, 12), costco, "Costco", Some("groceries"));
        insert_txn(conn, "chk", day_in(m, 22), tj, "Trader Joe's", Some("groceries"));
        insert_txn(conn, "chk", day_in(m, 15), -5000, "Chipotle", Some("dining"));
        insert_txn(conn, "chk", day_in(m, 18), shell, "Shell", Some("transport"));
        // Food delivery only became a habit in the last ~2 years.
        if months_ago <= 24 {
            insert_txn(conn, "chk", day_in(m, 19), -6000, "DoorDash", Some("dining"));
        }
    }

    // ── One-off / uncategorized expenses, placed ~5 months ago so they do NOT
    // inflate the 90-day expense average (which would crush the computed
    // surplus) — but stay inside the year for anomaly / largest-purchase /
    // 6-month-search questions. Still 4 uncategorized; the $2,500 Apple Store
    // charge is the one flagged anomaly.
    let midpast = first_of_month_back(newest, 5);
    insert_txn(conn, "cc", day_in(midpast, 10), -30000, "Best Buy", None);
    insert_txn(conn, "cc", day_in(midpast, 14), -45000, "Delta Airlines", None);
    insert_txn(conn, "chk", day_in(midpast, 24), -1800, "SQ *Blue Bottle", None);
    insert_txn(conn, "cc", day_in(midpast, 20), -250000, "Apple Store", None);
    conn.execute(
        "UPDATE transactions SET is_anomaly = 1, ai_explanation = 'Much larger than this account''s typical charge' WHERE merchant_raw = 'Apple Store'",
        [],
    )
    .unwrap();

    // Brokerage activity, before the 12-month analysis window. Positive (a
    // dividend inflow, like payroll) so it is NOT an uncategorized expense and
    // doesn't disturb the spending / uncategorized-count facts. Its only job is
    // to make the brokerage's balance genuinely "unknown".
    let old = first_of_month_back(newest, 13);
    insert_txn(conn, "inv", day_in(old, 15), 10000, "Vanguard Dividend", None);

    // ── Goals ────────────────────────────────────────────────────────────────
    conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('ef','Emergency Fund','save',1100000,500000,50000,'#10B981',0,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('vac','Vacation','save-by-date',300000,60000,10000,'#3B82F6',1,datetime('now'))", []).unwrap();

    // ── Current-month budget envelopes. Groceries ($400 actual vs $350) and
    // Dining ($110 actual vs $80) are consistently OVER; Transport and
    // Entertainment are under. Supports "where am I overspending vs my targets".
    // (get_budgets surfaces current-month budgeted-vs-actual.)
    let month = anchor.format("%Y-%m").to_string();
    for (cat, amt) in [
        ("groceries", 35000),
        ("dining", 8000),
        ("transport", 15000),
        ("entertainment", 4000),
        ("housing", 120000),
    ] {
        conn.execute(
            "INSERT INTO budgets(id,category_id,month,amount_cents,created_at,updated_at) \
             VALUES(hex(randomblob(8)),?1,?2,?3,datetime('now'),datetime('now'))",
            params![cat, month, amt],
        )
        .unwrap();
    }

    // ── A large annual obligation coming up (~4 months out): $1,200 insurance
    // premium. Surfaces via the snapshot's planned transactions / upcoming
    // obligations so liquidity-planning questions can account for it.
    conn.execute(
        "INSERT INTO planned_transactions(id,description,amount_cents,account_id,due_date,status,source,created_at) \
         VALUES('ins','Annual Insurance Premium',-120000,'chk',date('now','+120 days'),'planned','manual',datetime('now'))",
        [],
    )
    .unwrap();
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

    /// Diagnostic (ignored): prints the income/expense/surplus the tools compute
    /// from the seed, so the benchmark's surplus facts can be reconciled with
    /// what run_purchase_affordability / run_emergency_fund_scenarios actually use.
    #[test]
    #[ignore]
    fn diag_snapshot_income_and_surplus() {
        let (_d, db) = seeded();
        let mut conn = db.get().unwrap();
        let s = finsight_agent::finance::build_snapshot(&mut conn).unwrap();
        eprintln!("income_90d/mo   = {}", s.avg_monthly_income_90d_cents);
        eprintln!("income_12m/mo   = {}", s.avg_monthly_income_12m_cents);
        eprintln!("expense_90d/mo  = {}", s.avg_monthly_expense_90d_cents);
        eprintln!("expense_12m/mo  = {}", s.avg_monthly_expense_12m_cents);
        eprintln!("surplus(90d)    = {}", s.avg_monthly_income_90d_cents - s.avg_monthly_expense_90d_cents);
        eprintln!("liquid          = {}", s.liquid_balance_cents);
        eprintln!("ef_months       = {}", s.emergency_fund_months);
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

        // Deep history: ~10 years of monthly transactions.
        let tx_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert!(tx_count > 900, "10 years of monthly data → 900+ transactions, got {tx_count}");
        let oldest_days: i64 = conn
            .query_row("SELECT CAST(julianday('now') - julianday(MIN(posted_at)) AS INTEGER) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert!(oldest_days > 3200, "oldest transaction ~10 years old, got {oldest_days} days");

        // Current-state facts live in the trailing 12-month window: payroll at
        // the current $4,000/mo tier, rent at the $1,200 tier.
        let income_12m: i64 = conn
            .query_row("SELECT COALESCE(SUM(amount_cents),0) FROM transactions WHERE merchant_raw='Acme Payroll' AND posted_at >= date('now','-365 days')", [], |r| r.get(0))
            .unwrap();
        assert!((4_600_000..=5_000_000).contains(&income_12m), "recent-year income ≈ 12×$4,000, got {income_12m}");
        let housing_12m: i64 = conn
            .query_row("SELECT COALESCE(SUM(-amount_cents),0) FROM transactions WHERE category_id='housing' AND posted_at >= date('now','-365 days')", [], |r| r.get(0))
            .unwrap();
        assert!((1_300_000..=1_560_000).contains(&housing_12m), "recent-year rent ≈ 12×$1,200, got {housing_12m}");

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

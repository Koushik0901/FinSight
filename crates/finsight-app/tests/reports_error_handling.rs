//! Regression coverage for the silent-error-swallowing bug found in
//! get_report_data's fetch_monthly closure: `.filter_map(|r| r.ok())` /
//! `rows.flatten().collect()` silently dropped row-level DB errors (e.g. from
//! a corrupted page) and rendered a fabricated $0 instead of surfacing a
//! failure. We can't wire tauri::State in tests (see onboarding_cmd.rs), so
//! this replicates the exact SQL + row-collection pattern reports.rs now
//! uses and proves it distinguishes "no data" from "failed to load data".

use finsight_core::{db::run_migrations, keychain, Db};
use rusqlite::params;
use tempfile::TempDir;
use uuid::Uuid;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("reports_err.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

/// Mirrors fetch_monthly's row-collection step in crates/finsight-app/src/commands/reports.rs.
fn fetch_monthly_sums(
    conn: &rusqlite::Connection,
) -> Result<Vec<(String, i64, i64)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', posted_at) AS mo,
                SUM(CASE WHEN amount_cents > 0 THEN amount_cents  ELSE 0 END),
                SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END)
         FROM transactions
         GROUP BY mo
         ORDER BY mo",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>();
    rows
}

#[test]
fn empty_transactions_table_is_a_clean_no_data_result_not_an_error() {
    let (_d, db) = fresh_db();
    let conn = db.get().unwrap();

    let rows = fetch_monthly_sums(&conn).expect("empty table must not be treated as a failure");
    assert!(
        rows.is_empty(),
        "no transactions means no monthly rows, not an error"
    );
}

/// Mirrors get_month_totals' income/expense sums, which now exclude transfers.
fn month_totals(conn: &rusqlite::Connection) -> (i64, i64) {
    conn.query_row(
        "SELECT \
           COALESCE(SUM(CASE WHEN amount_cents > 0 AND is_transfer = 0 THEN amount_cents  ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN amount_cents < 0 AND is_transfer = 0 THEN -amount_cents ELSE 0 END), 0) \
         FROM transactions",
        [],
        |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
    )
    .unwrap()
}

/// Mirrors the top_categories query, which LEFT JOINs categories and must
/// tolerate uncategorized spending (NULL category id) instead of failing the
/// whole report.
fn top_categories(conn: &rusqlite::Connection) -> Result<Vec<(String, i64)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT c.id, SUM(-t.amount_cents) AS total \
         FROM transactions t \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 \
         GROUP BY c.id ORDER BY total DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                r.get::<_, i64>(1)?,
            ))
        })?
        .collect();
    rows
}

#[test]
fn top_categories_tolerates_uncategorized_spending_null_id() {
    let (_d, db) = fresh_db();
    let conn = db.get().unwrap();
    conn.execute(
        "INSERT INTO category_groups(id,label,sort_order) VALUES('daily','Daily',0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('dining','daily','Dining','#fff',0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
         VALUES('a1','Me','Bank','Credit','Card','USD','#000','manual','2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    // One categorized and one UNcategorized negative (spending) transaction —
    // the latter groups on a NULL category id.
    conn.execute(
        "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_transfer,created_at) \
         VALUES('t1','a1','2024-01-01T00:00:00Z',-900,'Tim Hortons','dining','cleared',0,'2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_transfer,created_at) \
         VALUES('t2','a1','2024-01-01T00:00:00Z',-500,'Anomaly',NULL,'cleared',0,'2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    let rows = top_categories(&conn).expect("uncategorized NULL id must not fail the report");
    assert_eq!(
        rows.len(),
        2,
        "one categorized group + one uncategorized group"
    );
}

#[test]
fn transfers_are_excluded_from_income_and_expense_totals() {
    let (_d, db) = fresh_db();
    let conn = db.get().unwrap();
    let acct = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at, source) \
         VALUES(?1, 'me', 'Bank', 'Credit', 'Card', 'USD', '#000', ?2, 'manual')",
        params![acct, chrono::Utc::now().to_rfc3339()],
    )
    .unwrap();

    let add = |cents: i64, merchant: &str, is_transfer: i64| {
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_transfer, created_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, 'cleared', ?6, ?3)",
            params![Uuid::new_v4().to_string(), acct, chrono::Utc::now().to_rfc3339(), cents, merchant, is_transfer],
        )
        .unwrap();
    };
    // Real spending and income.
    add(-5000, "Groceries", 0);
    add(3000, "Interest", 0);
    // A large credit-card payment (transfer) that must NOT count as either.
    add(300_000, "PAYMENT RECEIVED - THANK YOU", 1);
    add(-300_000, "Internet Withdrawal to Tangerine", 1);

    let (income, expense) = month_totals(&conn);
    assert_eq!(income, 3000, "transfer inflow excluded from income");
    assert_eq!(expense, 5000, "transfer outflow excluded from expense");
}

#[test]
fn row_level_conversion_failure_surfaces_as_a_real_error_not_a_fabricated_zero() {
    let (_d, db) = fresh_db();
    let conn = db.get().unwrap();

    let acct_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at, source) \
         VALUES(?1, 'me', 'Bank', 'Checking', 'Checking', 'USD', '#000', ?2, 'manual')",
        params![acct_id, chrono::Utc::now().to_rfc3339()],
    )
    .unwrap();

    // A normal transaction: this row must be readable fine on its own.
    conn.execute(
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, created_at) \
         VALUES(?1, ?2, ?3, -500, 'Coffee', 'cleared', ?3)",
        params![Uuid::new_v4().to_string(), acct_id, chrono::Utc::now().to_rfc3339()],
    )
    .unwrap();

    // Simulate a corrupted/unreadable row: SQLite's dynamic typing lets a
    // REAL far outside i64 range land in an INTEGER-declared column via raw
    // SQL. Aggregated through SUM(), this deterministically fails the
    // `r.get::<_, i64>(1)` conversion for the month it falls in — the same
    // failure mode a corrupted page produces in production.
    conn.execute(
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, created_at) \
         VALUES(?1, ?2, ?3, 1e300, 'Overflow', 'cleared', ?3)",
        params![Uuid::new_v4().to_string(), acct_id, chrono::Utc::now().to_rfc3339()],
    )
    .unwrap();

    let result = fetch_monthly_sums(&conn);
    assert!(
        result.is_err(),
        "a row that fails to convert must surface as Err, not be silently dropped into a $0 total"
    );
}

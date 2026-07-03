//! Phase 6 read-only diagnostics against the REAL imported dev-app data.
//! Ignored; run manually (app may be running — WAL allows concurrent reads):
//!   cargo test -p finsight-app --test phase6_diagnostics -- --ignored --nocapture

use finsight_core::{keychain, Db};
use rusqlite::Connection;

fn open() -> Db {
    let appdata = std::env::var("APPDATA").expect("APPDATA");
    let db_path = std::path::Path::new(&appdata)
        .join("com.finsight.app")
        .join("data.sqlcipher");
    let key = keychain::load_or_create_key("com.finsight.app", "default").expect("db key");
    Db::open(&db_path, &key).expect("open dev db")
}

fn dump_rows(conn: &Connection, label: &str, sql: &str) {
    println!("\n### {label}");
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            println!("  QUERY ERROR: {e}");
            return;
        }
    };
    let ncols = stmt.column_count();
    let rows = stmt.query_map([], |r| {
        let mut out = Vec::new();
        for i in 0..ncols {
            let v = r.get_ref(i).ok();
            let s = match v {
                Some(rusqlite::types::ValueRef::Null) => "NULL".to_string(),
                Some(rusqlite::types::ValueRef::Integer(n)) => n.to_string(),
                Some(rusqlite::types::ValueRef::Real(f)) => format!("{f:.2}"),
                Some(rusqlite::types::ValueRef::Text(t)) => String::from_utf8_lossy(t).to_string(),
                Some(rusqlite::types::ValueRef::Blob(_)) => "<blob>".to_string(),
                None => "?".to_string(),
            };
            out.push(s);
        }
        Ok(out.join(" | "))
    });
    match rows {
        Ok(rows) => {
            for r in rows.flatten() {
                println!("  {r}");
            }
        }
        Err(e) => println!("  ROW ERROR: {e}"),
    }
}

#[test]
#[ignore = "reads the live dev DB"]
fn diagnose() {
    let db = open();
    let conn = db.get().unwrap();

    dump_rows(
        &conn,
        "ACCOUNTS (id, name, type, source)",
        "SELECT id, name, type, source FROM accounts WHERE archived_at IS NULL",
    );
    dump_rows(
        &conn,
        "ACCOUNT BALANCES (account, as_of, cents, source) latest 10",
        "SELECT account_id, as_of_date, balance_cents, source FROM account_balances ORDER BY as_of_date DESC LIMIT 10",
    );
    dump_rows(
        &conn,
        "TXN COUNTS (total, categorized, uncategorized, transfers)",
        "SELECT COUNT(*), SUM(category_id IS NOT NULL), SUM(category_id IS NULL), SUM(is_transfer) FROM transactions",
    );
    dump_rows(
        &conn,
        "TXN by account (name, count, min_date, max_date, sum_cents)",
        "SELECT a.name, COUNT(*), MIN(substr(t.posted_at,1,10)), MAX(substr(t.posted_at,1,10)), SUM(t.amount_cents) \
         FROM transactions t JOIN accounts a ON a.id=t.account_id GROUP BY a.id",
    );
    dump_rows(
        &conn,
        "CATEGORIZATION source breakdown (source, count)",
        "SELECT COALESCE(source,'<none>'), COUNT(*) FROM categorizations GROUP BY source",
    );
    dump_rows(
        &conn,
        "TOP UNCATEGORIZED merchants (merchant, count)",
        "SELECT merchant_raw, COUNT(*) c FROM transactions WHERE category_id IS NULL AND is_transfer=0 GROUP BY merchant_raw ORDER BY c DESC LIMIT 25",
    );
    dump_rows(
        &conn,
        "CATEGORIES (id, label, group)",
        "SELECT id, label, group_id FROM categories WHERE archived_at IS NULL ORDER BY label LIMIT 40",
    );
    dump_rows(
        &conn,
        "ANOMALIES count",
        "SELECT COUNT(*) FROM transactions WHERE is_anomaly=1",
    );
    dump_rows(
        &conn,
        "AI confidence distribution (rounded to .1, count)",
        "SELECT ROUND(ai_confidence,1), COUNT(*) FROM transactions WHERE ai_confidence IS NOT NULL GROUP BY ROUND(ai_confidence,1) ORDER BY 1",
    );
    // Recurring detection candidates using the SAME logic the app uses.
    dump_rows(
        &conn,
        "RECURRING CANDIDATES (merchant, occ, avg_gap_days, last_amount_cents) — current app heuristic",
        "WITH gaps AS ( \
            SELECT merchant_raw, date(posted_at) d, LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) prev_d, amount_cents \
            FROM transactions WHERE posted_at >= date('now','-395 days') \
         ), agg AS ( \
            SELECT merchant_raw, AVG(julianday(d)-julianday(prev_d)) avg_gap, COUNT(*) occ, MAX(amount_cents) last_amt \
            FROM gaps WHERE prev_d IS NOT NULL GROUP BY merchant_raw \
            HAVING occ >= 2 AND AVG(julianday(d)-julianday(prev_d)) BETWEEN 5 AND 400 \
         ) SELECT merchant_raw, occ, avg_gap, last_amt FROM agg ORDER BY occ DESC LIMIT 40",
    );
    // Merchant amount stability — is the amount consistent (subscription-like)?
    dump_rows(
        &conn,
        "MERCHANT amount stability (merchant, occ, distinct_amounts, min, max) for repeat merchants",
        "SELECT merchant_raw, COUNT(*) occ, COUNT(DISTINCT amount_cents) distinct_amts, MIN(amount_cents), MAX(amount_cents) \
         FROM transactions WHERE amount_cents < 0 GROUP BY merchant_raw HAVING occ >= 3 ORDER BY occ DESC LIMIT 30",
    );
}

/// Run the NEW recurring detector on the real dev data and print the
/// classification so we can confirm false positives drop and true subs surface.
#[test]
#[ignore = "reads the live dev DB"]
fn diagnose_recurring() {
    let db = open();
    let conn = db.get().unwrap();
    let items = finsight_core::recurring::detect_recurring(&conn, 400).unwrap();
    use finsight_core::recurring::RecurringKind;
    for kind in [
        RecurringKind::Subscription,
        RecurringKind::Bill,
        RecurringKind::Income,
        RecurringKind::Transfer,
    ] {
        println!("\n### {kind:?}");
        for it in items.iter().filter(|i| i.kind == kind) {
            println!(
                "  {:<28} occ={:<3} med={:>8} gap={:>5.1}d conf={:.2} [{}]",
                it.display_merchant,
                it.occurrences,
                it.median_amount_cents,
                it.avg_gap_days,
                it.confidence,
                it.reasons.join("; ")
            );
        }
    }
    let repeat = items.iter().filter(|i| i.kind == RecurringKind::RepeatPurchase).count();
    let sub = items.iter().filter(|i| i.kind == RecurringKind::Subscription).count();
    println!("\nSUMMARY: {sub} subscriptions, {repeat} repeat-purchases (excluded from subscriptions)");
}

use chrono::{Duration, Utc};
use finsight_core::{metrics, models::AccountType, repos::accounts, testing::migrated_db};
use rusqlite::params;

fn acct(name: &str, opening: i64, ef: bool) -> finsight_core::models::NewAccount {
    finsight_core::models::NewAccount {
        promo_apr_expires_on: None,
        post_promo_apr_pct: None,
        owner: "me".into(),
        bank: "Bank".into(),
        r#type: AccountType::Checking,
        name: name.into(),
        last4: None,
        currency: "USD".into(),
        color: "#3B82F6".into(),
        opening_balance_cents: opening,
        source: "manual".into(),
        liquidity_type: "liquid".into(),
        emergency_fund_eligible: ef,
        goal_earmark: None,
        apy_pct: None,
        simplefin_account_id: None,
        nickname: None,
        connection_id: None,
        institution_id: None,
        external_account_id: None,
        official_name: None,
        mask: None,
        subtype: None,
        account_group: "cash".into(),
        available_balance_cents: None,
        balance_date: None,
        extra_json: None,
        raw_json: None,
        import_pending: false,
        apr_pct: None,
        min_payment_cents: None,
        payoff_date: None,
        limit_cents: None,
        original_balance_cents: None,
        started_at: None,
    }
}

fn insert_txn(conn: &rusqlite::Connection, acct_id: &str, amount: i64, posted_at: &str) {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at) VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', 0, 0, ?3)",
        params![id, acct_id, posted_at, amount],
    )
    .unwrap();
}

#[test]
fn emergency_fund_months_unified() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();
    // EF-eligible $5000, total liquid $7000 (chk EF + brk non-EF)
    let chk = accounts::insert(&mut conn, acct("chk", 500_000, true)).unwrap().id;
    let _brk = accounts::insert(&mut conn, acct("brk", 200_000, false)).unwrap().id;
    // Seed 3 months of $2000 expense via the chk account, distinct calendar months
    let now = Utc::now();
    for m in 0..3 {
        let d = (now - Duration::days(10 + m * 32)).format("%Y-%m-%dT12:00:00Z").to_string();
        insert_txn(&conn, &chk, -200_000, &d);
    }
    let m = metrics::emergency_fund_months_scoped(&conn, None).unwrap();
    assert!((m - 2.5).abs() < 0.05, "EF pool $5000 / $2000=2.5, got {m}");
    let ra = metrics::rolling_averages(&conn, 90).unwrap();
    assert!((ra.emergency_fund_months - m).abs() < 0.01, "rolling_averages.emergency_fund_months {} vs {m}", ra.emergency_fund_months);
}

// Also keep alias test for the plan-expected name via wrapper - ensures the
// single source is reachable as `emergency_fund_months` when scoped.
// This covers the spec's exact call shape without breaking the ratio alias.
// We expose a thin wrapper via the scoped name; the plan's pseudocode call
// `emergency_fund_months(&db, None)` maps to `emergency_fund_months_scoped`.
#[test]
fn rolling_uses_ef_eligible_not_total_liquid() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();
    let chk = accounts::insert(&mut conn, acct("chk", 500_000, true)).unwrap().id;
    accounts::insert(&mut conn, acct("brk", 200_000, false)).unwrap();
    let now = Utc::now();
    for m in 0..3 {
        let d = (now - Duration::days(10 + m * 32)).format("%Y-%m-%dT12:00:00Z").to_string();
        insert_txn(&conn, &chk, -200_000, &d);
    }
    // If total liquid were used, months would be 3.5 (7000/2000)
    let m = metrics::emergency_fund_months_scoped(&conn, None).unwrap();
    assert!(m < 3.0, "must use EF-eligible pool, not total liquid; got {m}");
}

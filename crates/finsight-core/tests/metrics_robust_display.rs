use chrono::{Duration, Utc};
use finsight_core::{metrics, models::AccountType, repos::accounts, testing::migrated_db};
use rusqlite::params;

fn acct() -> finsight_core::models::NewAccount {
    finsight_core::models::NewAccount {
        promo_apr_expires_on: None,
        post_promo_apr_pct: None,
        owner: "me".into(),
        bank: "Bank".into(),
        r#type: AccountType::Checking,
        name: "Chk".into(),
        last4: None,
        currency: "USD".into(),
        color: "#3B82F6".into(),
        opening_balance_cents: 0,
        source: "manual".into(),
        liquidity_type: "liquid".into(),
        emergency_fund_eligible: true,
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

fn insert_txn(
    conn: &rusqlite::Connection,
    acct_id: &str,
    amount: i64,
    posted_at: &str,
    is_anomaly: i64,
) {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at) \
         VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', ?5, 0, ?3)",
        params![id, acct_id, posted_at, amount, is_anomaly],
    )
    .unwrap();
}

#[test]
fn typical_monthly_expense_ignores_one_off_spike_display_path() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();
    let acct_id = accounts::insert(&mut conn, acct()).unwrap().id;
    // Need a mutable conn for inserts via &Connection (rusqlite Connection is not &mut for execute with &Connection)
    // Use db.get() again for &mut? But migrated_db gives Db pool; we can use conn (which is PooledConnection)
    // rusqlite Connection execute works on &Connection, so fine.
    let now = Utc::now();
    // Seed 3 distinct months within 90d window but with distinct calendar months.
    // Use offsets 10, 42, 74 days ago (32-day spacing ensures distinct months).
    let dates: Vec<String> = (0..3)
        .map(|m| {
            (now - Duration::days(10 + m * 32))
                .format("%Y-%m-%dT12:00:00Z")
                .to_string()
        })
        .collect();
    // Two normal months: 190_000 expense each.
    // Third month (oldest) has normal 190_000 + spike 250_000 anomaly.
    insert_txn(&conn, &acct_id, -190_000, &dates[0], 0);
    insert_txn(&conn, &acct_id, -190_000, &dates[1], 0);
    insert_txn(&conn, &acct_id, -190_000, &dates[2], 0);
    insert_txn(&conn, &acct_id, -250_000, &dates[2], 1); // anomaly spike same month

    let r = metrics::rolling_averages(&conn, 90).unwrap();
    // Before fix this is ~ (190+190+440)/3 ≈ 273k, after fix ~190k median (anomaly excluded).
    // The plan expects 276666 before fix; we tolerate 250k-300k range.
    assert!(
        r.avg_monthly_expense_cents < 210_000,
        "robust median should ignore spike, got {}",
        r.avg_monthly_expense_cents
    );
    assert!(
        r.avg_monthly_expense_cents > 170_000,
        "got {}",
        r.avg_monthly_expense_cents
    );
    // Also verify is_estimated flag: with >=2 months of history, robust is available => not estimated
    assert!(
        !r.is_estimated,
        "with 3 months history should not be estimated"
    );
}

#[test]
fn is_estimated_when_thin_history() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();
    let acct_id = accounts::insert(&mut conn, acct()).unwrap().id;
    let now = Utc::now();
    // Only one month of data
    let d = (now - Duration::days(5))
        .format("%Y-%m-%dT12:00:00Z")
        .to_string();
    insert_txn(&conn, &acct_id, -100_000, &d, 0);
    let r = metrics::rolling_averages(&conn, 90).unwrap();
    assert!(
        r.is_estimated,
        "thin history should be estimated, got {:?}",
        r
    );
}

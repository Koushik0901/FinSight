use chrono::{Duration, Utc};
use finsight_core::{metrics, models::AccountType, repos::accounts, testing::migrated_db};
use rusqlite::params;

fn acct(name: &str, opening: i64) -> finsight_core::models::NewAccount {
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
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at) VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', ?5, 0, ?3)",
        params![id, acct_id, posted_at, amount, is_anomaly],
    )
    .unwrap();
}

/// Joint 50% split, zero-member parity, and sum(member views) == household
#[test]
fn sum_member_views_equals_household() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();

    // Two members: Alice and Bob
    let alice = finsight_core::repos::household::create_member(&mut conn, "Alice", None).unwrap();
    let bob = finsight_core::repos::household::create_member(&mut conn, "Bob", None).unwrap();

    // Accounts: Alice sole, Bob sole, Joint 50/50, Unassigned residual
    let a_sole = accounts::insert(&mut conn, acct("A", 0)).unwrap().id;
    let b_sole = accounts::insert(&mut conn, acct("B", 0)).unwrap().id;
    let joint = accounts::insert(&mut conn, acct("J", 0)).unwrap().id;
    let shared = accounts::insert(&mut conn, acct("U", 0)).unwrap().id;

    finsight_core::repos::household::set_account_owners(
        &mut conn,
        &a_sole,
        std::slice::from_ref(&alice.id),
    )
    .unwrap();
    finsight_core::repos::household::set_account_owners(
        &mut conn,
        &b_sole,
        std::slice::from_ref(&bob.id),
    )
    .unwrap();
    finsight_core::repos::household::set_account_owners(
        &mut conn,
        &joint,
        &[alice.id.clone(), bob.id.clone()],
    )
    .unwrap();
    // `shared` left unassigned -> household residual

    // Transactions: distinct amounts to make weighting visible
    // Alice sole: +300k income, -100k expense
    // Bob sole: +200k income, -50k expense
    // Joint: +100k income, -40k expense (50k / 20k each)
    // Residual: +70k income, -30k expense
    let now = Utc::now();
    let mk_date = |days_ago: i64| (now - Duration::days(days_ago)).to_rfc3339();
    let d = mk_date(5);
    for (acct_id, amt) in [
        (&a_sole, 300_000),
        (&a_sole, -100_000),
        (&b_sole, 200_000),
        (&b_sole, -50_000),
        (&joint, 100_000),
        (&joint, -40_000),
        (&shared, 70_000),
        (&shared, -30_000),
    ] {
        insert_txn(&conn, acct_id, amt, &d, 0);
    }
    // Transfer in joint must be ignored on every slice
    insert_txn(&conn, &joint, 999_999, &d, 0);
    // Mark the 999_999 as transfer
    conn.execute(
        "UPDATE transactions SET is_transfer = 1 WHERE amount_cents = 999999",
        [],
    )
    .unwrap();

    let start = "1970-01-01T00:00:00Z";

    // Zero-member parity: None path == household verbatim
    let (h_inc, h_exp) = metrics::income_expense_since_for(&conn, start, None).unwrap();
    assert_eq!(
        (h_inc, h_exp),
        metrics::income_expense_since(&conn, start).unwrap(),
        "None path == household verbatim (zero-member parity)"
    );
    assert_eq!(h_inc, 670_000, "household income 670k");
    assert_eq!(h_exp, 220_000, "household expense 220k");

    // Joint 50% split: each member gets half of joint flows
    let (a_inc, a_exp) = metrics::income_expense_since_for(&conn, start, Some(&alice.id)).unwrap();
    let (b_inc, b_exp) = metrics::income_expense_since_for(&conn, start, Some(&bob.id)).unwrap();
    assert_eq!(a_inc, 350_000, "alice: 300k sole + 50k half-joint");
    assert_eq!(a_exp, 120_000, "alice: 100k sole + 20k half-joint");
    assert_eq!(b_inc, 250_000, "bob: 200k sole + 50k half-joint");
    assert_eq!(b_exp, 70_000, "bob: 50k sole + 20k half-joint");

    // Reconciliation: members + residual == household
    let (u_inc, u_exp) = (70_000, 30_000);
    assert_eq!(
        a_inc + b_inc + u_inc,
        h_inc,
        "income reconciles with residual"
    );
    assert_eq!(
        a_exp + b_exp + u_exp,
        h_exp,
        "expense reconciles with residual"
    );

    // Balance parity: joint 50% split for balances
    // Fresh DB for balance test to avoid transaction interference
    let (_dir2, db2) = migrated_db();
    let mut conn2 = db2.get().unwrap();
    let alice2 = finsight_core::repos::household::create_member(&mut conn2, "Alice", None).unwrap();
    let bob2 = finsight_core::repos::household::create_member(&mut conn2, "Bob", None).unwrap();
    let a_sole2 = accounts::insert(&mut conn2, acct("A", 40_000)).unwrap().id;
    let joint2 = accounts::insert(&mut conn2, acct("J", 100_000)).unwrap().id;
    finsight_core::repos::household::set_account_owners(
        &mut conn2,
        &a_sole2,
        std::slice::from_ref(&alice2.id),
    )
    .unwrap();
    finsight_core::repos::household::set_account_owners(
        &mut conn2,
        &joint2,
        &[alice2.id.clone(), bob2.id.clone()],
    )
    .unwrap();

    let h_bd = metrics::balance_breakdown_for(&mut conn2, None).unwrap();
    assert_eq!(
        h_bd,
        metrics::balance_breakdown(&mut conn2).unwrap(),
        "balance None == household verbatim"
    );
    let a_bd = metrics::balance_breakdown_for(&mut conn2, Some(&alice2.id)).unwrap();
    let b_bd = metrics::balance_breakdown_for(&mut conn2, Some(&bob2.id)).unwrap();
    // Joint 50_000 each (100_000 / 2)
    assert_eq!(
        a_bd.liquid_cents, 90_000,
        "alice: 40k sole + 50k half joint"
    );
    assert_eq!(b_bd.liquid_cents, 50_000, "bob: half joint");
    assert!(
        (a_bd.liquid_cents + b_bd.liquid_cents - h_bd.liquid_cents).abs() <= 1,
        "member balances reconcile within rounding"
    );

    // Rolling averages parity: member income + member income ≈ household income
    // Use the first DB's 90-day rolling averages (single calendar month of data)
    let h_roll = metrics::rolling_averages_for(&conn, 90, None).unwrap();
    let a_roll = metrics::rolling_averages_for(&conn, 90, Some(&alice.id)).unwrap();
    let b_roll = metrics::rolling_averages_for(&conn, 90, Some(&bob.id)).unwrap();
    // Data spans one calendar month, so avg == total
    assert_eq!(h_roll.months, 1);
    assert_eq!(a_roll.months, 1);
    assert_eq!(b_roll.months, 1);
    // Income parity: weighted sums divide by same months count
    assert_eq!(
        a_roll.avg_monthly_income_cents + b_roll.avg_monthly_income_cents + 70_000,
        h_roll.avg_monthly_income_cents,
        "rolling income reconciles (weighted sums)"
    );
}

#[test]
fn member_rolling_uses_robust_median_not_fallback() {
    // This test pins the parametrization intent: a member's rolling average must
    // use the same robust-median logic as household, not a raw 90-day mean.
    // Before parametrization, member path used fallback (including anomalies),
    // while household used median (excluding anomalies) -> divergence.
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();
    let alice = finsight_core::repos::household::create_member(&mut conn, "Alice", None).unwrap();
    let acct_id = accounts::insert(&mut conn, acct("Chk", 0)).unwrap().id;
    finsight_core::repos::household::set_account_owners(
        &mut conn,
        &acct_id,
        std::slice::from_ref(&alice.id),
    )
    .unwrap();

    let now = Utc::now();
    let dates: Vec<String> = (0..3)
        .map(|m| {
            (now - Duration::days(10 + m * 32))
                .format("%Y-%m-%dT12:00:00Z")
                .to_string()
        })
        .collect();
    // Two normal months 190k, one month 190k + 250k anomaly spike
    insert_txn(&conn, &acct_id, -190_000, &dates[0], 0);
    insert_txn(&conn, &acct_id, -190_000, &dates[1], 0);
    insert_txn(&conn, &acct_id, -190_000, &dates[2], 0);
    insert_txn(&conn, &acct_id, -250_000, &dates[2], 1); // anomaly

    let h = metrics::rolling_averages(&conn, 90).unwrap();
    // Household uses robust median ≈190k (anomaly excluded)
    assert!(
        h.avg_monthly_expense_cents < 210_000 && h.avg_monthly_expense_cents > 170_000,
        "household robust should be ~190k, got {}",
        h.avg_monthly_expense_cents
    );

    let a = metrics::rolling_averages_for(&conn, 90, Some(&alice.id)).unwrap();
    // Member sole account (weight 1.0) should also be ~190k, not ~276k fallback
    assert!(
        a.avg_monthly_expense_cents < 210_000,
        "member robust should ignore spike, got {} (fallback would be ~276k)",
        a.avg_monthly_expense_cents
    );
    assert!(
        (a.avg_monthly_expense_cents - h.avg_monthly_expense_cents).abs() < 5_000,
        "member and household robust should agree for sole account: member {} vs household {}",
        a.avg_monthly_expense_cents,
        h.avg_monthly_expense_cents
    );
}

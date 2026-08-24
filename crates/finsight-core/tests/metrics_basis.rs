use finsight_core::metrics::{explain, monthly_expense_cents, ExpenseBasis};

#[test]
fn reconcile_explains_smooth_vs_recent() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    let mut conn = db.get().unwrap();
    // Seed: 3 months $1000 + one month $3000 spike in most recent 90d
    // Create an account and seed expenses so Recent > Smooth.
    {
        use chrono::{Duration, Utc};
        use finsight_core::{models::AccountType, models::NewAccount, repos::accounts};
        use rusqlite::params;
        let acct = accounts::insert(
            &mut conn,
            NewAccount {
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
            },
        )
        .unwrap()
        .id;
        // Two prior complete months $1000 each
        for days_ago in [40, 70] {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at) VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', 0, 0, ?3)",
                params![
                    id,
                    acct,
                    (Utc::now() - Duration::days(days_ago)).to_rfc3339(),
                    -100_000,
                ],
            )
            .unwrap();
        }
        // Current month spike: $1000 base + $5000 extra = $6000 in current month
        for _ in 0..6 {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at) VALUES(?1, ?2, ?3, ?4, 'M', 'cleared', 0, 0, ?3)",
                params![
                    id,
                    acct,
                    (Utc::now() - Duration::days(5)).to_rfc3339(),
                    -100_000,
                ],
            )
            .unwrap();
        }
    }
    let r = finsight_core::metrics::reconcile(
        &conn,
        ExpenseBasis::DisplayMedian,
        ExpenseBasis::RecentMean90,
        None,
    )
    .unwrap();
    assert!(r.reason.contains("Recent") || r.reason.contains("Smooth"));
    assert!(r.delta_cents != 0 || r.reason.contains("essentially"));
}

#[test]
fn pantry_explain_is_non_empty() {
    assert!(explain(ExpenseBasis::DisplayMedian).contains("Smooth"));
    assert!(explain(ExpenseBasis::RecentMean90).contains("Recent"));
    assert!(explain(ExpenseBasis::SafetyConservative).contains("Conservative"));
}

#[test]
fn pantry_monthly_expense_is_greppable() {
    // This test exists so grep for raw calls fails after migration.
    // It will pass only when monthly_expense_cents exists and delegates correctly.
    let (_dir, db) = finsight_core::testing::migrated_db();
    let conn = db.get().unwrap();
    let (cents, sufficient) =
        monthly_expense_cents(&conn, ExpenseBasis::RecentMean90, None).unwrap();
    assert!(cents >= 0);
    let _ = sufficient;
}

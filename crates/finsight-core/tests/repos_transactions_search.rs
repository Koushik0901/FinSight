use finsight_core::db::run_migrations;
use finsight_core::keychain;
use finsight_core::models::{AccountType, NewAccount, NewTransaction, TransactionStatus};
use finsight_core::repos::{accounts, transactions};
use finsight_core::Db;
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

fn base_account(name: &str, opening_balance_cents: i64, source: &str) -> NewAccount {
    NewAccount {
        owner: "Me".into(),
        bank: "Bank".into(),
        r#type: AccountType::Checking,
        name: name.into(),
        last4: None,
        currency: "USD".into(),
        color: "#fff".into(),
        opening_balance_cents,
        source: source.into(),
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

fn mk_txn(account_id: &str, amount_cents: i64, date: &str) -> NewTransaction {
    let posted_at = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc();
    NewTransaction {
        account_id: account_id.to_string(),
        posted_at,
        amount_cents,
        merchant_raw: "Test Merchant".to_string(),
        category_id: None,
        notes: None,
        status: TransactionStatus::Cleared,
        imported_id: None,
        source: None,
        raw_synced_data: None,
        pending: false,
        external_tx_id: None,
        external_account_id: None,
    }
}

#[test]
fn search_filters_by_account_substring_and_min_amount() {
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let amex = accounts::insert(&mut conn, base_account("Amex Card", 0, "manual")).unwrap();
    let chase = accounts::insert(&mut conn, base_account("Chase Checking", 0, "manual")).unwrap();
    transactions::insert(&mut conn, mk_txn(&amex.id, -7_000, "2026-05-10")).unwrap();
    transactions::insert(&mut conn, mk_txn(&amex.id, -3_000, "2026-05-11")).unwrap();
    transactions::insert(&mut conn, mk_txn(&chase.id, -9_000, "2026-05-12")).unwrap();
    let query = transactions::SearchTxnQuery {
        account: Some("amex".to_string()),
        min_amount_cents: Some(6_000),
        ..Default::default()
    };
    let rows = transactions::search(&conn, &query, 50).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].amount_cents, -7_000);
    assert_eq!(rows[0].account, "Amex Card");
}

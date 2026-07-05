use finsight_core::{
    db::run_migrations,
    keychain,
    models::{AccountType, NewAccount, NewTransaction, TransactionStatus, TxnPatch},
    repos::{accounts, run, transactions},
    Db,
};
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("et.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

fn seed(conn: &mut rusqlite::Connection) -> (String, String) {
    conn.execute(
        "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
    let acc = accounts::insert(
        conn,
        NewAccount {
            owner: "Me".into(),
            bank: "B".into(),
            r#type: AccountType::Checking,
            name: "Ch".into(),
            last4: None,
            currency: "USD".into(),
            color: "#fff".into(),
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
    .unwrap();
    let txn = transactions::insert(
        conn,
        NewTransaction {
            account_id: acc.id.clone(),
            posted_at: chrono::Utc::now(),
            amount_cents: 500,
            merchant_raw: "STARBUCKS".to_string(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
            imported_id: None,
            source: None,
            raw_synced_data: None,
            pending: false,
            external_tx_id: None,
            external_account_id: None,
        },
    )
    .unwrap();
    (acc.id, txn.id)
}

#[tokio::test]
async fn update_category_proposes_rule() {
    let (_d, db) = fresh_db();
    let txn_id = {
        let mut c = db.get().unwrap();
        seed(&mut c).1
    };
    let patch = TxnPatch {
        category_id: Some(Some("cat1".into())),
        ..Default::default()
    };
    let (updated, rule) = run(&db, move |conn| transactions::update(conn, &txn_id, patch))
        .await
        .unwrap();
    assert_eq!(updated.category_id.as_deref(), Some("cat1"));
    assert!(rule.is_some());
    assert_eq!(rule.unwrap().pattern, "STARBUCKS");
}

#[tokio::test]
async fn delete_transaction_removes_it() {
    let (_d, db) = fresh_db();
    let txn_id = {
        let mut c = db.get().unwrap();
        seed(&mut c).1
    };
    let id_clone = txn_id.clone();
    run(&db, move |conn| transactions::delete(conn, &id_clone))
        .await
        .unwrap();
    let count: i64 = db
        .get()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE id = ?1",
            rusqlite::params![txn_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

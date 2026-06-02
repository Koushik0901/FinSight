use chrono::Utc;
use finsight_core::models::{AccountType, NewAccount, NewTransaction, TransactionStatus};
use finsight_core::repos::{accounts, transactions};
use finsight_core::Db;
use tempfile::tempdir;

fn open() -> (Db, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("t.sqlcipher");
    let key = "cd".repeat(32);
    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    (db, dir)
}

#[test]
fn insert_and_list_returns_descending_by_posted_at() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "X".into(),
            last4: None,
            currency: "USD".into(),
            color: "#fff".into(),
            opening_balance_cents: 0,
            source: "manual".into(),
        },
    )
    .unwrap();

    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: acct.id.clone(),
            posted_at: Utc::now() - chrono::Duration::days(2),
            amount_cents: -4200,
            merchant_raw: "Older".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
        },
    )
    .unwrap();
    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: acct.id,
            posted_at: Utc::now(),
            amount_cents: -1234,
            merchant_raw: "Newer".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
        },
    )
    .unwrap();

    let list = transactions::list(&mut conn, transactions::TxnFilter::default()).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].merchant_raw, "Newer");
    assert_eq!(list[1].merchant_raw, "Older");
}

#[test]
fn list_filtered_by_account_id_only_returns_that_account_txns() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let a = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "mira".into(),
            bank: "Schwab".into(),
            r#type: AccountType::Checking,
            name: "A".into(),
            last4: None,
            currency: "USD".into(),
            color: "#fff".into(),
            opening_balance_cents: 0,
            source: "manual".into(),
        },
    )
    .unwrap();
    let b = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "adam".into(),
            bank: "Chase".into(),
            r#type: AccountType::Checking,
            name: "B".into(),
            last4: None,
            currency: "USD".into(),
            color: "#000".into(),
            opening_balance_cents: 0,
            source: "manual".into(),
        },
    )
    .unwrap();

    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: a.id.clone(),
            posted_at: Utc::now(),
            amount_cents: -100,
            merchant_raw: "OnlyA".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
        },
    )
    .unwrap();
    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: b.id.clone(),
            posted_at: Utc::now(),
            amount_cents: -200,
            merchant_raw: "OnlyB".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
        },
    )
    .unwrap();

    let filtered = transactions::list(
        &mut conn,
        transactions::TxnFilter {
            account_id: Some(a.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].merchant_raw, "OnlyA");
}

#[test]
fn list_respects_limit() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "X".into(),
            last4: None,
            currency: "USD".into(),
            color: "#fff".into(),
            opening_balance_cents: 0,
            source: "manual".into(),
        },
    )
    .unwrap();
    for i in 0..5 {
        transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acct.id.clone(),
                posted_at: Utc::now() - chrono::Duration::seconds(i),
                amount_cents: -1,
                merchant_raw: format!("Txn{i}"),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
            },
        )
        .unwrap();
    }
    let limited = transactions::list(
        &mut conn,
        transactions::TxnFilter {
            account_id: None,
            limit: 2,
            offset: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(limited.len(), 2);
}

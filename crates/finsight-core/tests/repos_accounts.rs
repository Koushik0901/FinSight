use finsight_core::models::{AccountType, NewAccount};
use finsight_core::repos::accounts;
use finsight_core::Db;
use tempfile::tempdir;

fn open() -> (Db, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.sqlcipher");
    let key = "ab".repeat(32);
    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    (db, dir)
}

#[test]
fn insert_then_list_summaries_returns_one() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();

    accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "Joint Checking".into(),
            last4: Some("4421".into()),
            currency: "USD".into(),
            color: "#C9F950".into(),
            opening_balance_cents: 1_482_042,
        },
    )
    .unwrap();

    let list = accounts::list_summaries(&mut conn).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].balance_cents, 1_482_042);
    assert_eq!(list[0].name, "Joint Checking");
}

#[test]
fn list_summaries_returns_empty_when_no_accounts() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    assert!(accounts::list_summaries(&mut conn).unwrap().is_empty());
}

#[test]
fn list_summaries_excludes_archived_accounts() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();

    let live = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "Live".into(),
            last4: None,
            currency: "USD".into(),
            color: "#fff".into(),
            opening_balance_cents: 100,
        },
    )
    .unwrap();
    let archived = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "Gone".into(),
            last4: None,
            currency: "USD".into(),
            color: "#000".into(),
            opening_balance_cents: 0,
        },
    )
    .unwrap();

    // Mark one archived.
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        rusqlite::params![chrono::Utc::now().to_rfc3339(), archived.id],
    )
    .unwrap();

    let list = accounts::list_summaries(&mut conn).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, live.id);
}

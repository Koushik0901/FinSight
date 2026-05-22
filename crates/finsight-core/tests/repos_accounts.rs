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

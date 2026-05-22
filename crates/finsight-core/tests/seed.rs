use finsight_core::repos::{accounts, categories, transactions};
use finsight_core::Db;
use tempfile::tempdir;

#[test]
fn seed_walks_skeleton_creates_expected_counts() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("seed.sqlcipher");
    let key = "ef".repeat(32);
    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();

    finsight_core::seed::walking_skeleton(&db).unwrap();

    let mut conn = db.get().unwrap();
    assert_eq!(accounts::list_summaries(&mut conn).unwrap().len(), 1);
    assert_eq!(categories::list(&mut conn).unwrap().len(), 4);

    let txns = transactions::list(&mut conn, transactions::TxnFilter::default()).unwrap();
    assert_eq!(txns.len(), 3);
}

#[test]
fn seed_is_idempotent_no_duplicates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("seed2.sqlcipher");
    let key = "ef".repeat(32);
    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();

    finsight_core::seed::walking_skeleton(&db).unwrap();
    finsight_core::seed::walking_skeleton(&db).unwrap();

    let mut conn = db.get().unwrap();
    assert_eq!(accounts::list_summaries(&mut conn).unwrap().len(), 1);
}

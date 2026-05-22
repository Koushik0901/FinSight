use finsight_core::Db;
use tempfile::tempdir;

#[test]
fn migrations_run_and_create_expected_tables() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("m.sqlcipher");
    let key = "ab".repeat(32);

    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).expect("migrations should apply cleanly");

    let conn = db.get().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN \
             ('accounts','transactions','categories','category_groups','merchants','audit_log','account_balances')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 7, "all 7 expected tables exist");
}

#[test]
fn migrations_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("m.sqlcipher");
    let key = "ab".repeat(32);

    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    finsight_core::db::run_migrations(&db).unwrap(); // second run is a no-op
}

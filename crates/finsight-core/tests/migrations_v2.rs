use finsight_core::{db::run_migrations, keychain, Db};
use rusqlite::params;
use tempfile::TempDir;

#[test]
fn v002_creates_phase2_tables_and_columns() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("v2.sqlcipher");
    let key = keychain::generate_random_key();
    let db = Db::open(&db_path, &key).unwrap();
    run_migrations(&db).unwrap();

    let conn = db.get().unwrap();

    // imports table exists with the right columns.
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(imports)").unwrap()
        .query_map([], |r| r.get::<_, String>(1)).unwrap()
        .filter_map(Result::ok).collect();
    for expected in ["id","source","filename","account_id","started_at",
                     "finished_at","rows_imported","rows_skipped_duplicates","error"] {
        assert!(cols.iter().any(|c| c == expected),
                "imports missing column {expected}: got {cols:?}");
    }

    // csv_import_mappings + settings tables exist.
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap()
        .query_map([], |r| r.get::<_, String>(0)).unwrap()
        .filter_map(Result::ok).collect();
    for expected in ["csv_import_mappings","settings"] {
        assert!(names.iter().any(|n| n == expected),
                "missing table {expected}: got {names:?}");
    }

    // accounts.source column added with correct default.
    let acct_cols: Vec<(String, String)> = conn
        .prepare("PRAGMA table_info(accounts)").unwrap()
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(4)?))).unwrap()
        .filter_map(Result::ok).collect();
    let source = acct_cols.iter().find(|(n, _)| n == "source").expect("accounts.source missing");
    assert!(source.1.contains("'manual'"), "default not 'manual': {:?}", source.1);

    // idx_txn_dedup index exists on transactions.
    let idx_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_txn_dedup'",
        params![], |r| r.get(0)).unwrap();
    assert_eq!(idx_count, 1, "idx_txn_dedup missing");
}

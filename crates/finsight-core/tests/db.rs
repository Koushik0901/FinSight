use finsight_core::db::Db;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

fn open() -> (Db, TempDir) {
    let dir = tempdir().unwrap();
    let path: PathBuf = dir.path().join("data.sqlcipher");
    let key = "abcd".repeat(16); // 64-char hex
    let db = Db::open(&path, &key).expect("open pool");
    (db, dir)
}

#[test]
fn opens_pool_runs_pragmas_and_creates_table() {
    let (db, _dir) = open();
    let conn = db.get().unwrap();

    let mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");

    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);

    let sd: i64 = conn
        .query_row("PRAGMA secure_delete;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sd, 1, "secure_delete must be enabled");

    // mmap MUST NOT be enabled with SQLCipher.
    let mmap: i64 = conn
        .query_row("PRAGMA mmap_size;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mmap, 0, "mmap_size must remain 0 under SQLCipher");

    conn.execute("CREATE TABLE t (id INTEGER, v TEXT)", [])
        .unwrap();
    conn.execute("INSERT INTO t VALUES (1, 'ok')", []).unwrap();
    let v: String = conn
        .query_row("SELECT v FROM t WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, "ok");
}

#[test]
fn wrong_key_rejected_at_open() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.sqlcipher");
    let key1 = "11".repeat(32);
    let key2 = "22".repeat(32);

    {
        let db = Db::open(&path, &key1).unwrap();
        let conn = db.get().unwrap();
        conn.execute("CREATE TABLE t (id INTEGER)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (1)", []).unwrap();
    }

    // `Db::open` eagerly pulls a connection so the `with_init` PRAGMAs
    // (which touch the DB header) surface a bad key immediately.
    let err = Db::open(&path, &key2);
    assert!(err.is_err(), "wrong key must be rejected at open time");
}

#[test]
fn invalid_key_format_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.sqlcipher");
    assert!(Db::open(&path, "not-hex").is_err());
    assert!(Db::open(&path, "abc").is_err()); // wrong length
}

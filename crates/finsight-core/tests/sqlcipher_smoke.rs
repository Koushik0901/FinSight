//! Verifies SQLCipher links and encrypts data on this machine.
//! Smoke test — failure here blocks Phase 0.
//!
//! Note on PRAGMA key: SQLCipher's raw-key syntax is `PRAGMA key = "x'AABB...'";`
//! (the inner `x'...'` is a blob literal). `pragma_update("key", "x'...'")` would
//! bind the value as a SQL string, causing SQLCipher to run PBKDF2 over the
//! literal characters — silently downgrading the encryption to a passphrase.
//! We use execute_batch with a formatted statement to keep the raw-key form.

use rusqlite::Connection;
use tempfile::tempdir;

const KEY_HEX: &str = "2DD29CA851E7B56E4697B0E1F08507293D761A05CE4D1B628663F411A8086D99";

fn set_key(conn: &Connection, hex: &str) {
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";")).unwrap();
}

#[test]
fn open_encrypts_writes_and_reads_back() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.sqlcipher");

    // Open + key + create + insert.
    {
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, KEY_HEX);
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)", [])
            .unwrap();
        conn.execute("INSERT INTO t (v) VALUES (?1)", ["hello"]).unwrap();
    }

    // Reopen with same key, read it back.
    {
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, KEY_HEX);
        let v: String = conn
            .query_row("SELECT v FROM t WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
    }

    // Reopen with WRONG key — must fail to read.
    {
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, "0000000000000000000000000000000000000000000000000000000000000000");
        let res: Result<String, _> = conn.query_row("SELECT v FROM t WHERE id = 1", [], |r| r.get(0));
        assert!(res.is_err(), "Wrong key must fail to read the encrypted DB");
    }
}

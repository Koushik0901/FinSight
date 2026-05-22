use crate::error::{CoreError, CoreResult};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::Path;

#[derive(Clone)]
pub struct Db {
    pool: Pool<SqliteConnectionManager>,
}

impl Db {
    /// Open a SQLCipher-encrypted pool at `path` using `key_hex` (64 hex chars = 32 bytes).
    /// Runs initial PRAGMAs on every new connection.
    ///
    /// IMPORTANT: SQLCipher's raw-key syntax requires `PRAGMA key = "x'AABB...'";`.
    /// We use `execute_batch` for the key (parameter-bound PRAGMA values trigger PBKDF2)
    /// and `pragma_update` for the rest.
    pub fn open(path: &Path, key_hex: &str) -> CoreResult<Self> {
        if key_hex.len() != 64 || !key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CoreError::InvalidState(
                "key_hex must be 64 ASCII hex chars (32 bytes)".into(),
            ));
        }
        let key_hex = key_hex.to_owned();

        let manager =
            SqliteConnectionManager::file(path).with_init(move |conn: &mut Connection| {
                // Raw 256-bit key. MUST come first, before any other PRAGMA touches the DB.
                conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))?;

                // SQLCipher hygiene
                conn.execute_batch("PRAGMA cipher_memory_security = ON;")?;
                conn.pragma_update(None, "secure_delete", true)?;

                // Standard SQLite tuning. NOTE: do NOT set mmap_size with SQLCipher —
                // SQLCipher 4 does not support memory-mapped I/O and can leak
                // unencrypted pages to swap if enabled.
                conn.pragma_update(None, "journal_mode", "WAL")?;
                conn.pragma_update(None, "synchronous", "NORMAL")?;
                conn.pragma_update(None, "cache_size", -65536_i64)?;
                conn.pragma_update(None, "foreign_keys", true)?;
                conn.pragma_update(None, "busy_timeout", 5000_i64)?;
                Ok(())
            });

        let pool = Pool::builder().max_size(4).build(manager).map_err(|e| {
            CoreError::InvalidState(format!("failed to build connection pool: {e}"))
        })?;

        // Touch a connection once now to surface key/file errors immediately.
        let _ = pool.get()?;
        Ok(Self { pool })
    }

    pub fn get(&self) -> CoreResult<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// Run SQLite's integrity check. Returns "ok" on success, or the first error string.
    pub fn integrity_check(&self) -> CoreResult<String> {
        let conn = self.get()?;
        let v: String = conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0))?;
        Ok(v)
    }
}

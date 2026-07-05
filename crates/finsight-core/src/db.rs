use crate::error::{CoreError, CoreResult};
use crate::reset_barrier::ResetBarrier;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use refinery::embed_migrations;
use rusqlite::Connection;
use std::path::Path;
use zeroize::Zeroizing;

embed_migrations!("./migrations");

#[derive(Clone)]
pub struct Db {
    pool: Pool<SqliteConnectionManager>,
    /// Coordinates Delete-All against in-flight background writers. Shared
    /// across all clones of this `Db` so every writer and the reset path see
    /// one barrier.
    barrier: ResetBarrier,
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
        let key_hex = Zeroizing::new(key_hex.to_owned());

        let manager =
            SqliteConnectionManager::file(path).with_init(move |conn: &mut Connection| {
                // Raw 256-bit key. MUST come first, before any other PRAGMA touches the DB.
                // The format! produces a String that contains the key — wrap in Zeroizing
                // so it's wiped from memory when the closure invocation returns.
                let pragma = Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", &*key_hex));
                conn.execute_batch(&pragma)?;

                // SQLCipher hygiene
                conn.execute_batch("PRAGMA cipher_memory_security = ON;")?;
                conn.pragma_update(None, "secure_delete", true)?;

                // Standard SQLite tuning. NOTE: do NOT set mmap_size with SQLCipher —
                // SQLCipher 4 does not support memory-mapped I/O and can leak
                // unencrypted pages to swap if enabled.
                conn.pragma_update(None, "journal_mode", "WAL")?;
                conn.pragma_update(None, "synchronous", "NORMAL")?;
                // negative value = KiB → 64 MiB
                conn.pragma_update(None, "cache_size", -65536_i64)?;
                conn.pragma_update(None, "foreign_keys", true)?;
                // ms
                conn.pragma_update(None, "busy_timeout", 5000_i64)?;
                Ok(())
            });

        // 4 connections is plenty for a single-user desktop app.
        //
        // min_idle = Some(0): r2d2's default is min_idle = max_size, which builds
        // all 4 connections eagerly in parallel during Pool::build(). Each runs
        // with_init, and on SQLCipher + WAL the first connection holds the *-shm
        // file briefly while setting up WAL mode; the other three race for the
        // same lock and surface a transient "database is locked" error at
        // startup. Lazy construction (min_idle = 0) sidesteps the race entirely.
        let pool = Pool::builder()
            .max_size(4)
            .min_idle(Some(0))
            .build(manager)
            .map_err(|e| {
                CoreError::InvalidState(format!("failed to build connection pool: {e}"))
            })?;

        // Touch a connection once now to surface key/file errors immediately.
        let _ = pool.get()?;
        Ok(Self {
            pool,
            barrier: ResetBarrier::new(),
        })
    }

    pub fn get(&self) -> CoreResult<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// The reset barrier coordinating Delete-All with in-flight background
    /// writers (import cascade, agent categorizer). Writers snapshot
    /// `reset_barrier().epoch()` when they start and take a
    /// `writer_lease(start_epoch)` across their commit; the reset path takes
    /// `begin_reset()`, which drains outstanding leases before the wipe.
    pub fn reset_barrier(&self) -> &ResetBarrier {
        &self.barrier
    }

    /// Runs SQLite's integrity check. Returns "ok" when the database is clean.
    /// On corruption, SQLite returns multiple rows describing each problem;
    /// they are joined by newlines so the caller logs everything.
    pub fn integrity_check(&self) -> CoreResult<String> {
        let conn = self.get()?;
        let mut stmt = conn.prepare("PRAGMA integrity_check;")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out.join("\n"))
    }

    /// Apply all pending migrations.
    pub fn run_migrations_self(&self) -> CoreResult<()> {
        run_migrations(self)
    }
}

pub fn run_migrations(db: &Db) -> CoreResult<()> {
    let mut conn = db.get()?;
    migrations::runner().run(&mut *conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v003_tables_exist() {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("v003.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        let conn = db.get().unwrap();
        let cats: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='categorizations'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cats, 1, "categorizations table missing");
        let rules: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rules'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rules, 1, "rules table missing");
    }
}

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

    /// Force a full WAL checkpoint and truncate the WAL back to zero. Without
    /// this the WAL can grow to the size of the database (SQLite only
    /// auto-checkpoints at a page threshold and never truncates), which both
    /// wastes disk and lengthens crash recovery. Safe to call any time; a
    /// concurrent reader may keep the WAL non-empty, which is fine.
    pub fn checkpoint(&self) -> CoreResult<()> {
        let conn = self.get()?;
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |_| Ok(()))?;
        Ok(())
    }

    /// Number of embedded migrations not yet applied to this database. Used to
    /// decide whether to take a pre-migration backup.
    pub fn pending_migration_count(&self) -> CoreResult<usize> {
        let mut conn = self.get()?;
        let embedded = migrations::runner().get_migrations().len();
        let applied = migrations::runner()
            .get_applied_migrations(&mut *conn)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(embedded.saturating_sub(applied))
    }

    /// Write a consistent encrypted copy of the database into `dir`, named
    /// `data.backup-<label>-<timestamp>.sqlcipher`. Uses `VACUUM INTO`, which
    /// produces a transactionally-consistent snapshot (WAL-safe, unlike a raw
    /// file copy) encrypted with the same key. Returns the backup path.
    /// Retains at most `keep` most-recent backups in `dir` (older pruned).
    pub fn backup(&self, dir: &Path, label: &str, keep: usize) -> CoreResult<std::path::PathBuf> {
        std::fs::create_dir_all(dir)
            .map_err(|e| CoreError::InvalidState(format!("backup dir: {e}")))?;
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let safe_label: String = label
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let out = dir.join(format!("data.backup-{safe_label}-{ts}.sqlcipher"));
        // VACUUM INTO requires a literal path; escape single quotes.
        let target = out.to_string_lossy().replace('\'', "''");
        {
            let conn = self.get()?;
            conn.execute_batch(&format!("VACUUM INTO '{target}';"))?;
        }
        prune_backups(dir, keep);
        Ok(out)
    }
}

/// Keep only the `keep` most recent `data.backup-*.sqlcipher` files in `dir`.
fn prune_backups(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("data.backup-") && n.ends_with(".sqlcipher"))
                .unwrap_or(false)
        })
        .collect();
    // Timestamped names sort chronologically; newest last.
    backups.sort();
    if backups.len() > keep {
        for old in &backups[..backups.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
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
    fn backup_creates_a_readable_encrypted_snapshot_and_prunes() {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("main.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        db.get()
            .unwrap()
            .execute(
                "INSERT INTO settings(key, value) VALUES('probe', '\"hello\"')",
                [],
            )
            .unwrap();

        let backups = dir.path().join("backups");
        // Take 3 backups but keep only 2 → oldest pruned.
        let b1 = db.backup(&backups, "test", 2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let _b2 = db.backup(&backups, "test", 2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let b3 = db.backup(&backups, "test", 2).unwrap();

        let remaining = std::fs::read_dir(&backups).unwrap().count();
        assert_eq!(remaining, 2, "prune keeps only the newest 2 backups");
        assert!(!b1.exists(), "the oldest backup was pruned");
        assert!(b3.exists());

        // The newest backup opens with the same key and carries the data.
        let restored = crate::Db::open(&b3, &key).unwrap();
        let v: String = restored
            .get()
            .unwrap()
            .query_row("SELECT value FROM settings WHERE key='probe'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, "\"hello\"");
    }

    #[test]
    fn checkpoint_and_pending_migration_count_work() {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("m.sqlcipher"), &key).unwrap();
        // Fresh DB before migrations: every embedded migration is pending.
        assert!(db.pending_migration_count().unwrap() > 0);
        run_migrations(&db).unwrap();
        assert_eq!(
            db.pending_migration_count().unwrap(),
            0,
            "no migrations pending after run"
        );
        db.checkpoint().expect("checkpoint truncates the WAL");
    }

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

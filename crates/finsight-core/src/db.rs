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
                // negative value = KiB → 32 MiB. Halved from 64 MiB when
                // max_size went 4 → 8 below: SQLite's page cache is per
                // connection and NOT shared, so the pool's memory ceiling is
                // max_size × cache_size. Keeping that product constant buys
                // double the read concurrency at the same worst-case footprint.
                conn.pragma_update(None, "cache_size", -32768_i64)?;
                conn.pragma_update(None, "foreign_keys", true)?;
                // Keep sorter/temp-table spill in RAM. A win twice over: GROUP
                // BY / ORDER BY over a large transaction set stops round-
                // tripping through a temp file, AND those spills never reach
                // the disk — SQLCipher does not encrypt temp files written to
                // the default location, so this closes a plaintext leak on the
                // same line that makes the query faster.
                conn.pragma_update(None, "temp_store", "MEMORY")?;
                // ms
                conn.pragma_update(None, "busy_timeout", 5000_i64)?;
                Ok(())
            });

        // 8 readers. This pool is per user (the server builds one lazily per
        // logged-in account), and it is the binding constraint on how fast a
        // screen paints: a route like Today fans out ~10 tanstack-query calls
        // at once, `run()` hands each to a Tokio blocking thread, and then they
        // all queue here. WAL lets readers run concurrently with each other and
        // with a writer, so the old ceiling of 4 was serialising work SQLite
        // was perfectly willing to do in parallel. Paired with the halved
        // per-connection cache_size above, the memory ceiling is unchanged.
        //
        // min_idle = Some(0): r2d2's default is min_idle = max_size, which builds
        // every connection eagerly in parallel during Pool::build(). Each runs
        // with_init, and on SQLCipher + WAL the first connection holds the *-shm
        // file briefly while setting up WAL mode; the other three race for the
        // same lock and surface a transient "database is locked" error at
        // startup. Lazy construction (min_idle = 0) sidesteps the race entirely.
        let pool = Pool::builder()
            .max_size(8)
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
    analyze_with_bounded_cost(&conn);
    Ok(())
}

/// Give the query planner table statistics to work from.
///
/// Nothing in this codebase had ever run ANALYZE, so with 55 indexes across
/// 60-odd migrations SQLite was choosing between them on built-in heuristics
/// alone — which is how a query ends up scanning `transactions` when a
/// perfectly good index was sitting right there.
///
/// **Why not `PRAGMA optimize`**, which is the usual advice: it picks what to
/// analyse from *the current connection's* in-memory record of which tables
/// that connection has modified. Every DB handle here comes out of an r2d2
/// pool, so the connection running this has modified nothing, and `optimize`
/// correctly concludes there is nothing to do — on every startup, forever. It
/// is a silent no-op behind a pool. That is what
/// `migrations_populate_query_planner_statistics` caught, and it is the whole
/// reason this is a plain ANALYZE.
///
/// **Why unconditionally**, rather than only when `sqlite_stat1` is absent:
/// the first run happens on an empty database, where ANALYZE creates the table
/// but has nothing to record. Gating on "no stats yet" would therefore mark
/// the job done at exactly the moment it accomplished nothing, and the user
/// who later imports five years of transactions would never get statistics.
///
/// **Why that is affordable**: `analysis_limit` caps the rows examined per
/// index, so this is bounded work — a few hundred row visits per index rather
/// than a full scan — which is precisely the knob SQLite added to make ANALYZE
/// cheap enough to run routinely. The estimates come out approximate, which is
/// all the planner needs.
///
/// Best-effort: failing to gather statistics means "run with the heuristics we
/// had", never a failed startup, so migrations are never rolled back over it.
fn analyze_with_bounded_cost(conn: &Connection) {
    if let Err(e) = conn.execute_batch("PRAGMA analysis_limit=400; ANALYZE;") {
        tracing::debug!("ANALYZE skipped, query planner keeps its previous stats: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `run_migrations` ends by gathering query-planner statistics, and that is
    /// the kind of call that fails silently: errors go to `tracing::debug!`,
    /// and on an empty database it legitimately has nothing to record. A test
    /// that merely ran migrations on a fresh DB would pass whether the code
    /// worked or was deleted outright.
    ///
    /// So this models the shape that actually matters — a database that was
    /// created empty and only later filled with data, which is every real user
    /// — and asserts on the artifact ANALYZE produces. Rows in `sqlite_stat1`
    /// mean the planner has statistics instead of guessing between 55 indexes.
    ///
    /// This is also the regression test for two specific wrong implementations:
    /// `PRAGMA optimize` (a no-op behind a connection pool) and "ANALYZE only
    /// when `sqlite_stat1` is missing" (marks itself done during the empty
    /// first run, so real data never gets analysed).
    #[test]
    fn migrations_populate_query_planner_statistics() {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("main.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();

        {
            let conn = db.get().unwrap();
            for i in 0..500 {
                conn.execute(
                    "INSERT INTO settings(key, value) VALUES(?1, '\"v\"')",
                    [format!("probe-{i}")],
                )
                .unwrap();
            }
        }

        // Second pass: migrations are already applied, so this exercises only
        // the analysis step — exactly what every subsequent startup does.
        run_migrations(&db).unwrap();

        let conn = db.get().unwrap();
        let stats: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'sqlite_stat1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            stats, 1,
            "run_migrations should have run ANALYZE and created sqlite_stat1; \
             0 means the statistics step is a silent no-op"
        );

        let rows: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_stat1", [], |r| r.get(0))
            .unwrap();
        assert!(
            rows > 0,
            "sqlite_stat1 exists but holds no statistics — ANALYZE ran on the empty \
             database and then never again, so the planner still has nothing for the \
             500 rows inserted since"
        );
    }

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

//! Startup cascade: refreshes derived state when a database "wakes up" —
//! desktop app boot, and (Phase 2) server-side per-user login catch-up for
//! jobs missed while that user's DB key was not resident in memory.
//!
//! Extracted verbatim (behavior-preserving) from the Tauri app's `.setup()`
//! closure: same order, same steps, same error recording. Every step here is
//! best-effort — a failure is recorded into `StartupReport::warnings` rather
//! than aborting the cascade, because on the server a single corrupt or
//! mid-migration per-user DB must not take down the shared process.

use finsight_core::Db;
use std::path::Path;

/// Result of a startup cascade run: a human-readable one-line summary of what
/// was refreshed (may be empty when there was nothing to report) and the list
/// of individually-labelled non-fatal step failures.
pub struct StartupReport {
    pub summary: String,
    pub warnings: Vec<String>,
    /// Set when `run_migrations` itself failed. Every other step here is
    /// best-effort, but a failed migration means the schema does not match the
    /// code: callers that can refuse to serve the database (the server's
    /// per-user bootstrap, which turns this into a 500 for that one user)
    /// should check this and bail rather than run on a broken schema. The
    /// desktop caller stays lenient — the user needs the app open to restore
    /// the pre-migration backup that was just taken below.
    pub migration_error: Option<String>,
}

/// Everything FinSight refreshes when a database "wakes up": desktop app
/// startup, and server-side user login. Takes an already-opened `Db` and the
/// per-user (or per-install) backups directory; the caller owns everything
/// that happens before `Db::open` — app-data-dir resolution, staged-restore
/// file swap, keychain/wrapped key load.
pub fn run_startup_cascade(db: &Db, backups_dir: &Path) -> StartupReport {
    // ── Durability guards (P0-4) ──────────────────────────────────────
    // 1. Verify the database is not corrupt. Record the result so the
    //    Settings → Data & backups panel can show it; a failure does NOT
    //    block startup (the user needs the app open to restore a backup).
    let integrity = db
        .integrity_check()
        .unwrap_or_else(|e| format!("check failed: {e}"));
    if integrity.trim() != "ok" {
        eprintln!("⚠ database integrity check: {integrity}");
    }
    // 2. Take a consistent encrypted backup BEFORE applying any pending
    //    migration, so a failed/again-corrupting migration is always
    //    recoverable. Only when migrations are actually pending (keeps
    //    the backup set meaningful and avoids a copy on every launch).
    let pending = db.pending_migration_count().unwrap_or(0);
    let mut startup_warnings: Vec<String> = Vec::new();
    let mut last_backup: Option<String> = None;
    if pending > 0 {
        match db.backup(backups_dir, "pre-migration", 10) {
            Ok(p) => last_backup = Some(p.to_string_lossy().to_string()),
            Err(e) => startup_warnings.push(format!("pre-migration backup failed: {e}")),
        }
    }
    let mut migration_error: Option<String> = None;
    if let Err(e) = finsight_core::db::run_migrations(db) {
        startup_warnings.push(format!("migrations: {e}"));
        migration_error = Some(e.to_string());
    }
    if let Err(e) = crate::provider::migrate_provider_settings(db) {
        startup_warnings.push(format!("provider migration: {e}"));
    }
    if let Ok(conn) = db.get() {
        let _ = finsight_core::settings::set(&conn, "data.integrity_status", &integrity);
        let _ = finsight_core::settings::set(
            &conn,
            "data.integrity_checked_at",
            &chrono::Utc::now().to_rfc3339(),
        );
        if let Some(p) = &last_backup {
            let _ = finsight_core::settings::set(&conn, "data.last_backup_path", p);
            let _ = finsight_core::settings::set(
                &conn,
                "data.last_backup_at",
                &chrono::Utc::now().to_rfc3339(),
            );
        }
    }
    // Best-effort: derive balances for existing imported accounts (so the
    // "$0 after import" state resolves without a re-import), record today's
    // net-worth snapshot, and recompute statistical anomaly flags so
    // existing imported data populates without waiting for a re-import.
    // Each cascade step is best-effort, but a FAILURE is recorded (not
    // silently swallowed) so the user can see that derived data may be
    // stale, instead of the old `let _ =` that hid real problems.
    let mut startup_summary = String::new();
    if let Ok(mut conn) = db.get() {
        macro_rules! step {
            ($label:expr, $e:expr) => {
                if let Err(err) = $e {
                    startup_warnings.push(format!("{}: {err}", $label));
                }
            };
        }
        // Re-run the deterministic builtin pass so transfer flags reflect
        // the current keyword list (idempotent; fixes stale is_transfer),
        // then pair cross-account transfer legs so existing imports gain
        // pairing without a re-import. Positive outcomes are summarized
        // (P3: startup mutation transparency) — the user can see WHAT
        // launch changed, not just whether something failed.
        let mut startup_summary_parts: Vec<String> = Vec::new();
        match finsight_core::categorize::apply_builtin_categorization(&mut conn) {
            Ok(n) if n > 0 => {
                startup_summary_parts.push(format!("categorized {n}"));
            }
            Ok(_) => {}
            Err(err) => startup_warnings.push(format!("startup categorization: {err}")),
        }
        match finsight_core::categorize::pair_transfers(&mut conn) {
            Ok(n) if n > 0 => {
                startup_summary_parts.push(format!(
                    "matched {n} transfer pair{}",
                    if n == 1 { "" } else { "s" }
                ));
            }
            Ok(_) => {}
            Err(err) => startup_warnings.push(format!("startup transfer pairing: {err}")),
        }
        if let Ok(ids) = conn
            .prepare("SELECT id FROM accounts WHERE archived_at IS NULL")
            .and_then(|mut s| {
                s.query_map([], |r| r.get::<_, String>(0))
                    .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
            })
        {
            for id in ids {
                step!(
                    "startup balance recompute",
                    finsight_core::repos::accounts::recompute_balance_if_linked(&mut conn, &id)
                );
            }
        }
        step!(
            "startup net-worth snapshot",
            finsight_core::repos::net_worth::record_today(&mut conn)
        );
        step!(
            "startup net-worth backfill",
            finsight_core::repos::net_worth::backfill_history_from_transactions(&mut conn)
        );
        match finsight_core::anomaly::recompute_anomalies(&mut conn) {
            Ok(n) if n > 0 => {
                startup_summary_parts.push(format!(
                    "flagged {n} unusual charge{}",
                    if n == 1 { "" } else { "s" }
                ));
            }
            Ok(_) => {}
            Err(err) => startup_warnings.push(format!("startup anomaly recompute: {err}")),
        }
        startup_summary = if startup_summary_parts.is_empty() {
            String::new()
        } else {
            format!("Refreshed on launch: {}", startup_summary_parts.join(" · "))
        };
        let _ = finsight_core::settings::set(&conn, "data.startup_summary", &startup_summary);
        let _ = finsight_core::settings::set(&conn, "data.startup_warnings", &startup_warnings);
    }
    // Truncate the WAL now that the startup write burst is done, so it
    // doesn't linger at the size of the whole database between sessions.
    let _ = db.checkpoint();

    StartupReport {
        summary: startup_summary,
        warnings: startup_warnings,
        migration_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, settings};
    use rusqlite::{params, Connection};
    use tempfile::TempDir;

    /// An opened-but-NOT-migrated database — the state a real cold start is in,
    /// and the only state in which the pre-migration backup guard can fire.
    fn unmigrated_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("startup.sqlcipher"), &key).unwrap();
        (dir, db)
    }

    fn fresh_db() -> (TempDir, Db) {
        let (dir, db) = unmigrated_db();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    /// Minimal fixture the builtin categorization pass can actually act on: one
    /// category the keyword map targets, one account, one uncategorized txn
    /// whose merchant matches a builtin keyword.
    fn seed_categorizable_txn(conn: &Connection) {
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('daily','Daily',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id,group_id,label,color,sort_order) \
             VALUES('dining','daily','Dining','#fff',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Credit','Card','USD','#000','manual','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t1','a1','2024-01-01T00:00:00Z',-1500,?1,'cleared',0,'2024-01-01T00:00:00Z')",
            params!["STARBUCKS #1234"],
        )
        .unwrap();
    }

    #[test]
    fn runs_without_panic_on_a_fresh_migrated_db_and_writes_settings() {
        let (_dir, db) = fresh_db();
        let backups_dir = TempDir::new().unwrap();

        // Must not panic even though the DB has no data at all.
        let report = run_startup_cascade(&db, backups_dir.path());

        // Summary may legitimately be empty (nothing to categorize/flag on a
        // brand new DB), but warnings should be empty for a healthy fresh DB.
        assert!(
            report.warnings.is_empty(),
            "unexpected warnings on fresh db: {:?}",
            report.warnings
        );
        assert!(report.migration_error.is_none());

        // Assert the ACTUAL values written, not merely that the keys exist —
        // every one of these is `Some("")`-able, so `is_some()` would pass even
        // if the cascade body were deleted.
        let conn = db.get().unwrap();
        let integrity_status: Option<String> =
            settings::get(&conn, "data.integrity_status").unwrap();
        assert_eq!(integrity_status.as_deref(), Some("ok"));
        let checked_at: Option<String> =
            settings::get(&conn, "data.integrity_checked_at").unwrap();
        assert!(
            checked_at.is_some_and(|t| chrono::DateTime::parse_from_rfc3339(&t).is_ok()),
            "integrity_checked_at should be a parseable rfc3339 timestamp"
        );
        // Already migrated → no pending migrations → deliberately no backup.
        let last_backup: Option<String> = settings::get(&conn, "data.last_backup_path").unwrap();
        assert!(
            last_backup.is_none(),
            "no backup should be taken when nothing is pending"
        );
        let warnings: Option<Vec<String>> = settings::get(&conn, "data.startup_warnings").unwrap();
        assert_eq!(warnings, Some(Vec::new()), "warnings persisted as a JSON array");
    }

    /// The summary is a real report of work done, not a constant. Deleting the
    /// summary-building block must fail this test.
    #[test]
    fn summary_reports_the_work_the_cascade_actually_did() {
        let (_dir, db) = fresh_db();
        let backups_dir = TempDir::new().unwrap();
        {
            let conn = db.get().unwrap();
            seed_categorizable_txn(&conn);
        }

        let report = run_startup_cascade(&db, backups_dir.path());

        assert!(
            report.summary.contains("categorized 1"),
            "summary should report the one categorized txn, got {:?}",
            report.summary
        );
        // …and the transaction really was categorized, not just counted.
        let conn = db.get().unwrap();
        let cat: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cat.as_deref(), Some("dining"));
        // The summary is mirrored into settings verbatim for the UI.
        let persisted: Option<String> = settings::get(&conn, "data.startup_summary").unwrap();
        assert_eq!(persisted.as_deref(), Some(report.summary.as_str()));

        // A second run has nothing left to do → empty summary. (Proves the
        // summary tracks real work rather than being emitted unconditionally.)
        let report2 = run_startup_cascade(&db, backups_dir.path());
        assert_eq!(report2.summary, "");
    }

    /// Regression: the pre-migration backup guard must fire BEFORE migrations
    /// run. If a caller migrates first and only then invokes the cascade, the
    /// `pending > 0` gate reads 0 and the snapshot is silently never taken.
    #[test]
    fn takes_a_pre_migration_backup_before_migrating() {
        let (_dir, db) = unmigrated_db();
        let backups_dir = TempDir::new().unwrap();

        assert!(
            db.pending_migration_count().unwrap() > 0,
            "fixture must start unmigrated or this test is vacuous"
        );

        let report = run_startup_cascade(&db, backups_dir.path());
        assert!(report.migration_error.is_none(), "migrations should succeed");
        assert!(
            report.warnings.is_empty(),
            "unexpected warnings: {:?}",
            report.warnings
        );

        // A snapshot file actually landed on disk…
        let snapshots: Vec<_> = std::fs::read_dir(backups_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pre-migration"))
            .collect();
        assert_eq!(
            snapshots.len(),
            1,
            "exactly one pre-migration backup expected, found {snapshots:?}"
        );
        assert!(snapshots[0].metadata().unwrap().len() > 0, "backup is empty");

        // …and it was recorded for the Settings → Data & backups panel.
        let conn = db.get().unwrap();
        let path: Option<String> = settings::get(&conn, "data.last_backup_path").unwrap();
        assert_eq!(
            path.as_deref(),
            Some(snapshots[0].path().to_string_lossy().as_ref())
        );
        let at: Option<String> = settings::get(&conn, "data.last_backup_at").unwrap();
        assert!(at.is_some(), "last_backup_at should be recorded");

        // Migrations did run after the snapshot was safely on disk.
        assert_eq!(db.pending_migration_count().unwrap(), 0);
    }
}

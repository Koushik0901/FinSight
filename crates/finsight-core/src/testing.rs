//! Fast database fixtures for tests.
//!
//! # Why this exists
//!
//! Every test that touches data used to build its database the same way: make a
//! temp dir, generate a key, `Db::open`, then `run_migrations`. Measured, that
//! last step costs **683 ms** and the other three together cost **4.5 ms** —
//! replaying 62 migrations, each in its own transaction with its own
//! bookkeeping, is essentially the entire fixed cost of a test.
//!
//! Multiplied across the suite that was minutes of wall clock spent rebuilding
//! byte-identical schemas.
//!
//! # How it works
//!
//! Migrate **once per test process**, then copy the resulting file for each
//! test. Copying a fully-migrated SQLCipher database is ~1 ms, and opening the
//! copy is ~24 ms, so a fixture costs ~25 ms instead of ~688 ms — a measured
//! **27×**.
//!
//! The template is checkpointed before use so the schema lives entirely in the
//! main database file; without that the copy would miss whatever still sat in
//! the write-ahead log and open as an empty database.
//!
//! # What it does not replace
//!
//! Tests *about* migrations — that they apply cleanly, that they are ordered,
//! that a legacy database upgrades — must keep calling `run_migrations` against
//! a genuinely empty database. This helper hands out the *result* of migrating,
//! which is precisely the thing those tests exist to verify.

use crate::db::{run_migrations, Db};
use crate::error::CoreResult;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tempfile::TempDir;

/// The key every template-derived test database uses.
///
/// Fixed rather than random because a copied database can only be opened with
/// the key it was created under. Tests do not care which key they get — the
/// ones that *do* care about key handling exercise `Db::open` directly, and
/// should keep doing so.
pub const TEST_DB_KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

/// Process-wide migrated template. The `TempDir` is held for the life of the
/// process so the file outlives every copy taken from it.
static TEMPLATE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();

fn template_path() -> &'static Path {
    let (_dir, path) = TEMPLATE.get_or_init(|| {
        let dir = TempDir::new().expect("temp dir for the migration template");
        let path = dir.path().join("template.sqlcipher");
        let db = Db::open(&path, TEST_DB_KEY).expect("open the migration template");
        run_migrations(&db).expect("migrate the template");
        // Fold the WAL into the main file. A copy taken before this would carry
        // the schema in a sidecar we do not copy, and open as an empty DB.
        db.checkpoint().expect("checkpoint the migration template");
        drop(db);
        (dir, path)
    });
    path
}

/// A migrated database at `path`, built by copying the process template.
///
/// Equivalent to `Db::open` + `run_migrations`, ~27× faster. Use
/// [`TEST_DB_KEY`] if the test needs the key.
pub fn migrated_db_at(path: &Path) -> CoreResult<Db> {
    std::fs::copy(template_path(), path).map_err(|e| {
        crate::error::CoreError::InvalidState(format!(
            "could not copy the migration template to {}: {e}",
            path.display()
        ))
    })?;
    Db::open(path, TEST_DB_KEY)
}

/// A migrated database in a fresh temp dir.
///
/// The `TempDir` is returned so the caller can keep it alive — dropping it
/// deletes the database. This is the shape almost every fixture wants:
///
/// ```ignore
/// let (_dir, db) = finsight_core::testing::migrated_db();
/// ```
pub fn migrated_db() -> (TempDir, Db) {
    let dir = TempDir::new().expect("temp dir for a test database");
    let db = migrated_db_at(&dir.path().join("test.sqlcipher"))
        .expect("open a database from the migration template");
    (dir, db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_database_has_the_migrated_schema() {
        let (_dir, db) = migrated_db();
        let conn = db.get().unwrap();
        // A table from an early migration and one from a late migration: if the
        // checkpoint were missing, or only part of the WAL made it into the
        // copy, one of these would be absent.
        for table in ["accounts", "transactions", "categories"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{table} should exist in a template-derived database");
        }
    }

    #[test]
    fn each_fixture_is_independent() {
        let (_d1, a) = migrated_db();
        let (_d2, b) = migrated_db();

        a.get()
            .unwrap()
            .execute(
                "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at)
                 VALUES('x','Me','B','Checking','C','USD','#fff','manual','2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();

        // Copies must not share state — a row written to one must not appear in
        // the other, or tests would contaminate each other through the template.
        let n: i64 = b
            .get()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "fixtures must be independent copies");
    }

    #[test]
    fn the_schema_matches_running_migrations_for_real() {
        // The whole premise is that a copied template is indistinguishable from
        // a migrated database. Verify that rather than assume it.
        let (_d1, from_template) = migrated_db();

        let dir = TempDir::new().unwrap();
        let real = Db::open(&dir.path().join("real.sqlcipher"), TEST_DB_KEY).unwrap();
        run_migrations(&real).unwrap();

        let dump = |db: &Db| -> Vec<String> {
            let conn = db.get().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT type||' '||name||' '||COALESCE(sql,'') FROM sqlite_master
                     WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
                )
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };

        assert_eq!(
            dump(&from_template),
            dump(&real),
            "a template-derived database must have exactly the migrated schema"
        );
    }
}

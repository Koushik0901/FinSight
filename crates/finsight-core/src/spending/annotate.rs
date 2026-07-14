//! Sticky user verdicts on spending drivers (keyed by canonical merchant key).
//! The "remember" half of propose → confirm → remember: once set, every
//! decompose honours it so an accepted driver drops out of the "levers".

use crate::error::CoreResult;
use rusqlite::Connection;
use std::collections::HashMap;

/// The verdicts a user can stick on a driver.
pub const VERDICTS: [&str; 3] = ["one_off", "expected", "investment"];

/// Upsert a verdict for a merchant key. `verdict` must be one of [`VERDICTS`].
pub fn set_annotation(conn: &Connection, merchant_key: &str, verdict: &str, note: Option<&str>) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO spending_driver_annotations(merchant_key, verdict, note, created_at, updated_at) \
         VALUES(?1, ?2, ?3, datetime('now'), datetime('now')) \
         ON CONFLICT(merchant_key) DO UPDATE SET verdict = ?2, note = ?3, updated_at = datetime('now')",
        rusqlite::params![merchant_key, verdict, note],
    )?;
    Ok(())
}

/// Remove a verdict (the driver returns to computed persistence).
pub fn clear_annotation(conn: &Connection, merchant_key: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM spending_driver_annotations WHERE merchant_key = ?1",
        rusqlite::params![merchant_key],
    )?;
    Ok(())
}

/// All current verdicts as `merchant_key -> verdict`.
pub fn annotations(conn: &Connection) -> CoreResult<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT merchant_key, verdict FROM spending_driver_annotations")?;
    let map = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("an.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn set_update_clear_roundtrip() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        set_annotation(&conn, "flair airlines", "one_off", Some("a trip")).unwrap();
        assert_eq!(annotations(&conn).unwrap().get("flair airlines").unwrap(), "one_off");
        set_annotation(&conn, "flair airlines", "expected", None).unwrap();
        let m = annotations(&conn).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("flair airlines").unwrap(), "expected");
        clear_annotation(&conn, "flair airlines").unwrap();
        assert!(annotations(&conn).unwrap().is_empty());
    }
}

//! Typed key/value reader+writer over the `settings` table.
//! Values are JSON-encoded so callers can store booleans, structs, or strings uniformly.

use crate::error::{CoreError, CoreResult};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};

/// Read a setting by key, returning None if absent.
pub fn get<T: DeserializeOwned>(conn: &Connection, key: &str) -> CoreResult<Option<T>> {
    let row: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    match row {
        None => Ok(None),
        Some(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| CoreError::InvalidState(format!("settings[{key}] parse: {e}"))),
    }
}

/// Insert or replace a setting. Value is JSON-encoded.
pub fn set<T: Serialize>(conn: &Connection, key: &str, value: &T) -> CoreResult<()> {
    let json = serde_json::to_string(value)
        .map_err(|e| CoreError::InvalidState(format!("settings[{key}] encode: {e}")))?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}

/// Delete a setting. No-op if absent.
pub fn delete(conn: &Connection, key: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("s.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn get_missing_key_returns_none() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let v: Option<bool> = get(&conn, "nope").unwrap();
        assert_eq!(v, None);
    }

    #[test]
    fn round_trip_bool() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        set(&conn, "onboarding_completion_marked", &true).unwrap();
        let got: Option<bool> = get(&conn, "onboarding_completion_marked").unwrap();
        assert_eq!(got, Some(true));
    }

    #[test]
    fn overwrite_existing_value() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        set(&conn, "k", &"a").unwrap();
        set(&conn, "k", &"b").unwrap();
        let got: Option<String> = get(&conn, "k").unwrap();
        assert_eq!(got.as_deref(), Some("b"));
    }

    #[test]
    fn delete_removes_key() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        set(&conn, "k", &42i64).unwrap();
        delete(&conn, "k").unwrap();
        let got: Option<i64> = get(&conn, "k").unwrap();
        assert_eq!(got, None);
    }
}

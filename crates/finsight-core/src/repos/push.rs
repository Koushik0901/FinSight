//! Web Push subscriptions for the installed PWA.
//!
//! A subscription is the browser's promise that a message posted to `endpoint`,
//! encrypted to `p256dh`/`auth`, will reach that device — even with the app
//! closed. See `crates/finsight-api/src/commands/push.rs` for the send side.

use crate::error::CoreResult;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushSubscription {
    pub id: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn row_to_sub(r: &rusqlite::Row<'_>) -> rusqlite::Result<PushSubscription> {
    let created: String = r.get(5)?;
    let last_used: Option<String> = r.get(6)?;
    Ok(PushSubscription {
        id: r.get(0)?,
        endpoint: r.get(1)?,
        p256dh: r.get(2)?,
        auth: r.get(3)?,
        label: r.get(4)?,
        created_at: parse_dt(&created),
        last_used_at: last_used.as_deref().map(parse_dt),
    })
}

const COLUMNS: &str = "id, endpoint, p256dh, auth, label, created_at, last_used_at";

/// Register a device, or refresh the keys of one already registered.
///
/// Upsert on `endpoint` rather than insert: browsers rotate a subscription's
/// keys without changing its endpoint, and they re-subscribe on their own
/// schedule. Inserting blindly would either fail the UNIQUE constraint or pile
/// up stale rows that every future send would try and fail to deliver to.
pub fn upsert(
    conn: &mut Connection,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    label: Option<&str>,
) -> CoreResult<PushSubscription> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO push_subscriptions (id, endpoint, p256dh, auth, label, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(endpoint) DO UPDATE SET \
           p256dh = excluded.p256dh, \
           auth = excluded.auth, \
           label = COALESCE(excluded.label, push_subscriptions.label)",
        params![
            uuid::Uuid::new_v4().to_string(),
            endpoint,
            p256dh,
            auth,
            label,
            now.to_rfc3339(),
        ],
    )?;
    get_by_endpoint(conn, endpoint)?
        .ok_or_else(|| crate::error::CoreError::InvalidState("push subscription vanished".into()))
}

pub fn get_by_endpoint(
    conn: &Connection,
    endpoint: &str,
) -> CoreResult<Option<PushSubscription>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM push_subscriptions WHERE endpoint = ?1"
    ))?;
    let mut rows = stmt.query_map(params![endpoint], row_to_sub)?;
    Ok(match rows.next() {
        Some(r) => Some(r?),
        None => None,
    })
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<PushSubscription>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM push_subscriptions ORDER BY created_at ASC"
    ))?;
    let rows = stmt.query_map([], row_to_sub)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Unregister a device. Returns whether a row was actually removed, so callers
/// pruning after a 410 Gone can tell a real cleanup from a no-op.
pub fn delete_by_endpoint(conn: &mut Connection, endpoint: &str) -> CoreResult<bool> {
    let n = conn.execute(
        "DELETE FROM push_subscriptions WHERE endpoint = ?1",
        params![endpoint],
    )?;
    Ok(n > 0)
}

/// Stamp a successful delivery — the only signal we have that a subscription is
/// still alive, and what a "last reached this device" UI would read.
pub fn mark_used(conn: &mut Connection, endpoint: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE push_subscriptions SET last_used_at = ?2 WHERE endpoint = ?1",
        params![endpoint, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("p.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn upsert_then_list_round_trips() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let sub = upsert(&mut conn, "https://push.example/abc", "key1", "auth1", Some("Pixel")).unwrap();
        assert_eq!(sub.endpoint, "https://push.example/abc");
        assert_eq!(sub.label.as_deref(), Some("Pixel"));
        assert_eq!(list(&mut conn).unwrap().len(), 1);
    }

    /// The behaviour the UNIQUE constraint exists for: a browser rotating its
    /// keys must refresh the row, not create a second one that can never be
    /// delivered to.
    #[test]
    fn resubscribing_same_endpoint_updates_keys_without_duplicating() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        upsert(&mut conn, "https://push.example/abc", "old", "oldauth", Some("Pixel")).unwrap();
        upsert(&mut conn, "https://push.example/abc", "new", "newauth", None).unwrap();

        let all = list(&mut conn).unwrap();
        assert_eq!(all.len(), 1, "same endpoint must not create a second row");
        assert_eq!(all[0].p256dh, "new");
        assert_eq!(all[0].auth, "newauth");
        // A re-subscribe that carries no label must not wipe the existing one.
        assert_eq!(all[0].label.as_deref(), Some("Pixel"));
    }

    #[test]
    fn distinct_endpoints_are_separate_devices() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        upsert(&mut conn, "https://push.example/a", "k", "a", None).unwrap();
        upsert(&mut conn, "https://push.example/b", "k", "a", None).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 2);
    }

    #[test]
    fn delete_reports_whether_it_removed_anything() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        upsert(&mut conn, "https://push.example/a", "k", "a", None).unwrap();
        assert!(delete_by_endpoint(&mut conn, "https://push.example/a").unwrap());
        assert!(!delete_by_endpoint(&mut conn, "https://push.example/a").unwrap());
        assert!(list(&mut conn).unwrap().is_empty());
    }

    #[test]
    fn mark_used_stamps_last_used_at() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        upsert(&mut conn, "https://push.example/a", "k", "a", None).unwrap();
        assert!(list(&mut conn).unwrap()[0].last_used_at.is_none());
        mark_used(&mut conn, "https://push.example/a").unwrap();
        assert!(list(&mut conn).unwrap()[0].last_used_at.is_some());
    }
}

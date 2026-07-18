//! Opaque-token session store. The UNWRAPPED per-user DB key lives here, in
//! memory only, for the life of the session (spec: background work possible
//! only while a session holds the key). Sessions do not survive restarts.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub const SESSION_COOKIE: &str = "finsight_session";
const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600); // 30d sliding

pub struct SessionEntry {
    pub user_id: String,
    /// 64-hex SQLCipher key, unwrapped at login. `Zeroizing` so this in-memory
    /// copy is wiped the moment the entry is dropped (session removed,
    /// expired, or the store itself is torn down).
    pub db_key_hex: Zeroizing<String>,
    pub is_admin: bool,
    pub expires: Instant,
}

#[derive(Default)]
pub struct SessionStore(Mutex<HashMap<String, SessionEntry>>);

impl SessionStore {
    /// Create a session with an explicit TTL. Exists mainly so the default
    /// 30-day TTL in `create` is not hardwired into the token-generation path
    /// — tests exercise expiry by constructing entries directly (see below),
    /// but a short/zero TTL here would work too if ever needed.
    pub fn create_with_ttl(
        &self,
        user_id: &str,
        db_key_hex: String,
        is_admin: bool,
        ttl: Duration,
    ) -> String {
        let mut tok = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut tok);
        let token = hex::encode(tok);
        self.0.lock().unwrap().insert(
            token.clone(),
            SessionEntry {
                user_id: user_id.to_string(),
                db_key_hex: Zeroizing::new(db_key_hex),
                is_admin,
                expires: Instant::now() + ttl,
            },
        );
        token
    }

    pub fn create(&self, user_id: &str, db_key_hex: String, is_admin: bool) -> String {
        self.create_with_ttl(user_id, db_key_hex, is_admin, SESSION_TTL)
    }

    /// Sliding expiry: touch on every successful lookup. Returns a fresh
    /// `Zeroizing` clone of the key so the caller's copy also wipes on drop;
    /// callers that need a plain `&str` (e.g. `Db::open`) can deref it.
    pub fn get(&self, token: &str) -> Option<(String, Zeroizing<String>, bool)> {
        let mut map = self.0.lock().unwrap();
        let now = Instant::now();
        match map.get_mut(token) {
            Some(entry) if entry.expires > now => {
                entry.expires = now + SESSION_TTL;
                Some((
                    entry.user_id.clone(),
                    Zeroizing::new(entry.db_key_hex.to_string()),
                    entry.is_admin,
                ))
            }
            Some(_) => {
                // Expired: purge it so it doesn't linger in the map forever.
                map.remove(token);
                None
            }
            None => None,
        }
    }

    pub fn remove(&self, token: &str) {
        self.0.lock().unwrap().remove(token);
    }

    /// Drop every session for a user (admin deletion).
    pub fn remove_user(&self, user_id: &str) {
        self.0.lock().unwrap().retain(|_, e| e.user_id != user_id);
    }

    /// Does this user still hold at least one live session? Used by `logout` to
    /// decide whether the user's runtime may be torn down: the DB key must not
    /// outlive the last session that holds it, but a sign-out on ONE device
    /// must not evict a runtime another device is still using.
    pub fn has_user_sessions(&self, user_id: &str) -> bool {
        self.0.lock().unwrap().values().any(|e| e.user_id == user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_then_get_round_trip() {
        let store = SessionStore::default();
        let token = store.create("user-1", "deadbeef".repeat(8), true);
        let (user_id, db_key_hex, is_admin) = store.get(&token).unwrap();
        assert_eq!(user_id, "user-1");
        assert_eq!(db_key_hex.as_str(), "deadbeef".repeat(8));
        assert!(is_admin);
    }

    #[test]
    fn get_after_remove_is_none() {
        let store = SessionStore::default();
        let token = store.create("user-1", "k".repeat(64), false);
        store.remove(&token);
        assert!(store.get(&token).is_none());
    }

    #[test]
    fn remove_user_clears_all_of_their_sessions() {
        let store = SessionStore::default();
        let t1 = store.create("user-1", "k".repeat(64), false);
        let t2 = store.create("user-1", "k".repeat(64), false);
        let t3 = store.create("user-2", "k".repeat(64), false);

        store.remove_user("user-1");

        assert!(store.get(&t1).is_none());
        assert!(store.get(&t2).is_none());
        assert!(store.get(&t3).is_some());
    }

    #[test]
    fn expired_entry_returns_none_and_is_purged() {
        let store = SessionStore::default();
        let token = "expired-token".to_string();
        // Insert directly with an expiry already in the past — deterministic,
        // no reliance on real time elapsing between create() and get().
        store.0.lock().unwrap().insert(
            token.clone(),
            SessionEntry {
                user_id: "user-1".to_string(),
                db_key_hex: Zeroizing::new("k".repeat(64)),
                is_admin: false,
                expires: Instant::now() - Duration::from_secs(1),
            },
        );

        assert!(store.get(&token).is_none());
        // And it was purged, not just skipped.
        assert!(!store.0.lock().unwrap().contains_key(&token));
    }
}

//! Opaque-token session store. The UNWRAPPED per-user DB key lives here, in
//! memory, for the life of the session (spec: background work possible only
//! while a session holds the key).
//!
//! With a persistence backend attached ([`SessionStore::with_persistence`]),
//! each session is ALSO mirrored to `users.db` with its DB key wrapped under the
//! server master key, so a restart no longer forces every user to sign in
//! again. The unwrapped key still lives only in memory at runtime — on disk it
//! exists solely in server-key-wrapped form. See
//! `crypto::load_or_create_server_key` for the security tradeoff this accepts.

use crate::users::UsersDb;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub const SESSION_COOKIE: &str = "finsight_session";
const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600); // 30d sliding
const SESSION_TTL_SECS: i64 = 30 * 24 * 3600;

pub struct SessionEntry {
    pub user_id: String,
    /// 64-hex SQLCipher key, unwrapped at login. `Zeroizing` so this in-memory
    /// copy is wiped the moment the entry is dropped (session removed,
    /// expired, or the store itself is torn down).
    pub db_key_hex: Zeroizing<String>,
    pub is_admin: bool,
    pub expires: Instant,
}

/// Persistence backend for sessions: a shared handle to `users.db` plus the
/// server master key that wraps each session's DB key on disk. Absent in the
/// `Default` store (in-memory only — the shape the unit tests exercise).
struct SessionPersist {
    users: Arc<UsersDb>,
    server_key: [u8; 32],
}

#[derive(Default)]
pub struct SessionStore {
    map: Mutex<HashMap<String, SessionEntry>>,
    persist: Option<SessionPersist>,
}

impl SessionStore {
    /// A store that also mirrors sessions to `users.db` so they survive a
    /// restart. `server_key` wraps each session's DB key at rest.
    pub fn with_persistence(users: Arc<UsersDb>, server_key: [u8; 32]) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            persist: Some(SessionPersist { users, server_key }),
        }
    }

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
        // Mirror to disk first (best-effort — a persistence hiccup must not
        // fail the login; the in-memory session still works this run).
        self.persist_row(&token, user_id, &db_key_hex, is_admin, ttl);
        self.map.lock().unwrap().insert(
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
    ///
    /// On an in-memory miss with persistence attached, this is also the
    /// restart-recovery path: the persisted row is re-hydrated (its DB key
    /// unwrapped with the server master key) so the first request after a
    /// restart re-authenticates transparently, no re-login.
    pub fn get(&self, token: &str) -> Option<(String, Zeroizing<String>, bool)> {
        {
            let mut map = self.map.lock().unwrap();
            let now = Instant::now();
            match map.get_mut(token) {
                Some(entry) if entry.expires > now => {
                    entry.expires = now + SESSION_TTL;
                    return Some((
                        entry.user_id.clone(),
                        Zeroizing::new(entry.db_key_hex.to_string()),
                        entry.is_admin,
                    ));
                }
                Some(_) => {
                    // Expired in memory: purge it. Don't fall through to disk —
                    // a persisted row for the same token is no newer.
                    map.remove(token);
                    return None;
                }
                None => {} // fall through to the persistent-recovery path
            }
        } // map lock released before touching the DB
        self.recover_from_disk(token)
    }

    /// Rehydrate a persisted session after a restart. Returns `None` when there
    /// is no persistence backend, no row, an expired row, or the wrapped key
    /// fails to unwrap (wrong/rotated server key).
    fn recover_from_disk(&self, token: &str) -> Option<(String, Zeroizing<String>, bool)> {
        let persist = self.persist.as_ref()?;
        let token_hash = crate::crypto::hash_session_token(token);
        let now_unix = chrono::Utc::now().timestamp();
        let row = persist.users.recover_session(&token_hash, now_unix).ok()??;
        let dbkey =
            crate::crypto::unwrap_key_with_server_key(&persist.server_key, &row.wrapped_db_key)
                .ok()?;
        let db_key_hex = crate::crypto::db_key_to_hex(&dbkey);
        // Slide the persisted expiry forward so an actively-used session doesn't
        // hard-expire 30 days after it was first created, only after last use.
        let _ = persist
            .users
            .slide_session_expiry(&token_hash, now_unix + SESSION_TTL_SECS);
        self.map.lock().unwrap().insert(
            token.to_string(),
            SessionEntry {
                user_id: row.user_id.clone(),
                db_key_hex: Zeroizing::new(db_key_hex.clone()),
                is_admin: row.is_admin,
                expires: Instant::now() + SESSION_TTL,
            },
        );
        Some((row.user_id, Zeroizing::new(db_key_hex), row.is_admin))
    }

    /// Write the on-disk mirror of a session. No-op without persistence. Wrap
    /// failures are logged, never fatal — a session that can't be persisted
    /// simply won't survive the next restart.
    fn persist_row(&self, token: &str, user_id: &str, db_key_hex: &str, is_admin: bool, ttl: Duration) {
        let Some(persist) = self.persist.as_ref() else {
            return;
        };
        let dbkey: Option<[u8; 32]> = hex::decode(db_key_hex)
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok());
        let Some(dbkey) = dbkey else {
            tracing::warn!("session db key was not 32 bytes of hex; not persisting");
            return;
        };
        let Ok(wrapped) = crate::crypto::wrap_key_with_server_key(&persist.server_key, &dbkey) else {
            tracing::warn!("failed to wrap session key; session won't survive restart");
            return;
        };
        let token_hash = crate::crypto::hash_session_token(token);
        let expires_unix = chrono::Utc::now().timestamp() + ttl.as_secs() as i64;
        if let Err(e) =
            persist
                .users
                .persist_session(&token_hash, user_id, is_admin, &wrapped, expires_unix)
        {
            tracing::warn!("failed to persist session (won't survive restart): {e}");
        }
    }

    pub fn remove(&self, token: &str) {
        self.map.lock().unwrap().remove(token);
        // Purge the on-disk mirror too, or a signed-out session resurrects on
        // the next restart. This is a security invariant, not an optimization.
        if let Some(persist) = self.persist.as_ref() {
            let token_hash = crate::crypto::hash_session_token(token);
            let _ = persist.users.delete_session(&token_hash);
        }
    }

    /// Drop every session for a user (admin deletion), in memory and on disk.
    pub fn remove_user(&self, user_id: &str) {
        self.map.lock().unwrap().retain(|_, e| e.user_id != user_id);
        if let Some(persist) = self.persist.as_ref() {
            let _ = persist.users.delete_user_sessions(user_id);
        }
    }

    /// Sign out every OTHER session for a user, keeping `keep_token` alive
    /// (the "sign out other devices" control). Purges the on-disk mirror too,
    /// so signed-out devices don't come back on a restart. Returns the count
    /// removed — from the persisted rows when persistence is on (which also
    /// covers devices not currently resident in memory), else the in-memory
    /// count.
    pub fn remove_user_except(&self, user_id: &str, keep_token: &str) -> usize {
        let mem_removed = {
            let mut map = self.map.lock().unwrap();
            let before = map.len();
            map.retain(|tok, e| e.user_id != user_id || tok == keep_token);
            before - map.len()
        };
        if let Some(persist) = self.persist.as_ref() {
            let keep_hash = crate::crypto::hash_session_token(keep_token);
            return persist
                .users
                .delete_user_sessions_except(user_id, &keep_hash)
                .unwrap_or(mem_removed);
        }
        mem_removed
    }

    /// Drop every already-expired persisted session. Called once at startup so
    /// dead rows don't accumulate across restarts. No-op without persistence.
    pub fn purge_expired_persisted(&self) {
        if let Some(persist) = self.persist.as_ref() {
            let now_unix = chrono::Utc::now().timestamp();
            match persist.users.purge_expired_sessions(now_unix) {
                Ok(n) if n > 0 => tracing::info!("purged {n} expired persisted session(s)"),
                Ok(_) => {}
                Err(e) => tracing::warn!("failed to purge expired sessions: {e}"),
            }
        }
    }

    /// Does this user still hold at least one live session? Used by `logout` to
    /// decide whether the user's runtime may be torn down: the DB key must not
    /// outlive the last session that holds it, but a sign-out on ONE device
    /// must not evict a runtime another device is still using.
    pub fn has_user_sessions(&self, user_id: &str) -> bool {
        self.map.lock().unwrap().values().any(|e| e.user_id == user_id)
    }
}

// -------------------------------------------------------- login throttle ---

/// Consecutive failures tolerated before a username is locked out.
pub const MAX_FAILURES: u32 = 5;
/// How long a locked-out username stays locked.
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Default)]
struct FailureEntry {
    consecutive: u32,
    locked_until: Option<Instant>,
}

/// In-memory, per-username credential-failure tracker guarding `login` and
/// `recover`. Same shape as `SessionStore`: a `Mutex<HashMap<..>>` hung off
/// `ServerState`, so it is shared across requests and lost on restart (a
/// restart is operator action, not an attacker primitive).
///
/// Keyed on the SUBMITTED username, lowercased, whether or not that user
/// exists — an existence-dependent lockout would itself be a username oracle,
/// which is exactly what `bad_credentials` exists to avoid. Consequence worth
/// naming: this is a lockout, so an attacker who knows a username can deny
/// that user logins for the cooldown window. Acceptable for a single-tenant
/// self-host; the alternative (IP-keyed) is meaningless behind the reverse
/// proxies this deploys under.
pub struct LoginThrottle {
    entries: Mutex<HashMap<String, FailureEntry>>,
    cooldown: Duration,
}

impl Default for LoginThrottle {
    fn default() -> Self {
        Self::new(DEFAULT_COOLDOWN)
    }
}

impl LoginThrottle {
    pub fn new(cooldown: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            cooldown,
        }
    }

    fn key(username: &str) -> String {
        // usernames collate NOCASE in users.db — match that, or `Alice` and
        // `alice` would get independent failure budgets for one account.
        username.trim().to_lowercase()
    }

    /// `true` when this username is inside its cooldown and must be rejected
    /// with 429 BEFORE any DB lookup or Argon2 work (shedding that load under
    /// attack is the point). An elapsed lock is cleared here, so the very next
    /// attempt proceeds normally with a fresh budget.
    pub fn is_locked(&self, username: &str) -> bool {
        let mut map = self.entries.lock().unwrap();
        let key = Self::key(username);
        let Some(entry) = map.get_mut(&key) else {
            return false;
        };
        match entry.locked_until {
            Some(until) if Instant::now() < until => true,
            Some(_) => {
                // Window elapsed: forget the whole entry so the attempt that
                // follows starts from zero failures.
                map.remove(&key);
                false
            }
            None => false,
        }
    }

    /// Count one failed credential check. Trips the lock on the Nth consecutive
    /// failure. Call on EVERY failure path — including unknown-username — so
    /// the throttle can't be probed for account existence.
    pub fn record_failure(&self, username: &str) {
        let mut map = self.entries.lock().unwrap();
        let entry = map.entry(Self::key(username)).or_default();
        entry.consecutive += 1;
        if entry.consecutive >= MAX_FAILURES {
            entry.locked_until = Some(Instant::now() + self.cooldown);
        }
    }

    /// Successful auth wipes the budget.
    pub fn record_success(&self, username: &str) {
        self.entries.lock().unwrap().remove(&Self::key(username));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_locks_after_max_failures_and_expires() {
        let t = LoginThrottle::new(Duration::from_millis(120));

        for _ in 0..MAX_FAILURES - 1 {
            t.record_failure("alice");
            assert!(!t.is_locked("alice"), "under the limit must stay open");
        }
        t.record_failure("alice");
        assert!(t.is_locked("alice"), "the Nth failure locks");

        std::thread::sleep(Duration::from_millis(200));
        assert!(!t.is_locked("alice"), "the lock lifts once the window elapses");
        // ...and the budget reset, so one more failure doesn't re-lock instantly.
        t.record_failure("alice");
        assert!(!t.is_locked("alice"));
    }

    #[test]
    fn throttle_success_clears_the_budget() {
        let t = LoginThrottle::default();
        for _ in 0..MAX_FAILURES - 1 {
            t.record_failure("alice");
        }
        t.record_success("alice");
        // Budget was wiped: a full fresh run of failures is needed to lock.
        for _ in 0..MAX_FAILURES - 1 {
            t.record_failure("alice");
            assert!(!t.is_locked("alice"));
        }
    }

    #[test]
    fn throttle_is_per_username_and_case_insensitive() {
        let t = LoginThrottle::default();
        for _ in 0..MAX_FAILURES {
            t.record_failure("Alice");
        }
        // usernames collate NOCASE, so the lock must follow the same rule.
        assert!(t.is_locked("alice"));
        assert!(t.is_locked("ALICE"));
        // A different account is unaffected.
        assert!(!t.is_locked("bob"));
    }

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
        store.map.lock().unwrap().insert(
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
        assert!(!store.map.lock().unwrap().contains_key(&token));
    }

    // ------------------------------------------------ persistent sessions ---

    use std::sync::Arc;

    fn persistent_store(dir: &std::path::Path) -> (SessionStore, Arc<UsersDb>, [u8; 32]) {
        let users = Arc::new(UsersDb::open(&dir.join("users.db")).unwrap());
        let smk = crate::crypto::generate_db_key();
        (SessionStore::with_persistence(Arc::clone(&users), smk), users, smk)
    }

    /// A fresh `SessionStore` over the SAME users.db + server key simulates a
    /// server restart: the in-memory map starts empty, so any session that comes
    /// back MUST have been recovered from disk.
    fn restart(users: &Arc<UsersDb>, smk: [u8; 32]) -> SessionStore {
        SessionStore::with_persistence(Arc::clone(users), smk)
    }

    #[test]
    fn persisted_session_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let (store, users, smk) = persistent_store(dir.path());
        let key = "ab".repeat(32); // 64 hex chars
        let token = store.create("user-1", key.clone(), true);

        let after = restart(&users, smk);
        let (uid, recovered_key, is_admin) = after.get(&token).expect("session must survive restart");
        assert_eq!(uid, "user-1");
        assert_eq!(recovered_key.as_str(), key, "the exact DB key must round-trip through the server-key wrap");
        assert!(is_admin);
    }

    #[test]
    fn removed_session_does_not_resurrect_on_restart() {
        // The load-bearing security test: logout must purge the on-disk row, or
        // a signed-out session comes back to life after a restart.
        let dir = tempfile::tempdir().unwrap();
        let (store, users, smk) = persistent_store(dir.path());
        let token = store.create("user-1", "cd".repeat(32), false);
        store.remove(&token);

        let after = restart(&users, smk);
        assert!(after.get(&token).is_none(), "a removed session must NOT resurrect");
    }

    #[test]
    fn deleting_a_user_purges_their_persisted_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let (store, users, smk) = persistent_store(dir.path());
        let t1 = store.create("user-1", "11".repeat(32), false);
        let t2 = store.create("user-2", "22".repeat(32), false);
        store.remove_user("user-1");

        let after = restart(&users, smk);
        assert!(after.get(&t1).is_none(), "deleted user's session must not survive");
        assert!(after.get(&t2).is_some(), "other users' sessions are untouched");
    }

    #[test]
    fn a_different_server_key_cannot_recover_a_session() {
        // If the server master key is rotated/lost, persisted sessions become
        // unreadable (fail closed) rather than leaking or panicking.
        let dir = tempfile::tempdir().unwrap();
        let (store, users, _smk) = persistent_store(dir.path());
        let token = store.create("user-1", "ef".repeat(32), false);

        let wrong_key = crate::crypto::generate_db_key();
        let after = SessionStore::with_persistence(Arc::clone(&users), wrong_key);
        assert!(after.get(&token).is_none(), "wrong server key must fail closed");
    }

    #[test]
    fn sign_out_others_keeps_current_and_purges_the_rest_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let (store, users, smk) = persistent_store(dir.path());
        let keep = store.create("user-1", "11".repeat(32), false);
        let other1 = store.create("user-1", "11".repeat(32), false);
        let other2 = store.create("user-1", "11".repeat(32), false);
        let elsewhere = store.create("user-2", "22".repeat(32), false);

        let removed = store.remove_user_except("user-1", &keep);
        assert_eq!(removed, 2, "both of user-1's OTHER sessions are removed");

        // The kept session survives a restart; the others don't; another user
        // is untouched.
        let after = restart(&users, smk);
        assert!(after.get(&keep).is_some(), "current device must stay signed in");
        assert!(after.get(&other1).is_none());
        assert!(after.get(&other2).is_none());
        assert!(after.get(&elsewhere).is_some(), "a different user is unaffected");
    }

    #[test]
    fn expired_persisted_session_is_not_recovered_and_purge_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        let (store, users, smk) = persistent_store(dir.path());
        // Zero TTL => the row is already expired the instant it's written.
        let token = store.create_with_ttl("user-1", "ab".repeat(32), false, Duration::ZERO);

        let after = restart(&users, smk);
        assert!(after.get(&token).is_none(), "an expired row must never re-authenticate");
        after.purge_expired_persisted(); // must not panic; clears the dead row
    }

    #[test]
    fn default_store_has_no_persistence_and_stays_in_memory() {
        // The in-memory-only shape the rest of the suite relies on.
        let store = SessionStore::default();
        let token = store.create("user-1", "ab".repeat(32), false);
        assert!(store.get(&token).is_some());
        // A "restart" (fresh default store) shares nothing — no recovery.
        assert!(SessionStore::default().get(&token).is_none());
    }
}

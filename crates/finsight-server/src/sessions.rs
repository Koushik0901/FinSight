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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Lock a mutex, recovering from poisoning instead of panicking.
///
/// A panic in one request thread while holding a server mutex (the users.db
/// connection, the session map, the runtime registry) would otherwise poison
/// it and turn every subsequent login/token/registry operation into a panic
/// loop until restart. The guarded data is kept consistent by transactional
/// discipline in the writers, so a poisoned-but-recovered lock is strictly
/// better than a bricked auth layer.
pub(crate) fn lock_recovered<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// How long an OAuth client registration that no one ever consented to is kept
/// before the startup sweep drops it.
///
/// A month, because the two failure directions are not symmetric. Too short and
/// a real connector registered but not yet approved (a user who starts linking
/// Claude, gets distracted, and comes back next week) silently loses its
/// registration and has to start over. Too long only means junk lingers behind
/// a cap that already bounds it. So err long.
const UNUSED_CLIENT_TTL_DAYS: i64 = 30;

/// One event as the UI's Tauri-event shim expects it: `{ event, payload }`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OutboundEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

/// Phase 2: the server owns only the user registry, the session store, and
/// the lazy per-user runtime registry. There is no single shared `ApiState`
/// or `db.key` any more — each authenticated user gets their own SQLCipher
/// DB (opened lazily by `registry::get_or_bootstrap` with the key unwrapped
/// at login time, see `sessions.rs`).
pub struct ServerState {
    /// Shared so the session store can persist/recover sessions in the same
    /// `users.db` (see `sessions::SessionStore::with_persistence`).
    pub users: Arc<crate::users::UsersDb>,
    pub sessions: crate::sessions::SessionStore,
    /// Per-username failure budget guarding `login` and `recover`. In-memory
    /// and shared across requests — see `sessions::LoginThrottle`.
    pub throttle: crate::sessions::LoginThrottle,
    pub registry: crate::registry::Registry,
    pub data_dir: PathBuf,
    /// Serializes the one-time setup transition. Without this guard, two
    /// concurrent requests can both observe an empty users table, perform the
    /// expensive credential work, and each insert an administrator.
    pub setup_lock: tokio::sync::Mutex<()>,
}

impl ServerState {
    /// Sync: opens `users.db` only (plain SQLite, cheap). Per-user SQLCipher
    /// DBs open lazily on first authenticated request/login — see
    /// `registry::Registry::get_or_bootstrap`. The legacy Phase 1 keyfile
    /// (`<data>/db.key`) is read exactly once more, by the setup handler's
    /// migration path (`auth.rs`), and is NOT touched here.
    pub fn bootstrap(data_dir: &Path) -> anyhow::Result<Arc<Self>> {
        Self::bootstrap_with_throttle(data_dir, crate::sessions::LoginThrottle::default())
    }

    /// `bootstrap` with an explicit lockout policy. The 60-second production
    /// cooldown makes "locks out, then recovers once the window passes"
    /// untestable at the HTTP level without a minute-long sleep, so that
    /// behaviour is exercised through this ctor with a short cooldown.
    pub fn bootstrap_with_throttle(
        data_dir: &Path,
        throttle: crate::sessions::LoginThrottle,
    ) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir)?;
        let users = Arc::new(crate::users::UsersDb::open(&data_dir.join("users.db"))?);
        // Persistent sessions: the server master key wraps each session's DB key
        // on disk so a restart no longer forces every user to sign in again.
        // Supplied via FINSIGHT_SESSION_KEY, else generated into session.key.
        let server_key = crate::crypto::load_or_create_server_key(&data_dir.join("session.key"))?;
        let sessions =
            crate::sessions::SessionStore::with_persistence(Arc::clone(&users), server_key);
        sessions.purge_expired_persisted();
        // In-flight OAuth codes live ~10 minutes; anything still on disk at
        // startup is dead by definition. Best-effort — a failure here must not
        // stop the server from booting.
        match users.purge_expired_oauth_codes(chrono::Utc::now().timestamp()) {
            Ok(n) if n > 0 => tracing::info!(count = n, "purged expired OAuth authorization codes"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not purge expired OAuth codes"),
        }
        // Expired access tokens are already refused at auth time; this only
        // keeps the table from growing without bound as connectors refresh.
        match users.purge_expired_api_tokens(chrono::Utc::now().timestamp()) {
            Ok(n) if n > 0 => tracing::info!(count = n, "purged expired access tokens"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not purge expired access tokens"),
        }
        // `POST /api/oauth/register` is unauthenticated by spec, so on an
        // internet-reachable instance anyone can fill `oauth_clients` to its
        // MAX_REGISTERED_CLIENTS ceiling and block every NEW connector. Without
        // this sweep the only recovery was to open users.db in a SQLite client
        // and delete rows by hand.
        //
        // Only registrations no one ever consented to are dropped, and only
        // after they have sat unused for a month — long enough that a client
        // registered legitimately and approved later is never caught, while
        // junk still cannot accumulate indefinitely.
        let stale_before =
            (chrono::Utc::now() - chrono::Duration::days(UNUSED_CLIENT_TTL_DAYS)).to_rfc3339();
        match users.prune_unused_oauth_clients(&stale_before) {
            Ok(n) if n > 0 => {
                tracing::info!(
                    count = n,
                    "pruned OAuth client registrations that were never used"
                )
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not prune unused OAuth clients"),
        }
        Ok(Arc::new(Self {
            users,
            sessions,
            throttle,
            registry: crate::registry::Registry::default(),
            data_dir: data_dir.to_path_buf(),
            setup_lock: tokio::sync::Mutex::new(()),
        }))
    }
}

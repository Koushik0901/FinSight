//! Lazy per-user runtimes. Each logged-in user gets: their own SQLCipher pool
//! (ApiState), their own event broadcast (SSE), their own agent thread — built
//! on first authenticated request, evicted after idle timeout (pools dropped;
//! the session still holds the unwrapped key, so the next request rebuilds).

use crate::state::OutboundEvent;
use finsight_agent::agent::{AgentEvent, EventCallback};
use finsight_api::ApiState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

pub struct UserRuntime {
    pub api: Arc<ApiState>,
    pub events: broadcast::Sender<OutboundEvent>,
    pub last_active: Mutex<Instant>,
    /// Handle for this runtime's background sync loop; aborted on eviction
    /// and, as a backstop, whenever the runtime itself is dropped (see `Drop`).
    pub sync_task: tokio::task::JoinHandle<()>,
}

/// Dropping a `JoinHandle` DETACHES its task rather than cancelling it, so a
/// runtime that dies without going through `evict` would leave its sync loop
/// running — holding a `Db` clone — until the process exits. That is reachable:
/// `evict` (logout, user deletion) can remove a still-initializing cell while a
/// bootstrap is in flight, and the resulting runtime is never in the map for
/// `evict` to abort. Tying cancellation to the runtime's lifetime closes that
/// race for every path, present and future. `evict` still aborts explicitly so
/// eviction is prompt rather than waiting on the last `Arc` to drop.
impl Drop for UserRuntime {
    fn drop(&mut self) {
        self.sync_task.abort();
    }
}

/// Map value: a lazily-initialized cell, NOT a built runtime. Concurrent
/// callers that miss share one cell and join a single bootstrap (see
/// `get_or_bootstrap`), so an uninitialized cell is a normal, observable state
/// — `touch`/`evict`/`evict_idle` must all tolerate `cell.get() == None`.
type RuntimeCell = Arc<tokio::sync::OnceCell<Arc<UserRuntime>>>;

#[derive(Default)]
pub struct Registry(Mutex<HashMap<String, RuntimeCell>>);

pub fn user_data_dir(data_dir: &Path, user_id: &str) -> PathBuf {
    data_dir.join("users").join(user_id)
}

/// Apply a staged restore (P0-4) BEFORE opening the DB, so we never swap a
/// database that has live connections. Moves `data.pending-restore.sqlcipher`
/// over `data.sqlcipher` and drops the stale WAL/SHM so the restored snapshot
/// is authoritative.
///
/// This is the server-side half of Settings → Restore from backup:
/// `stage_restore_backup` only writes the pending file, and something must
/// swap it in before the next `Db::open` or the restore is a silent no-op
/// while `get_data_health` reports `pendingRestore: true` forever. On the
/// desktop that caller was `configure_app`; here it is the per-user bootstrap,
/// and "restart FinSight" becomes "this user's runtime is (re)built".
pub(crate) fn apply_staged_restore(user_dir: &Path) {
    let pending_restore = user_dir.join("data.pending-restore.sqlcipher");
    if !pending_restore.exists() {
        return;
    }
    let db_path = user_dir.join("data.sqlcipher");
    let _ = std::fs::remove_file(user_dir.join("data.sqlcipher-wal"));
    let _ = std::fs::remove_file(user_dir.join("data.sqlcipher-shm"));

    // Retry briefly. Unlike the desktop app — where this ran in a fresh
    // process with nothing holding the file — a server-side restore swaps the
    // DB of a runtime that was just evicted, and eviction is not synchronous:
    // the agent runs on its own OS thread holding a `Db` clone and only winds
    // down once its channel sender drops. On Windows `rename` fails outright
    // while any handle to the destination is open, so an evict-then-rebuild
    // would silently no-op the restore — exactly the bug this function exists
    // to fix. A second is far more than the wind-down needs; on Unix the
    // first attempt always succeeds.
    let mut last_err = None;
    for attempt in 0..10 {
        match std::fs::rename(&pending_restore, &db_path) {
            Ok(()) => {
                tracing::info!("applied staged restore over the live database");
                return;
            }
            Err(e) => {
                last_err = Some(e);
                if attempt < 9 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
    // Loud on purpose: the staged file survives, so `get_data_health` keeps
    // reporting `pendingRestore: true` and the next bootstrap tries again.
    tracing::error!(
        "failed to apply staged restore (database still in use?): {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    );
}

impl Registry {
    /// Get or lazily bootstrap the runtime for `user_id`. Mirrors Phase 1's
    /// `ServerState::bootstrap` but per-user: apply any staged restore, open
    /// Db with `db_key_hex`, run the startup cascade (which owns the
    /// pre-migration backup, migrations and the provider-settings migration),
    /// wire AgentEvent→broadcast (the same names `configure_app`/Phase 1's
    /// `state.rs` use), `ApiState::new`, load+set provider.
    ///
    /// **Single-flight.** A cold page load fires ~14 queries before the first
    /// insert lands, so without a guard ~14 callers each miss the map and
    /// independently open + migrate the SAME SQLite file, throwing 13 results
    /// away; concurrent `run_migrations` on one file is a correctness risk,
    /// not merely waste. So the map holds a `OnceCell` per user rather than a
    /// built runtime: racing callers insert/find the same cell and all join
    /// ONE `get_or_init`, and the losers wait on the winner instead of
    /// duplicating its work. (This also retires the old "loser aborts its own
    /// orphaned sync_task" dance — only the winner ever starts one.)
    ///
    /// **Locking shape.** The map lock is held ONLY for the cell lookup /
    /// insert and is dropped before the `.await` — the bootstrap itself does
    /// real blocking I/O and must not block other users' lookups, and holding
    /// a `std::sync::MutexGuard` across an await would make this future
    /// `!Send` (see `_assert_send` below).
    ///
    /// A failed bootstrap leaves the cell uninitialized, so the next request
    /// retries rather than caching the failure.
    pub async fn get_or_bootstrap(
        &self,
        data_dir: &Path,
        user_id: &str,
        db_key_hex: &str,
    ) -> anyhow::Result<Arc<UserRuntime>> {
        // --- Lock held for the cell lookup/insert ONLY, never across await ---
        let cell: RuntimeCell = {
            let mut map = self.0.lock().unwrap();
            Arc::clone(map.entry(user_id.to_string()).or_default())
        };

        let runtime = cell
            .get_or_try_init(|| self.bootstrap(data_dir, user_id, db_key_hex))
            .await?;
        Ok(Arc::clone(runtime))
    }

    /// The actual per-user bootstrap. Runs exactly once per cell (see
    /// `get_or_bootstrap`); never call it directly.
    async fn bootstrap(
        &self,
        data_dir: &Path,
        user_id: &str,
        db_key_hex: &str,
    ) -> anyhow::Result<Arc<UserRuntime>> {
        let user_dir = user_data_dir(data_dir, user_id);

        // Directory creation, the staged-restore swap, `Db::open` and the
        // login catch-up cascade (integrity check, pre-migration backup,
        // migrations, provider migration, categorization/transfer-pairing/
        // balances/net-worth/anomaly recompute) are all blocking I/O, so they
        // run on a blocking thread rather than stalling an async worker.
        //
        // The cascade OWNS migrations and must therefore run before anything
        // else touches the schema: its pre-migration backup is gated on
        // `pending_migration_count() > 0`, so migrating here first would read
        // 0 and silently skip the snapshot that makes a data-corrupting
        // migration recoverable. Same order as the desktop `configure_app`.
        let dir = user_dir.clone();
        let key = db_key_hex.to_string();
        let (db, report) = tokio::task::spawn_blocking(
            move || -> anyhow::Result<(finsight_core::Db, finsight_api::startup::StartupReport)> {
                std::fs::create_dir_all(&dir)?;
                apply_staged_restore(&dir);
                let db = finsight_core::Db::open(&dir.join("data.sqlcipher"), &key)?;
                let report = finsight_api::startup::run_startup_cascade(&db, &dir.join("backups"));
                Ok((db, report))
            },
        )
        .await??;

        // Migrations are the one cascade step that is NOT best-effort here: a
        // schema that does not match the code must not be served. Failing this
        // one user's bootstrap yields a 500 for them and leaves the process
        // (and every other user) up — and the pre-migration backup taken just
        // above is still on disk to restore from.
        if let Some(e) = &report.migration_error {
            anyhow::bail!("migrations failed for user {user_id}: {e}");
        }
        if !report.warnings.is_empty() {
            tracing::warn!(
                user_id = %user_id,
                warnings = ?report.warnings,
                "login catch-up cascade reported warnings"
            );
        }

        let (tx, _) = broadcast::channel::<OutboundEvent>(256);
        let etx = tx.clone();
        let on_event: EventCallback = Arc::new(move |event: AgentEvent| {
            // Same names Phase 1's state.rs bootstrap uses, so the UI event
            // listeners work unchanged.
            let name = match &event {
                AgentEvent::CategorizationProgress { .. } => "categorization.progress",
                AgentEvent::CategorizationComplete { .. } => "categorization.complete",
                AgentEvent::Error { .. } => "agent.error",
            };
            let _ = etx.send(OutboundEvent {
                event: name.to_string(),
                payload: serde_json::to_value(&event).unwrap_or_default(),
            });
        });

        let api = Arc::new(ApiState::new(db.clone(), user_dir.clone(), on_event));
        if let Some(p) = finsight_api::provider::load_provider_from_settings(&db) {
            api.agent.set_provider(p);
        }
        let _ = api
            .agent
            .tx
            .send(finsight_agent::agent::AgentJob::CheckDueRecipes)
            .await;

        // Start this user's background SimpleFin sync loop on the current
        // Tokio runtime (we're inside an axum handler, so a runtime is
        // always entered here — unlike the desktop `.setup()` closure).
        let sync_task = api.sync_scheduler.start(&tokio::runtime::Handle::current());

        Ok(Arc::new(UserRuntime {
            api,
            events: tx,
            last_active: Mutex::new(Instant::now()),
            sync_task,
        }))
    }

    pub fn touch(&self, user_id: &str) {
        if let Some(rt) = self.0.lock().unwrap().get(user_id).and_then(|c| c.get()) {
            *rt.last_active.lock().unwrap() = Instant::now();
        }
    }

    /// Removes the runtime and aborts its background sync task.
    pub fn evict(&self, user_id: &str) {
        if let Some(cell) = self.0.lock().unwrap().remove(user_id) {
            if let Some(rt) = cell.get() {
                rt.sync_task.abort();
            }
        }
    }

    /// Called by a background interval task: evict runtimes idle > `max_idle`.
    /// Returns the evicted user ids for logging.
    ///
    /// A runtime with live SSE subscribers is NEVER idle, regardless of its
    /// `last_active`: `touch()` only fires on SSE connect and on RPC dispatch,
    /// so a backgrounded PWA tab holding an open EventSource (TanStack pauses
    /// polling while hidden) would otherwise be evicted on the dot every 30
    /// minutes — killing the stream, triggering an EventSource reconnect and
    /// replaying the whole cascade, forever.
    pub fn evict_idle(&self, max_idle: Duration) -> Vec<String> {
        let now = Instant::now();
        let idle_ids: Vec<String> = {
            let map = self.0.lock().unwrap();
            map.iter()
                .filter(|(_, cell)| match cell.get() {
                    // Still bootstrapping — not idle, and evicting the cell
                    // mid-flight would strand the in-flight callers' result.
                    None => false,
                    Some(rt) => {
                        rt.events.receiver_count() == 0
                            && now.duration_since(*rt.last_active.lock().unwrap()) >= max_idle
                    }
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in &idle_ids {
            self.evict(id);
        }
        idle_ids
    }
}

/// Compile-time guard: `get_or_bootstrap`'s returned future must be `Send` —
/// axum handlers require it. A `std::sync::MutexGuard` (from `self.0.lock()`)
/// held across the `spawn_blocking().await` inside the bootstrap path would
/// make this future `!Send` and break the build at the router, not here;
/// this function fails to compile instead, right next to the code it guards.
#[allow(dead_code)]
fn _assert_send() {
    fn assert_send<T: Send>(_: T) {}
    assert_send(async {
        let registry = Registry::default();
        let _ = registry
            .get_or_bootstrap(std::path::Path::new("."), "user", "key")
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> String {
        finsight_core::keychain::generate_random_key().to_string()
    }

    #[test]
    fn user_data_dir_shape() {
        let base = Path::new("/data");
        assert_eq!(
            user_data_dir(base, "user-123"),
            base.join("users").join("user-123")
        );
    }

    #[tokio::test]
    async fn get_or_bootstrap_returns_same_arc_on_second_call() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let key = test_key();

        let rt1 = registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();
        let rt2 = registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();

        assert!(Arc::ptr_eq(&rt1, &rt2), "second call must not double-bootstrap");
    }

    #[tokio::test]
    async fn evict_then_get_or_bootstrap_builds_a_new_arc() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let key = test_key();

        let rt1 = registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();
        registry.evict("user-1");
        let rt2 = registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();

        assert!(!Arc::ptr_eq(&rt1, &rt2), "post-eviction call must rebuild");
    }

    /// Regression: `stage_restore_backup` only writes the pending file — if the
    /// bootstrap does not swap it in before `Db::open`, Restore from backup is
    /// a silent no-op while the UI keeps promising it will apply "next restart".
    #[test]
    fn apply_staged_restore_swaps_the_pending_file_over_the_live_db() {
        let dir = tempfile::tempdir().unwrap();
        let user_dir = dir.path();
        let key = test_key();

        // A "live" DB holding the value the restore is meant to roll back.
        {
            let live = finsight_core::Db::open(&user_dir.join("data.sqlcipher"), &key).unwrap();
            finsight_core::db::run_migrations(&live).unwrap();
            let conn = live.get().unwrap();
            finsight_core::settings::set(&conn, "marker", &"live").unwrap();
        }
        // A staged restore holding the value the user wants back.
        {
            let staged =
                finsight_core::Db::open(&user_dir.join("data.pending-restore.sqlcipher"), &key)
                    .unwrap();
            finsight_core::db::run_migrations(&staged).unwrap();
            let conn = staged.get().unwrap();
            finsight_core::settings::set(&conn, "marker", &"restored").unwrap();
        }
        // Stale sidecars from the live DB must not survive the swap.
        std::fs::write(user_dir.join("data.sqlcipher-wal"), b"stale").unwrap();
        std::fs::write(user_dir.join("data.sqlcipher-shm"), b"stale").unwrap();

        apply_staged_restore(user_dir);

        // Sidecars are checked BEFORE reopening — opening the DB legitimately
        // recreates a WAL, which would mask whether the stale one was dropped.
        assert!(!user_dir.join("data.sqlcipher-wal").exists(), "stale WAL survived");
        assert!(!user_dir.join("data.sqlcipher-shm").exists(), "stale SHM survived");
        assert!(
            !user_dir.join("data.pending-restore.sqlcipher").exists(),
            "pending file must be consumed, or get_data_health reports pendingRestore forever"
        );

        let db = finsight_core::Db::open(&user_dir.join("data.sqlcipher"), &key).unwrap();
        let conn = db.get().unwrap();
        let marker: Option<String> = finsight_core::settings::get(&conn, "marker").unwrap();
        assert_eq!(marker.as_deref(), Some("restored"), "restore was not applied");
    }

    #[test]
    fn apply_staged_restore_is_a_no_op_without_a_pending_file() {
        let dir = tempfile::tempdir().unwrap();
        let key = test_key();
        let path = dir.path().join("data.sqlcipher");
        {
            let live = finsight_core::Db::open(&path, &key).unwrap();
            finsight_core::db::run_migrations(&live).unwrap();
            let conn = live.get().unwrap();
            finsight_core::settings::set(&conn, "marker", &"live").unwrap();
        }

        apply_staged_restore(dir.path());

        let db = finsight_core::Db::open(&path, &key).unwrap();
        let conn = db.get().unwrap();
        let marker: Option<String> = finsight_core::settings::get(&conn, "marker").unwrap();
        assert_eq!(marker.as_deref(), Some("live"), "live db must be untouched");
    }

    /// End-to-end: a restore staged between two bootstraps is live afterwards.
    #[tokio::test]
    async fn bootstrap_applies_a_restore_staged_since_the_last_bootstrap() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let key = test_key();

        registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();
        let user_dir = user_data_dir(dir.path(), "user-1");
        {
            let staged =
                finsight_core::Db::open(&user_dir.join("data.pending-restore.sqlcipher"), &key)
                    .unwrap();
            finsight_core::db::run_migrations(&staged).unwrap();
            let conn = staged.get().unwrap();
            finsight_core::settings::set(&conn, "marker", &"restored").unwrap();
        }

        // Runtime rebuild is the server's equivalent of "restart FinSight".
        registry.evict("user-1");
        let staged_path = user_dir.join("data.pending-restore.sqlcipher");
        let rt = registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();

        assert!(
            !staged_path.exists(),
            "bootstrap did not consume the staged file (rename failed?)"
        );
        let conn = rt.api.db.get().unwrap();
        let marker: Option<String> = finsight_core::settings::get(&conn, "marker").unwrap();
        assert_eq!(marker.as_deref(), Some("restored"));
    }

    /// Migrations are the one cascade step that must NOT degrade to a warning:
    /// serving a schema that does not match the code is worse than a 500 for
    /// this one user. Guards the deliberate strictness of `migration_error`,
    /// and confirms the pre-migration snapshot lands BEFORE the failing
    /// migration — that backup is the whole point of the guard.
    #[tokio::test]
    async fn bootstrap_fails_when_migrations_fail() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let key = test_key();

        // Poison the schema: a table a migration will try to create.
        let user_dir = user_data_dir(dir.path(), "user-1");
        std::fs::create_dir_all(&user_dir).unwrap();
        {
            let db = finsight_core::Db::open(&user_dir.join("data.sqlcipher"), &key).unwrap();
            let conn = db.get().unwrap();
            conn.execute("CREATE TABLE accounts(not_the_real_schema TEXT)", [])
                .unwrap();
        }

        // (`expect_err` would need `UserRuntime: Debug`, which it isn't.)
        let err = match registry.get_or_bootstrap(dir.path(), "user-1", &key).await {
            Ok(_) => panic!("a failed migration must not yield a usable runtime"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("migrations failed"),
            "unexpected error: {err}"
        );

        // The pre-migration backup was still taken, so the user is recoverable.
        let backups = user_dir.join("backups");
        assert_eq!(
            std::fs::read_dir(&backups).map(|d| d.count()).unwrap_or(0),
            1,
            "pre-migration snapshot must exist even when the migration fails"
        );

        // The failure was not cached: a later attempt re-runs the bootstrap.
        assert!(registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .is_err());
    }

    /// Regression: without single-flight, the ~14 queries of a cold page load
    /// each miss the map and independently open + migrate the same file.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_cold_callers_join_a_single_bootstrap() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Arc::new(Registry::default());
        let key = test_key();

        let mut tasks = Vec::new();
        for _ in 0..12 {
            let registry = Arc::clone(&registry);
            let path = dir.path().to_path_buf();
            let key = key.clone();
            tasks.push(tokio::spawn(async move {
                registry.get_or_bootstrap(&path, "user-1", &key).await.unwrap()
            }));
        }
        let mut runtimes: Vec<Arc<UserRuntime>> = Vec::new();
        for task in tasks {
            runtimes.push(task.await.unwrap());
        }

        for rt in &runtimes {
            assert!(
                Arc::ptr_eq(&runtimes[0], rt),
                "all concurrent callers must observe ONE runtime"
            );
        }
        // The load-bearing assertion: the cascade's pre-migration backup runs
        // once per bootstrap of an unmigrated DB, so a duplicated bootstrap
        // leaves a second snapshot behind.
        let backups = user_data_dir(dir.path(), "user-1").join("backups");
        let snapshots = std::fs::read_dir(&backups).unwrap().count();
        assert_eq!(snapshots, 1, "bootstrap ran more than once");
    }

    /// Regression: a backgrounded PWA tab holding an open EventSource stops
    /// issuing RPCs (so `touch()` goes quiet) but is emphatically still alive.
    #[tokio::test]
    async fn evict_idle_spares_runtimes_with_live_sse_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let key = test_key();

        let rt = registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();
        let subscriber = rt.events.subscribe();

        assert!(
            registry.evict_idle(Duration::ZERO).is_empty(),
            "a runtime with a live SSE subscriber must never be evicted"
        );
        assert!(registry.0.lock().unwrap().contains_key("user-1"));

        // Once the stream closes, normal idle eviction resumes.
        drop(subscriber);
        assert_eq!(registry.evict_idle(Duration::ZERO), vec!["user-1".to_string()]);
    }

    #[tokio::test]
    async fn evict_idle_zero_evicts_and_returns_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let key = test_key();

        registry
            .get_or_bootstrap(dir.path(), "user-1", &key)
            .await
            .unwrap();

        let evicted = registry.evict_idle(Duration::ZERO);
        assert_eq!(evicted, vec!["user-1".to_string()]);
        assert!(!registry.0.lock().unwrap().contains_key("user-1"));
    }
}

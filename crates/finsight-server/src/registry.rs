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
    /// Handle for this runtime's background sync loop; aborted on eviction.
    pub sync_task: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
pub struct Registry(Mutex<HashMap<String, Arc<UserRuntime>>>);

pub fn user_data_dir(data_dir: &Path, user_id: &str) -> PathBuf {
    data_dir.join("users").join(user_id)
}

impl Registry {
    /// Get or lazily bootstrap the runtime for `user_id`. Mirrors Phase 1's
    /// `ServerState::bootstrap` but per-user: open Db with `db_key_hex`, run
    /// migrations + provider migration, wire AgentEvent→broadcast (the same
    /// names `configure_app`/Phase 1's `state.rs` use), `ApiState::new`,
    /// load+set provider.
    ///
    /// Locking shape: the map lock is held ONLY for the initial lookup and
    /// the final insert-if-absent — never across the bootstrap itself, which
    /// does real I/O (DB open, migrations, agent spawn) and must not block
    /// other users' lookups. If two requests race to bootstrap the same user,
    /// both do the (wasted) I/O, but only the first to re-acquire the lock
    /// wins the insert; the loser's freshly-built runtime is dropped and its
    /// caller uses the winner's Arc instead — so callers never observe two
    /// live runtimes for one user.
    pub async fn get_or_bootstrap(
        &self,
        data_dir: &Path,
        user_id: &str,
        db_key_hex: &str,
    ) -> anyhow::Result<Arc<UserRuntime>> {
        if let Some(rt) = self.0.lock().unwrap().get(user_id) {
            return Ok(Arc::clone(rt));
        }

        // --- Bootstrap outside the lock ---
        let user_dir = user_data_dir(data_dir, user_id);
        std::fs::create_dir_all(&user_dir)?;
        let db = finsight_core::Db::open(&user_dir.join("data.sqlcipher"), db_key_hex)?;
        finsight_core::db::run_migrations(&db)?;
        finsight_api::provider::migrate_provider_settings(&db)?;

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

        // Login catch-up: refresh derived state (integrity check, pending
        // migrations, categorization/transfer-pairing/balances/net-worth/
        // anomaly recompute) — the same cascade the desktop app runs on
        // launch, extracted to `finsight_api::startup` so both callers share
        // it. This does real (blocking) DB work, so it runs on a blocking
        // thread rather than tying up an async worker; no lock is held
        // across this `.await` (the map lock was already released above).
        let cascade_db = db.clone();
        let backups_dir = user_dir.join("backups");
        let uid_for_log = user_id.to_string();
        match tokio::task::spawn_blocking(move || {
            finsight_api::startup::run_startup_cascade(&cascade_db, &backups_dir)
        })
        .await
        {
            Ok(report) => {
                if !report.warnings.is_empty() {
                    tracing::warn!(
                        user_id = %uid_for_log,
                        warnings = ?report.warnings,
                        "login catch-up cascade reported warnings"
                    );
                }
            }
            Err(e) => {
                tracing::error!(user_id = %uid_for_log, "login catch-up cascade task panicked: {e}");
            }
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

        let runtime = Arc::new(UserRuntime {
            api,
            events: tx,
            last_active: Mutex::new(Instant::now()),
            sync_task,
        });

        // --- Insert-if-absent, lock held only for this ---
        let mut map = self.0.lock().unwrap();
        if let Some(existing) = map.get(user_id) {
            // Another request raced us and won; use theirs. Abort OUR
            // sync_task before dropping our runtime — a dropped JoinHandle
            // does NOT cancel the task it points to, it just detaches it,
            // which would otherwise leave a second, orphaned background
            // sync loop running forever for this user.
            let existing = Arc::clone(existing);
            drop(map);
            runtime.sync_task.abort();
            return Ok(existing);
        }
        map.insert(user_id.to_string(), Arc::clone(&runtime));
        Ok(runtime)
    }

    pub fn touch(&self, user_id: &str) {
        if let Some(rt) = self.0.lock().unwrap().get(user_id) {
            *rt.last_active.lock().unwrap() = Instant::now();
        }
    }

    /// Removes the runtime and aborts its background sync task.
    pub fn evict(&self, user_id: &str) {
        if let Some(rt) = self.0.lock().unwrap().remove(user_id) {
            rt.sync_task.abort();
        }
    }

    /// Called by a background interval task: evict runtimes idle > `max_idle`.
    /// Returns the evicted user ids for logging.
    pub fn evict_idle(&self, max_idle: Duration) -> Vec<String> {
        let now = Instant::now();
        let idle_ids: Vec<String> = {
            let map = self.0.lock().unwrap();
            map.iter()
                .filter(|(_, rt)| {
                    now.duration_since(*rt.last_active.lock().unwrap()) >= max_idle
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

//! Background + batch SimpleFin sync scheduler.
//!
//! Lives on `ApiState`. Spawns a background Tokio task that sleeps for
//! the configured interval (default 6 hours) then syncs all linked accounts.
//! Manual "sync all" calls go through the scheduler to avoid overlapping.

use finsight_core::keychain;
use finsight_core::models::{SimpleFinAlert, SimpleFinConnectionPatch};
use finsight_core::repos::{accounts, alerts, connections, run, sync_runs};
use finsight_core::settings;
use finsight_core::Db;
use finsight_providers::simplefin::{
    check_drift, commit_simplefin_import_for_run, detect_transfers, fetch_simplefin_data,
    import_holdings,
};
use finsight_providers::ProviderError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

const SIMPLEFIN_ACCESS_SERVICE: &str = "com.finsight.simplefin.access";
const DEFAULT_INTERVAL_MINUTES: u32 = 360;
const BRIDGE_CALL_STAGGER_SECS: u64 = 1;
const MAX_FETCH_ATTEMPTS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinSyncSettings {
    pub background_sync_enabled: bool,
    pub background_sync_interval_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountSyncResult {
    pub account_id: String,
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,
    pub queued_for_review: usize,
    pub error: Option<String>,
}

pub struct SyncScheduler {
    db: Db,
    interval_minutes: Arc<AtomicU32>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    sync_in_progress: Arc<AtomicBool>,
}

impl SyncScheduler {
    pub fn new(db: Db) -> Self {
        let (enabled, interval) = Self::read_stored_settings(&db);
        Self {
            db,
            interval_minutes: Arc::new(AtomicU32::new(interval)),
            enabled: Arc::new(AtomicBool::new(enabled)),
            shutdown: Arc::new(AtomicBool::new(false)),
            sync_in_progress: Arc::new(AtomicBool::new(false)),
        }
    }

    fn read_stored_settings(db: &Db) -> (bool, u32) {
        let conn = match db.get() {
            Ok(c) => c,
            Err(_) => return (true, DEFAULT_INTERVAL_MINUTES),
        };
        let enabled: bool = settings::get::<bool>(&conn, "simplefin.background_sync_enabled")
            .ok()
            .flatten()
            .unwrap_or(true);
        let interval: u32 =
            settings::get::<u32>(&conn, "simplefin.background_sync_interval_minutes")
                .ok()
                .flatten()
                .unwrap_or(DEFAULT_INTERVAL_MINUTES);
        (enabled, interval)
    }

    /// Start the background sync loop. Must be called once. Returns the JoinHandle.
    ///
    /// Takes an explicit runtime `Handle` and spawns via `handle.spawn` rather
    /// than the bare `tokio::spawn`. The only call site is inside Tauri's
    /// synchronous `.setup()` closure, which has NO ambient Tokio runtime entered
    /// (the desktop `main` is a plain `fn main`, not `#[tokio::main]`), so
    /// `tokio::spawn` there would panic with "there is no reactor running".
    /// `Handle::spawn` needs no ambient context.
    pub fn start(&self, handle: &tokio::runtime::Handle) -> tokio::task::JoinHandle<()> {
        let enabled = self.enabled.clone();
        let shutdown = self.shutdown.clone();
        let interval = self.interval_minutes.clone();
        let db = self.db.clone();
        let sync_in_progress = self.sync_in_progress.clone();
        handle.spawn(async move {
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    return;
                }
                if !enabled.load(Ordering::Relaxed) {
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
                let mins = interval.load(Ordering::Relaxed);
                if mins == 0 {
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
                let seconds = (mins as u64) * 60;
                for _ in 0..seconds {
                    if shutdown.load(Ordering::Relaxed) {
                        return;
                    }
                    if !enabled.load(Ordering::Relaxed) {
                        break;
                    }
                    // Re-read interval each second so changes take effect fast.
                    let current = interval.load(Ordering::Relaxed);
                    if current != mins {
                        break; // restart the outer loop with the new interval
                    }
                    sleep(Duration::from_secs(1)).await;
                }
                if !enabled.load(Ordering::Relaxed) {
                    continue;
                }
                // Sync all linked accounts.
                tracing::info!("background SimpleFin sync starting");
                let results =
                    sync_all_accounts_with_guard(&db, &sync_in_progress, "background").await;
                let errors: Vec<_> = results.iter().filter(|r| r.error.is_some()).collect();
                if !errors.is_empty() {
                    tracing::warn!("background sync had {} errors", errors.len());
                } else {
                    tracing::info!(
                        "background SimpleFin sync complete ({} accounts)",
                        results.len()
                    );
                }
            }
        })
    }

    pub fn set_interval(&self, minutes: u32) {
        self.interval_minutes.store(minutes, Ordering::Relaxed);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn interval(&self) -> u32 {
        self.interval_minutes.load(Ordering::Relaxed)
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    pub async fn sync_all_now(&self) -> Vec<AccountSyncResult> {
        sync_all_accounts_with_guard(&self.db, &self.sync_in_progress, "manual").await
    }
}

/// Sync every linked SimpleFin account across all active connections.
/// Returns per-account results. Called from both the background loop and
/// the manual "sync all" Tauri command.
pub async fn sync_all_accounts(db: &Db) -> Vec<AccountSyncResult> {
    let guard = Arc::new(AtomicBool::new(false));
    sync_all_accounts_with_guard(db, &guard, "manual").await
}

async fn sync_all_accounts_with_guard(
    db: &Db,
    sync_in_progress: &Arc<AtomicBool>,
    trigger: &str,
) -> Vec<AccountSyncResult> {
    if sync_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return vec![AccountSyncResult {
            account_id: "sync-all".to_string(),
            added: 0,
            updated: 0,
            skipped: 0,
            queued_for_review: 0,
            error: Some("A SimpleFIN sync is already running".to_string()),
        }];
    }

    let sync_run_id = run(db, {
        let trigger = trigger.to_string();
        move |conn| sync_runs::start(conn, &trigger).map(|r| r.id)
    })
    .await
    .ok();

    let results = sync_all_accounts_inner(db, sync_run_id.as_deref()).await;

    if let Some(run_id) = sync_run_id {
        let errors: Vec<String> = results.iter().filter_map(|r| r.error.clone()).collect();
        let status = if results.is_empty() || results.iter().all(|r| r.error.is_some()) {
            "failed"
        } else if errors.is_empty() {
            "success"
        } else {
            "partial"
        };
        let _ = run(db, {
            let run_id = run_id.clone();
            let errors = errors.clone();
            let results = results.clone();
            move |conn| {
                sync_runs::finish(
                    conn,
                    &run_id,
                    sync_runs::SyncRunFinish {
                        status: status.to_string(),
                        accounts_total: results.len() as i64,
                        accounts_succeeded: results.iter().filter(|r| r.error.is_none()).count()
                            as i64,
                        accounts_failed: results.iter().filter(|r| r.error.is_some()).count()
                            as i64,
                        added: results.iter().map(|r| r.added as i64).sum(),
                        updated: results.iter().map(|r| r.updated as i64).sum(),
                        skipped: results.iter().map(|r| r.skipped as i64).sum(),
                        queued_for_review: results.iter().map(|r| r.queued_for_review as i64).sum(),
                        error_summary: if errors.is_empty() {
                            None
                        } else {
                            Some(errors.join("; "))
                        },
                    },
                )
                .map(|_| ())
            }
        })
        .await;
    }

    sync_in_progress.store(false, Ordering::SeqCst);
    results
}

async fn sync_all_accounts_inner(db: &Db, sync_run_id: Option<&str>) -> Vec<AccountSyncResult> {
    let conn_rows = match run(db, |conn| connections::list(conn)).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("failed to list connections: {e}");
            return Vec::new();
        }
    };

    let active: Vec<_> = conn_rows
        .into_iter()
        .filter(|c| c.status == "active")
        .collect();
    if active.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for conn_row in &active {
        let access_url = match keychain::get_key(SIMPLEFIN_ACCESS_SERVICE, &conn_row.access_url_ref)
        {
            Ok(Some(url)) => url,
            Ok(None) => {
                let _ =
                    mark_connection_error(db, &conn_row.id, "missing access url in keychain").await;
                continue;
            }
            Err(e) => {
                let _ = mark_connection_error(db, &conn_row.id, &e.to_string()).await;
                continue;
            }
        };

        let linked = match run(db, {
            let connection_id = conn_row.id.clone();
            move |conn| accounts::list_by_connection_id(conn, &connection_id)
        })
        .await
        {
            Ok(accounts) => accounts,
            Err(e) => {
                tracing::error!(
                    "failed to list accounts for connection {}: {e}",
                    conn_row.id
                );
                continue;
            }
        };

        let account_ids: Vec<String> = linked.iter().map(|a| a.id.clone()).collect();

        for account in &linked {
            let result =
                sync_one_account(db, account, &access_url, Some(&conn_row.id), sync_run_id).await;
            results.push(result);
        }

        // Run post-processors: transfer detection across all linked accounts.
        let conn_id = conn_row.id.clone();
        let aids = account_ids.clone();
        let _ = run(db, {
            move |conn| {
                match detect_transfers(conn, &aids) {
                    Ok(detected) => {
                        if !detected.is_empty() {
                            tracing::info!(
                                conn_id = %conn_id,
                                count = detected.len(),
                                "transfer suggestions detected"
                            );
                        }
                    }
                    Err(e) => tracing::error!(
                        conn_id = %conn_id,
                        "transfer detection failed: {e}"
                    ),
                }
                Ok(())
            }
        })
        .await;

        sleep(Duration::from_secs(BRIDGE_CALL_STAGGER_SECS)).await;
    }

    results
}

async fn sync_one_account(
    db: &Db,
    account: &finsight_core::models::Account,
    access_url: &str,
    connection_id: Option<&str>,
    sync_run_id: Option<&str>,
) -> AccountSyncResult {
    let simplefin_id = match &account.simplefin_account_id {
        Some(id) => id.clone(),
        None => {
            return AccountSyncResult {
                account_id: account.id.clone(),
                added: 0,
                updated: 0,
                skipped: 0,
                queued_for_review: 0,
                error: Some("missing simplefin_account_id".to_string()),
            };
        }
    };

    let import_pending = account.import_pending;

    // Snapshot the ledger epoch before the network fetch so we can refuse to
    // commit fetched rows if a Delete-All lands while this (scheduled or manual
    // "sync all") sync is in flight. Sync inserts top-level rows with no FK
    // guard, so a post-wipe commit would survive without this.
    let start_epoch = db.reset_barrier().epoch();

    let pending = match fetch_with_retry(
        access_url,
        &simplefin_id,
        &account.id,
        account.last_synced_at,
        import_pending,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            if let Some(connection_id) = connection_id {
                let _ = mark_connection_error(db, connection_id, &e.to_string()).await;
            }
            let _ = create_sync_error_alert(db, &account.id, &e.to_string()).await;
            return AccountSyncResult {
                account_id: account.id.clone(),
                added: 0,
                updated: 0,
                skipped: 0,
                queued_for_review: 0,
                error: Some(format!("fetch failed: {e}")),
            };
        }
    };

    let fresh_extra = pending.sfin_account.extra.clone();
    let conn_id = connection_id.map(|s| s.to_string());
    let acc_id = account.id.clone();
    let is_investment = account.r#type == finsight_core::models::AccountType::Investment;
    let sync_run_id_owned = sync_run_id.map(str::to_string);

    // Hold a reset lease across the commit; skip it if a Delete-All landed while
    // we fetched. Delete-All drains this lease before wiping, so fetched rows
    // either commit before the wipe (and are wiped) or are never written.
    let commit_lease = db.reset_barrier().writer_lease(start_epoch).await;
    if commit_lease.superseded() {
        return AccountSyncResult {
            account_id: account.id.clone(),
            added: 0,
            updated: 0,
            skipped: 0,
            queued_for_review: 0,
            error: Some("sync cancelled: data was cleared during the sync".to_string()),
        };
    }

    let summary = match run(db, {
        let pending = pending;
        let acc_id = acc_id.clone();
        let sync_run_id = sync_run_id_owned.clone();
        move |conn| {
            let summary = commit_simplefin_import_for_run(pending, conn, sync_run_id.as_deref())
                .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))?;

            // Drift check.
            match check_drift(conn, &acc_id) {
                Ok(Some(alert)) => tracing::info!(
                    account_id = %acc_id, severity = %alert.severity,
                    "drift alert created"
                ),
                Ok(None) => {}
                Err(e) => tracing::error!(account_id = %acc_id, "drift check failed: {e}"),
            }

            // Holdings import for investment accounts.
            if is_investment {
                let cid = conn_id.as_deref().unwrap_or("");
                match import_holdings(conn, cid, &acc_id, fresh_extra.as_ref()) {
                    Ok(holdings) => {
                        if !holdings.is_empty() {
                            tracing::info!(
                                account_id = %acc_id, count = holdings.len(),
                                "holdings imported"
                            );
                        }
                    }
                    Err(e) => tracing::error!(account_id = %acc_id, "holdings import failed: {e}"),
                }
            }

            Ok(summary)
        }
    })
    .await
    {
        Ok(s) => s,
        Err(e) => {
            if let Some(connection_id) = connection_id {
                let _ = mark_connection_error(db, connection_id, &e.to_string()).await;
            }
            let _ = create_sync_error_alert(db, &acc_id, &e.to_string()).await;
            return AccountSyncResult {
                account_id: acc_id,
                added: 0,
                updated: 0,
                skipped: 0,
                queued_for_review: 0,
                error: Some(format!("commit failed: {e}")),
            };
        }
    };
    // Commit is done (and was wiped-or-safe under the lease); release it. The
    // connection-status update below writes only self-healing metadata.
    drop(commit_lease);

    if let Some(connection_id) = connection_id {
        let _ = mark_connection_success(db, connection_id).await;
    }

    AccountSyncResult {
        account_id: acc_id,
        added: summary.added,
        updated: summary.updated,
        skipped: summary.skipped,
        queued_for_review: summary.queued_for_review,
        error: None,
    }
}

async fn fetch_with_retry(
    access_url: &str,
    simplefin_id: &str,
    account_id: &str,
    last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    import_pending: bool,
) -> Result<finsight_providers::simplefin::sync::PendingImport, ProviderError> {
    let mut attempt = 0usize;
    loop {
        match fetch_simplefin_data(
            access_url,
            simplefin_id,
            account_id,
            last_synced_at,
            import_pending,
        )
        .await
        {
            Ok(pending) => return Ok(pending),
            Err(e) if should_retry(&e) && attempt + 1 < MAX_FETCH_ATTEMPTS => {
                let delay = 1u64 << attempt;
                attempt += 1;
                tracing::warn!(attempt, delay, error = %e, "retrying SimpleFIN fetch");
                sleep(Duration::from_secs(delay)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn should_retry(error: &ProviderError) -> bool {
    match error {
        ProviderError::Http(e) => {
            e.is_timeout()
                || e.is_connect()
                || e.status().map(|s| s.is_server_error()).unwrap_or(false)
        }
        ProviderError::ServerError(msg) => {
            let lower = msg.to_ascii_lowercase();
            !lower.contains("payment required")
                && !lower.contains("forbidden")
                && !lower.contains("auth")
                && !lower.contains("403")
                && !lower.contains("402")
        }
        _ => false,
    }
}

async fn create_sync_error_alert(
    db: &Db,
    account_id: &str,
    message: &str,
) -> Result<(), finsight_core::CoreError> {
    let account_id = account_id.to_string();
    let message = message.to_string();
    run(db, move |conn| {
        if alerts::has_recent_unacknowledged(conn, &account_id, "sync_error")? {
            return Ok(());
        }
        alerts::create(
            conn,
            SimpleFinAlert {
                id: Uuid::new_v4().to_string(),
                account_id,
                alert_type: "sync_error".to_string(),
                severity: "error".to_string(),
                message: format!("SimpleFIN sync failed: {message}"),
                details_json: None,
                acknowledged_at: None,
                created_at: chrono::Utc::now(),
            },
        )?;
        Ok(())
    })
    .await
}

async fn mark_connection_success(
    db: &Db,
    connection_id: &str,
) -> Result<(), finsight_core::CoreError> {
    let id = connection_id.to_string();
    run(db, move |conn| {
        connections::update(
            conn,
            &id,
            SimpleFinConnectionPatch {
                status: Some("active".to_string()),
                last_error: Some(None),
                last_synced_at: Some(Some(chrono::Utc::now())),
                ..Default::default()
            },
        )
        .map(|_| ())
    })
    .await
}

async fn mark_connection_error(
    db: &Db,
    connection_id: &str,
    error_msg: &str,
) -> Result<(), finsight_core::CoreError> {
    let id = connection_id.to_string();
    let msg = error_msg.to_string();
    run(db, move |conn| {
        connections::update(
            conn,
            &id,
            SimpleFinConnectionPatch {
                status: Some("error".to_string()),
                last_error: Some(Some(msg)),
                ..Default::default()
            },
        )
        .map(|_| ())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::keychain;

    /// Regression guard: `SyncScheduler::start` must NOT require an ambient Tokio
    /// runtime. Its real call site is inside Tauri's synchronous `.setup()` closure,
    /// which has no runtime entered — a bare `tokio::spawn` there panics with
    /// "there is no reactor running". We reproduce that exact environment: build a
    /// runtime, grab its `Handle`, then call `start(&handle)` from a plain
    /// `std::thread` with NO runtime entered (deliberately not `#[tokio::test]`),
    /// and assert it returns a JoinHandle without panicking.
    #[test]
    fn start_does_not_need_ambient_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("sched.sqlcipher"), &key).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle = rt.handle().clone();
        let scheduler = SyncScheduler::new(db);

        // Run from a bare OS thread: no `#[tokio::test]`, no `Runtime::enter`, so
        // there is no thread-local runtime context. A bare `tokio::spawn` would
        // panic here; `handle.spawn` must not.
        let join = std::thread::spawn(move || {
            let task = scheduler.start(&handle);
            // Immediately stop the loop so the spawned task can exit cleanly.
            scheduler.stop();
            task
        })
        .join()
        .expect("start must not panic without an ambient runtime");

        // The task was accepted by the runtime; drop the handle without awaiting.
        drop(join);
        rt.shutdown_background();
    }
}

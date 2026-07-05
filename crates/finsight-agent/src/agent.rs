use crate::CompletionProvider;
use finsight_core::Db;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AgentJob {
    CategorizeImport {
        import_id: String,
    },
    CategorizeAll,
    /// Re-run LLM categorization on transactions whose current AI confidence
    /// is below the threshold. Useful after a user adds new rules or corrections.
    RecategorizeLowConfidence,
    RunRecipe {
        recipe_id: String,
    },
    CheckDueRecipes,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum AgentEvent {
    CategorizationProgress {
        import_id: Option<String>,
        done: u32,
        total: u32,
    },
    CategorizationComplete {
        import_id: Option<String>,
        categorized: u32,
        skipped: u32,
    },
    Error {
        message: String,
    },
}

pub type EventCallback = Arc<dyn Fn(AgentEvent) + Send + Sync>;

pub struct AgentHandle {
    pub tx: mpsc::Sender<AgentJob>,
    provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    /// Monotonic reset generation. A running categorization job snapshots this
    /// at start and aborts at its next batch boundary if it changes — so work
    /// that began before a Delete-All / factory reset can never write
    /// categorizations against the freshly-wiped ledger.
    reset_epoch: Arc<AtomicU64>,
}

impl AgentHandle {
    /// Spawn the agent background task and return a handle to enqueue jobs.
    /// `on_event` is called on the spawning thread's runtime for each event emitted.
    pub fn spawn(
        db: Db,
        provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
        on_event: EventCallback,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<AgentJob>(64);
        let provider_clone = Arc::clone(&provider);
        let reset_epoch = Arc::new(AtomicU64::new(0));
        let reset_epoch_loop = Arc::clone(&reset_epoch);
        // Spawn a dedicated OS thread with its own Tokio runtime so this
        // works whether or not a runtime is already active on the calling
        // thread (e.g. Tauri's synchronous `.setup()` callback).
        std::thread::Builder::new()
            .name("finsight-agent".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("agent tokio runtime");
                rt.block_on(run_loop(db, rx, provider_clone, on_event, reset_epoch_loop));
            })
            .expect("spawn agent thread");
        Self {
            tx,
            provider,
            reset_epoch,
        }
    }

    /// Replace the active provider at runtime. No task restart needed.
    pub fn set_provider(&self, p: Arc<dyn CompletionProvider>) {
        *self.provider.write().unwrap() = Some(p);
    }

    /// Signal that persisted data was wiped (Delete-All / factory reset). A
    /// categorization job already in flight will stop at its next batch
    /// boundary instead of writing against the reset ledger. Cheap and safe to
    /// call even when no job is running.
    pub fn cancel_running_work(&self) {
        self.reset_epoch.fetch_add(1, Ordering::SeqCst);
    }
}

async fn run_loop(
    db: Db,
    mut rx: mpsc::Receiver<AgentJob>,
    provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    on_event: EventCallback,
    reset_epoch: Arc<AtomicU64>,
) {
    while let Some(job) = rx.recv().await {
        match job {
            AgentJob::CheckDueRecipes => {
                let p = provider.read().unwrap().clone();
                if let Some(p) = p {
                    let _ = crate::recipe_runner::run_due_recipes(&db, p).await;
                }
            }
            AgentJob::RunRecipe { recipe_id } => {
                let p = provider.read().unwrap().clone();
                match p {
                    None => {
                        on_event(AgentEvent::Error {
                            message: "No completion provider configured".to_string(),
                        });
                    }
                    Some(p) => {
                        if let Err(e) =
                            crate::recipe_runner::run_recipe(&db, &recipe_id, Arc::clone(&p)).await
                        {
                            on_event(AgentEvent::Error {
                                message: format!("Recipe '{}' failed: {e}", recipe_id),
                            });
                        }
                    }
                }
            }
            job @ (AgentJob::CategorizeImport { .. }
            | AgentJob::CategorizeAll
            | AgentJob::RecategorizeLowConfidence) => {
                let p = provider.read().unwrap().clone();
                match p {
                    None => {
                        on_event(AgentEvent::Error {
                            message: "No completion provider configured".to_string(),
                        });
                    }
                    Some(p) => {
                        let result = crate::categorizer::run_job(
                            &db,
                            job,
                            p,
                            Arc::clone(&on_event),
                            Arc::clone(&reset_epoch),
                        )
                        .await;
                        if let Err(e) = result {
                            on_event(AgentEvent::Error {
                                message: e.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;
    use finsight_core::{db::run_migrations, keychain};
    use serde_json::json;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[tokio::test]
    async fn handle_sends_job_and_receives_error_when_no_provider() {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("h.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();

        let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let handle = AgentHandle::spawn(
            db,
            provider,
            Arc::new(move |e| {
                events_clone.lock().unwrap().push(e);
            }),
        );

        handle.tx.send(AgentJob::CategorizeAll).await.unwrap();
        let mut seen_error = false;
        for _ in 0..20 {
            if events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, AgentEvent::Error { .. }))
            {
                seen_error = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(
            seen_error,
            "expected an error event when no provider is configured"
        );
    }

    #[tokio::test]
    async fn set_provider_replaces_atomically() {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("sp.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        let handle = AgentHandle::spawn(db, Arc::clone(&provider), Arc::new(|_| {}));
        let mock = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([]),
            tool_turns: Mutex::new(vec![]),
        });
        handle.set_provider(mock);
        let locked = provider.read().unwrap();
        assert!(locked.is_some());
        assert_eq!(locked.as_ref().unwrap().provider_id(), "mock");
    }
}

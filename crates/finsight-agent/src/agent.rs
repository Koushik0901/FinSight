use crate::CompletionProvider;
use finsight_core::Db;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AgentJob {
    CategorizeImport { import_id: String },
    CategorizeAll,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum AgentEvent {
    CategorizationProgress { import_id: Option<String>, done: u32, total: u32 },
    CategorizationComplete { import_id: Option<String>, categorized: u32, skipped: u32 },
    Error { message: String },
}

pub type EventCallback = Arc<dyn Fn(AgentEvent) + Send + Sync>;

pub struct AgentHandle {
    pub tx: mpsc::Sender<AgentJob>,
    provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
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
        tokio::spawn(run_loop(db, rx, provider_clone, on_event));
        Self { tx, provider }
    }

    /// Replace the active provider at runtime. No task restart needed.
    pub fn set_provider(&self, p: Arc<dyn CompletionProvider>) {
        *self.provider.write().unwrap() = Some(p);
    }
}

async fn run_loop(
    db: Db,
    mut rx: mpsc::Receiver<AgentJob>,
    provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    on_event: EventCallback,
) {
    while let Some(job) = rx.recv().await {
        let p = provider.read().unwrap().clone();
        match p {
            None => {
                on_event(AgentEvent::Error {
                    message: "No completion provider configured".to_string(),
                });
            }
            Some(p) => {
                let result = crate::categorizer::run_job(&db, job, p, Arc::clone(&on_event)).await;
                if let Err(e) = result {
                    on_event(AgentEvent::Error { message: e.to_string() });
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
        let handle = AgentHandle::spawn(db, provider, Arc::new(move |e| {
            events_clone.lock().unwrap().push(e);
        }));

        handle.tx.send(AgentJob::CategorizeAll).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let evs = events.lock().unwrap();
        assert!(evs.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
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
        });
        handle.set_provider(mock);
        let locked = provider.read().unwrap();
        assert!(locked.is_some());
        assert_eq!(locked.as_ref().unwrap().provider_id(), "mock");
    }
}

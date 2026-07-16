use finsight_agent::agent::{AgentEvent, EventCallback};
use finsight_api::ApiState;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;

/// One event as the UI's Tauri-event shim expects it: `{ event, payload }`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OutboundEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

pub struct ServerState {
    pub api: Arc<ApiState>,
    pub events: broadcast::Sender<OutboundEvent>,
}

/// Phase 1 key management: hex keyfile in the data dir (Phase 2 replaces this
/// with per-user password-wrapped keys). NOT the OS keychain: must work headless
/// (a Docker container has no Secret Service / Keychain / Credential Manager).
fn load_or_create_keyfile(data_dir: &Path) -> std::io::Result<String> {
    let path = data_dir.join("db.key");
    if path.exists() {
        return Ok(std::fs::read_to_string(&path)?.trim().to_string());
    }
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // This key decrypts the whole DB: create owner-read-only (0600) atomically
    // on Unix so it is never even briefly world-readable (Docker hosts often
    // run with a permissive umask). `create_new` also removes the exists→write
    // TOCTOU: a concurrent creator loses with an error instead of clobbering.
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(&path)?.write_all(key.as_bytes())?;
    }
    Ok(key)
}

impl ServerState {
    pub async fn bootstrap(data_dir: &Path) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir)?;
        let key = load_or_create_keyfile(data_dir)?;
        let db = finsight_core::Db::open(&data_dir.join("data.sqlcipher"), &key)?;
        finsight_core::db::run_migrations(&db)?;
        finsight_api::provider::migrate_provider_settings(&db)?;

        let (tx, _) = broadcast::channel::<OutboundEvent>(256);
        let etx = tx.clone();
        let on_event: EventCallback = Arc::new(move |event: AgentEvent| {
            // Same names configure_app uses, so the UI listeners work unchanged.
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

        // NOTE: ApiState::new constructs a SyncScheduler but never starts it.
        // Starting it is a separate, explicit `sync_scheduler.start(&handle)`
        // call (see finsight-app's setup). Background sync is a Phase 2 item
        // for the server — deliberately not started here in Phase 1.
        let api = Arc::new(ApiState::new(db.clone(), data_dir.to_path_buf(), on_event));
        if let Some(p) = finsight_api::provider::load_provider_from_settings(&db) {
            api.agent.set_provider(p);
        }
        Ok(Arc::new(Self { api, events: tx }))
    }
}

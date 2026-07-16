pub mod commands; // transport-agnostic command surface: each command is a plain
                  // `async fn(&ApiState, args) -> AppResult<T>`, shared by the Tauri
                  // wrappers and finsight-server. See commands/mod.rs.
pub mod error;
pub mod sink; // FrameSink: transport-agnostic event emission (progress/streaming).
              // See sink.rs. Consumed by import_csv and copilot_chat once they move here.

use finsight_agent::{
    agent::{AgentHandle, EventCallback},
    CompletionProvider,
};
use finsight_core::Db;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Transport-agnostic application state: everything a command needs,
/// with no Tauri types. Both the Tauri app and finsight-server own one.
pub struct ApiState {
    pub db: Arc<Db>,
    pub agent: AgentHandle,
    pub agent_provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    /// App-data directory. Consumed by finsight-server (DB + keyfile location) and
    /// by the data_health commands once they migrate off `tauri::AppHandle`.
    pub data_dir: PathBuf,
}

impl ApiState {
    pub fn new(db: Db, data_dir: PathBuf, on_event: EventCallback) -> Self {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let agent = AgentHandle::spawn(db.clone(), Arc::clone(&provider), on_event);
        Self {
            db: Arc::new(db),
            agent,
            agent_provider: provider,
            data_dir,
        }
    }
}

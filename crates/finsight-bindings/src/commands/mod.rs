/// Tauri-backed [`finsight_api::sink::FrameSink`]: forwards emissions to real
/// Tauri window events, the same events the frontend already listens for
/// (`"import-progress"`, `"import-complete"`, and — once Task 6 lands —
/// `"copilot-stream-frame"`). finsight-server's dispatcher (Task 9) uses a
/// different `FrameSink` impl (`BroadcastSink`) that fans the same events out
/// over SSE instead.
pub struct TauriFrameSink(pub tauri::AppHandle);
impl finsight_api::sink::FrameSink for TauriFrameSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        let _ = self.0.emit(event, payload);
    }
}

pub mod accounts;
pub mod agent;
pub mod assets;
pub mod budget;
pub mod cashflow;
pub mod categories;
pub mod copilot;
pub mod copilot_chat;
pub mod data_health;
pub mod household;
pub mod import;
pub mod inbox;
pub mod insights;
pub mod investments;
pub mod journey;
pub mod meta;
pub mod metrics;
pub mod onboarding;
pub mod planned_transactions;
pub mod push;
pub mod recipes;
pub mod recurring;
pub mod reports;
pub mod restoration;
pub mod scenarios;
pub mod settings;
pub mod simplefin;
pub mod spending;
pub mod transactions;

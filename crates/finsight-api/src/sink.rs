use std::sync::Arc;

/// Transport-agnostic replacement for `tauri::AppHandle::emit`. The Tauri app
/// emits window events; finsight-server pushes into a broadcast channel → SSE.
pub trait FrameSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

/// Wire names for every event crossing the backend→frontend boundary, plus the
/// one frontend-synthesized auth event that completes the shared vocabulary.
///
/// This module is the single source of truth for these strings: emitters
/// reference the consts, `cargo run -p finsight-bindings --bin export_bindings`
/// generates `ui/src/api/eventNames.ts` from [`event_names::ALL`], and
/// `crates/finsight-server/tests/parity.rs` fails if the generated file ever
/// drifts. To rename an event, change it here and regenerate — never edit a
/// string literal at an emit site or in TypeScript.
pub mod event_names {
    /// One streamed chunk/frame of a Copilot answer (both chat runtimes).
    pub const COPILOT_STREAM_FRAME: &str = "copilot-stream-frame";
    /// A finished background Copilot answer (async follow-up delivery).
    pub const COPILOT_ASYNC_ANSWER: &str = "copilot-async-answer";
    /// CSV import row progress.
    pub const IMPORT_PROGRESS: &str = "import-progress";
    /// CSV import finished (success or failure).
    pub const IMPORT_COMPLETE: &str = "import-complete";
    /// Background AI categorizer made progress on its scan.
    pub const CATEGORIZATION_PROGRESS: &str = "categorization.progress";
    /// Background AI categorizer finished its scan.
    pub const CATEGORIZATION_COMPLETE: &str = "categorization.complete";
    /// Background agent job failed (no dedicated frontend listener yet; still
    /// part of the wire contract via SSE).
    pub const AGENT_ERROR: &str = "agent.error";
    /// SSE comment-frame keep-alive sent by finsight-server.
    pub const KEEP_ALIVE: &str = "finsight:keepalive";
    /// Frontend-synthesized only (httpBackend on a 401, logout): never emitted
    /// by the backend. Listed so the whole event vocabulary lives in one place.
    pub const AUTH_REQUIRED: &str = "finsight:auth-required";

    /// Every name above. Drives the generated TypeScript mirror and its
    /// parity guard.
    pub const ALL: &[&str] = &[
        COPILOT_STREAM_FRAME,
        COPILOT_ASYNC_ANSWER,
        IMPORT_PROGRESS,
        IMPORT_COMPLETE,
        CATEGORIZATION_PROGRESS,
        CATEGORIZATION_COMPLETE,
        AGENT_ERROR,
        KEEP_ALIVE,
        AUTH_REQUIRED,
    ];
}

/// A no-op sink: for command paths that emit but where the caller doesn't care
/// (and as a safe default). Also handy in unit tests that ignore emissions.
pub struct NullSink;
impl FrameSink for NullSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}

/// Test/collector sink — records every (event, payload) in order.
pub struct VecSink(pub std::sync::Mutex<Vec<(String, serde_json::Value)>>);
impl VecSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(std::sync::Mutex::new(Vec::new())))
    }
}
impl FrameSink for VecSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.lock().unwrap().push((event.to_string(), payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vec_sink_collects_events_in_order() {
        let sink = VecSink::new();
        sink.emit(
            event_names::IMPORT_PROGRESS,
            serde_json::json!({"rows_done": 1}),
        );
        sink.emit(event_names::IMPORT_COMPLETE, serde_json::json!({"ok": true}));
        let got = sink.0.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, event_names::IMPORT_PROGRESS);
        assert_eq!(got[1].1["ok"], true);
    }

    /// Guard against accidental duplicates or typos in the contract list.
    #[test]
    fn all_event_names_are_unique_and_non_empty() {
        let mut seen = std::collections::BTreeSet::new();
        for name in event_names::ALL {
            assert!(!name.is_empty());
            assert!(seen.insert(*name), "duplicate event name: {name}");
        }
    }
}

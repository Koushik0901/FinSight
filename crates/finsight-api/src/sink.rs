use std::sync::Arc;

/// Transport-agnostic replacement for `tauri::AppHandle::emit`. The Tauri app
/// emits window events; finsight-server pushes into a broadcast channel → SSE.
pub trait FrameSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
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
        sink.emit("import-progress", serde_json::json!({"rows_done": 1}));
        sink.emit("import-complete", serde_json::json!({"ok": true}));
        let got = sink.0.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "import-progress");
        assert_eq!(got[1].1["ok"], true);
    }
}

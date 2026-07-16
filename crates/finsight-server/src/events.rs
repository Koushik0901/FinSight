use crate::state::{OutboundEvent, ServerState};
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// One SSE `data:` line: `{"event": name, "payload": ...}` — the shim
/// dispatches on `event`, mirroring Tauri's listen(event) semantics.
pub fn sse_data(ev: &OutboundEvent) -> String {
    serde_json::to_string(ev).unwrap_or_else(|_| "{}".into())
}

pub async fn events(
    State(st): State<Arc<ServerState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = st.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|ev| match ev {
        Ok(ev) => Some(Ok(Event::default().data(sse_data(&ev)))),
        Err(_lagged) => None, // dropped frames are acceptable; see spec reconnect rule
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_event_reaches_sse_subscriber() {
        let state = crate::router::tests::test_state().await;
        let mut rx = state.events.subscribe();
        state
            .events
            .send(OutboundEvent {
                event: "copilot-stream-frame".into(),
                payload: serde_json::json!({"type":"text","delta":"hi"}),
            })
            .unwrap();
        let got = rx.recv().await.unwrap();
        assert_eq!(got.event, "copilot-stream-frame");
        let line = sse_data(&got);
        assert!(line.contains("\"event\":\"copilot-stream-frame\""));
    }

    /// Integration-flavored: drives the actual `BroadcastStream` the `events`
    /// handler wraps (not a raw `rx.recv()`), and confirms the mapped item
    /// serializes through `sse_data` with the exact shape `httpBackend.ts`
    /// parses (`JSON.parse(msg.data)` → `{ event, payload }`).
    #[tokio::test]
    async fn broadcast_stream_yields_event_mapped_through_sse_data() {
        let state = crate::router::tests::test_state().await;
        let mut stream = BroadcastStream::new(state.events.subscribe());
        state
            .events
            .send(OutboundEvent {
                event: "categorization.progress".into(),
                payload: serde_json::json!({"done": 3, "total": 10}),
            })
            .unwrap();
        let item = stream.next().await.unwrap().unwrap();
        let line = sse_data(&item);
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["event"], "categorization.progress");
        assert_eq!(parsed["payload"]["done"], 3);
    }
}

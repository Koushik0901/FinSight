use crate::auth::AuthedUser;
use crate::state::{OutboundEvent, ServerState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use finsight_api::error::AppError;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// One SSE `data:` line: `{"event": name, "payload": ...}` — the shim
/// dispatches on `event`, mirroring Tauri's listen(event) semantics.
pub fn sse_data(ev: &OutboundEvent) -> String {
    serde_json::to_string(ev).unwrap_or_else(|_| "{}".into())
}

pub async fn events(State(st): State<Arc<ServerState>>, user: AuthedUser) -> Response {
    let rt = match st
        .registry
        .get_or_bootstrap(&st.data_dir, &user.user_id, &user.db_key_hex)
        .await
    {
        Ok(rt) => rt,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError::new("auth.runtime", e.to_string())),
            )
                .into_response()
        }
    };
    st.registry.touch(&user.user_id);
    let rx = rt.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|ev| match ev {
        Ok(ev) => Some(Ok::<_, Infallible>(Event::default().data(sse_data(&ev)))),
        Err(_lagged) => None, // dropped frames are acceptable; see spec reconnect rule
    });
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A standalone per-user runtime (not routed through `ServerState`/HTTP) —
    /// enough to exercise the broadcast→SSE mapping these tests care about.
    async fn test_runtime() -> Arc<crate::registry::UserRuntime> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.keep();
        let key = finsight_core::keychain::generate_random_key().to_string();
        let registry = crate::registry::Registry::default();
        registry
            .get_or_bootstrap(&path, "user-1", &key)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn broadcast_event_reaches_sse_subscriber() {
        let rt = test_runtime().await;
        let mut rx = rt.events.subscribe();
        rt.events
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
        let rt = test_runtime().await;
        let mut stream = BroadcastStream::new(rt.events.subscribe());
        rt.events
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

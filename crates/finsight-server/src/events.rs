use crate::auth::AuthedUser;
use crate::state::{OutboundEvent, ServerState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use finsight_api::error::AppError;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

static DROPPED_FRAMES: AtomicU64 = AtomicU64::new(0);
static LAG_WARNED: AtomicBool = AtomicBool::new(false);

/// Total broadcast frames dropped due to lag since process start.
pub fn dropped_frames() -> u64 {
    DROPPED_FRAMES.load(Ordering::Relaxed)
}

const KEEP_ALIVE_EVENT: &str = finsight_api::sink::event_names::KEEP_ALIVE;

/// One SSE `data:` line: `{"event": name, "payload": ...}` — the shim
/// dispatches on `event`, mirroring Tauri's listen(event) semantics.
pub fn sse_data(ev: &OutboundEvent) -> String {
    serde_json::to_string(ev).unwrap_or_else(|_| "{}".into())
}

fn sse_keep_alive_data() -> String {
    sse_data(&OutboundEvent {
        event: KEEP_ALIVE_EVENT.into(),
        payload: serde_json::Value::Null,
    })
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
        Ok(ev) => {
            // Reset burst flag on successful delivery so next lag burst warns again.
            LAG_WARNED.store(false, Ordering::Relaxed);
            Some(Ok::<_, Infallible>(Event::default().data(sse_data(&ev))))
        }
        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
            let total = DROPPED_FRAMES.fetch_add(n, Ordering::Relaxed) + n;
            // Emit lag_warned metric once per burst to avoid log spam while still surfacing the loss.
            if !LAG_WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    dropped = n,
                    total_dropped = total,
                    "sse broadcast lagged, dropped frames"
                );
            }
            None
        }
    });
    // Safari/WebKit can time out an otherwise healthy EventSource when the
    // only traffic is an SSE comment. Send a real, valid envelope instead;
    // the browser shim parses it and safely ignores the reserved event name
    // because no listener is registered for it.
    let keep_alive = KeepAlive::new()
        .interval(Duration::from_secs(15))
        .event(Event::default().data(sse_keep_alive_data()));
    Sse::new(stream).keep_alive(keep_alive).into_response()
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
                event: finsight_api::sink::event_names::COPILOT_STREAM_FRAME.into(),
                payload: serde_json::json!({"type":"text","delta":"hi"}),
            })
            .unwrap();
        let got = rx.recv().await.unwrap();
        assert_eq!(
            got.event,
            finsight_api::sink::event_names::COPILOT_STREAM_FRAME
        );
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
                event: finsight_api::sink::event_names::CATEGORIZATION_PROGRESS.into(),
                payload: serde_json::json!({"done": 3, "total": 10}),
            })
            .unwrap();
        let item = stream.next().await.unwrap().unwrap();
        let line = sse_data(&item);
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            parsed["event"],
            finsight_api::sink::event_names::CATEGORIZATION_PROGRESS
        );
        assert_eq!(parsed["payload"]["done"], 3);
    }

    #[test]
    fn keep_alive_is_a_valid_ignorable_event_envelope() {
        let parsed: serde_json::Value = serde_json::from_str(&sse_keep_alive_data()).unwrap();
        assert_eq!(parsed["event"], KEEP_ALIVE_EVENT);
        assert!(parsed["payload"].is_null());
    }
}

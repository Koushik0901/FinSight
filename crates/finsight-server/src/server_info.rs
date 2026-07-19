//! Server↔client version handshake. `protocol` is an integer bumped whenever a
//! breaking wire change ships (RPC arg shapes, event frames, auth flow). A PWA
//! cached offline may be older than the server after an upgrade — the client
//! compares its own CLIENT_PROTOCOL (ui/src/api/serverInfo.ts) against
//! `minClientProtocol` and shows a "refresh to update" banner on mismatch.

use axum::Json;

/// Wire-protocol version. Bump on any breaking RPC/event/auth change.
pub const PROTOCOL_VERSION: u32 = 1;
/// Oldest client protocol this server still serves. Raise it only when a change
/// genuinely breaks older cached clients (forcing them to refresh).
pub const MIN_CLIENT_PROTOCOL: u32 = 1;

/// `GET /api/server/about` — open (no auth): a cached client must be able to
/// learn it's out of date even before/without a valid session.
pub async fn about() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_VERSION,
        "minClientProtocol": MIN_CLIENT_PROTOCOL,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn about_returns_version_and_protocol_without_auth() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let res = app
            .oneshot(Request::get("/api/server/about").body(Body::empty()).unwrap())
            .await
            .unwrap();
        // No auth cookie — this route must stay open.
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["protocol"], PROTOCOL_VERSION);
        assert_eq!(v["minClientProtocol"], MIN_CLIENT_PROTOCOL);
        assert!(v["version"].as_str().is_some());
    }
}

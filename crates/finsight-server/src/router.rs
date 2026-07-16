use crate::state::ServerState;
use axum::{
    routing::{get, post},
    Json, Router,
};
use std::path::Path;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

/// `ui_dir` is the built frontend (`ui/dist` in production). The fallback
/// service serves any real static file it finds there; anything else (an SPA
/// client-side route like `/transactions`) falls back to `index.html` so the
/// React router can take over. Registered LAST so `/api/*` routes always win.
pub fn build_router(state: Arc<ServerState>, ui_dir: &Path) -> Router {
    let index = ui_dir.join("index.html");
    Router::new()
        .route(
            "/api/health",
            get(|| async { Json(serde_json::json!({"status":"ok"})) }),
        )
        .route("/api/rpc/{cmd}", post(crate::dispatch::rpc))
        .route("/api/events", get(crate::events::events))
        .with_state(state)
        .fallback_service(ServeDir::new(ui_dir).fallback(ServeFile::new(index)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    pub(crate) async fn test_state() -> Arc<ServerState> {
        let dir = tempfile::tempdir().unwrap();
        // Leak the tempdir so the DB outlives the test body.
        let path = dir.keep();
        ServerState::bootstrap(&path).await.unwrap()
    }

    /// A fresh, empty tempdir for tests that only exercise `/api/*` routes and
    /// don't care about static file serving. `ServeDir` resolves lazily per
    /// request, so an empty (or even missing) directory never panics here —
    /// unmatched requests just fall through to a 404.
    pub(crate) fn test_ui_dir() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap();
        dir.keep()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = build_router(test_state().await, &test_ui_dir());
        let res = app
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn spa_fallback_serves_index_html_for_unknown_route() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state().await, &ui_dir);
        let res = app
            .oneshot(
                Request::get("/some/spa/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"SENTINEL_INDEX_HTML");
    }

    #[tokio::test]
    async fn api_routes_win_over_static_fallback() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state().await, &ui_dir);
        let res = app
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[tokio::test]
    async fn real_static_file_is_served_from_ui_dir() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        std::fs::write(ui_dir.join("asset.txt"), "STATIC_ASSET_CONTENT").unwrap();
        let app = build_router(test_state().await, &ui_dir);
        let res = app
            .oneshot(Request::get("/asset.txt").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"STATIC_ASSET_CONTENT");
    }
}

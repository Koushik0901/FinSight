use crate::state::ServerState;
use axum::{
    routing::{delete, get, post},
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
        .route("/api/server/about", get(crate::server_info::about))
        .route("/api/auth/status", get(crate::auth::status))
        .route("/api/auth/setup", post(crate::auth::setup))
        .route("/api/auth/login", post(crate::auth::login))
        .route("/api/auth/logout", post(crate::auth::logout))
        .route(
            "/api/auth/users",
            get(crate::auth::list_users).post(crate::auth::create_user),
        )
        .route("/api/auth/users/{id}", delete(crate::auth::delete_user))
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

    pub(crate) fn test_state() -> Arc<ServerState> {
        let dir = tempfile::tempdir().unwrap();
        // Leak the tempdir so the DB outlives the test body.
        let path = dir.keep();
        ServerState::bootstrap(&path).unwrap()
    }

    /// A fresh, empty tempdir for tests that only exercise `/api/*` routes and
    /// don't care about static file serving. `ServeDir` resolves lazily per
    /// request, so an empty (or even missing) directory never panics here —
    /// unmatched requests just fall through to a 404.
    pub(crate) fn test_ui_dir() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap();
        dir.keep()
    }

    /// Runs `POST /api/auth/setup` on `app` (creating the sole admin account,
    /// username `tester`) and returns the `finsight_session=<token>` cookie
    /// pair for use in a `Cookie:` header on subsequent authenticated
    /// requests. Panics if setup doesn't succeed (callers rely on a fresh,
    /// unauthenticated `ServerState` from `test_state()`).
    pub(crate) async fn setup_and_login(app: &Router) -> String {
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/auth/setup")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"tester","password":"hunter22"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "setup must succeed for the test helper");
        let cookie = res
            .headers()
            .get(axum::http::header::SET_COOKIE)
            .expect("setup response must set a session cookie")
            .to_str()
            .unwrap();
        // The Set-Cookie value is `name=value; HttpOnly; ...` — the request
        // `Cookie:` header only wants the first `name=value` segment.
        cookie.split(';').next().unwrap().to_string()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = build_router(test_state(), &test_ui_dir());
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
        let app = build_router(test_state(), &ui_dir);
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
        let app = build_router(test_state(), &ui_dir);
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
        let app = build_router(test_state(), &ui_dir);
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

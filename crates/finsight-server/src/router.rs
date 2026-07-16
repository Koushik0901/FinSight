use crate::state::ServerState;
use axum::{routing::get, Json, Router};
use std::sync::Arc;

pub fn build_router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route(
            "/api/health",
            get(|| async { Json(serde_json::json!({"status":"ok"})) }),
        )
        .with_state(state)
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

    #[tokio::test]
    async fn health_returns_ok() {
        let app = build_router(test_state().await);
        let res = app
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}

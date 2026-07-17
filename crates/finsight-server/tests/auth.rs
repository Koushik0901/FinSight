//! Auth integration tests: setup, login/logout, session-cookie enforcement,
//! admin user management, and per-user data isolation. Runs the real router
//! (`build_router`) over `tower::ServiceExt::oneshot` — no network socket.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use finsight_server::router::build_router;
use finsight_server::state::ServerState;
use std::path::PathBuf;
use std::sync::Arc;
use tower::util::ServiceExt;

fn fresh_state() -> (Arc<ServerState>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep(); // leak so the DB outlives the test body
    let state = ServerState::bootstrap(&path).unwrap();
    (state, path)
}

fn test_ui_dir() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    dir.keep()
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Pulls the `finsight_session=<token>` pair out of a response's `Set-Cookie`
/// header, ready to hand back as a request `Cookie:` header value.
fn cookie_from(res: &axum::response::Response) -> String {
    let raw = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("response should set a session cookie")
        .to_str()
        .unwrap();
    raw.split(';').next().unwrap().to_string()
}

fn setup_req(username: &str, password: &str) -> Request<Body> {
    Request::post("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"username": username, "password": password}).to_string(),
        ))
        .unwrap()
}

fn login_req(username: &str, password: &str) -> Request<Body> {
    Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"username": username, "password": password}).to_string(),
        ))
        .unwrap()
}

fn rpc_req(cmd: &str, cookie: &str) -> Request<Body> {
    Request::post(format!("/api/rpc/{cmd}"))
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from("{}"))
        .unwrap()
}

fn new_account_payload(name: &str) -> serde_json::Value {
    // NewAccount has no `rename_all`, so keys are snake_case (matches
    // dispatch.rs's rpc tests and bindings.ts's `NewAccount` TS type).
    serde_json::json!({ "input": {
        "owner": "You",
        "bank": "Test Bank",
        "type": "Checking",
        "name": name,
        "currency": "USD",
        "color": "#336699",
        "opening_balance_cents": 0
    }})
}

#[tokio::test]
async fn setup_when_empty_creates_admin_and_logs_in() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app
        .clone()
        .oneshot(setup_req("alice", "correct horse battery staple"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = cookie_from(&res);
    let body = json_body(res).await;
    let recovery_key = body["recoveryKey"].as_str().expect("recoveryKey present");
    assert_eq!(recovery_key.split('-').count(), 8, "recovery key is 8 dash-separated groups");

    let res = app
        .oneshot(
            Request::get("/api/auth/status")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["authenticated"], true);
    assert_eq!(body["username"], "alice");
    assert_eq!(body["isAdmin"], true);
    assert_eq!(body["needsSetup"], false);
}

#[tokio::test]
async fn setup_twice_is_409() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app
        .clone()
        .oneshot(setup_req("alice", "correct horse battery staple"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(setup_req("mallory", "whatever-password-123"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.already_setup");
}

#[tokio::test]
async fn login_logout_lifecycle() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app.clone().oneshot(setup_req("alice", "hunter22-plus")).await.unwrap();
    let cookie = cookie_from(&res);

    // Authenticated rpc works.
    let res = app.clone().oneshot(rpc_req("list_accounts", &cookie)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Logout invalidates the session.
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/logout")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app.clone().oneshot(rpc_req("list_accounts", &cookie)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Logging back in issues a fresh, working session.
    let res = app.clone().oneshot(login_req("alice", "hunter22-plus")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie2 = cookie_from(&res);
    let res = app.oneshot(rpc_req("list_accounts", &cookie2)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn bad_password_is_401_bad_credentials() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());
    app.clone()
        .oneshot(setup_req("alice", "correct horse battery staple"))
        .await
        .unwrap();

    let res = app.clone().oneshot(login_req("alice", "wrong-password")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.bad_credentials");

    // Unknown username returns the SAME code — no username oracle.
    let res = app.oneshot(login_req("nobody-here", "whatever")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.bad_credentials");
}

#[tokio::test]
async fn rpc_without_cookie_is_401_auth_required() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app
        .oneshot(
            Request::post("/api/rpc/list_accounts")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.required");
}

#[tokio::test]
async fn admin_create_user_and_user_isolation() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app.clone().oneshot(setup_req("admin", "hunter22-plus")).await.unwrap();
    let admin_cookie = cookie_from(&res);

    // Admin creates an account in their OWN db.
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/rpc/create_account")
                .header("content-type", "application/json")
                .header("cookie", admin_cookie.clone())
                .body(Body::from(new_account_payload("Admin Acct").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Admin creates bob.
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/users")
                .header("content-type", "application/json")
                .header("cookie", admin_cookie)
                .body(Body::from(r#"{"username":"bob","password":"bobs-password-1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body["recoveryKey"].as_str().unwrap().split('-').count(),
        8
    );

    // Bob logs in and sees an EMPTY account list — isolated from admin's DB.
    let res = app.clone().oneshot(login_req("bob", "bobs-password-1")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bob_cookie = cookie_from(&res);

    let res = app.oneshot(rpc_req("list_accounts", &bob_cookie)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn non_admin_cannot_manage_users() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app.clone().oneshot(setup_req("admin", "hunter22-plus")).await.unwrap();
    let admin_cookie = cookie_from(&res);

    app.clone()
        .oneshot(
            Request::post("/api/auth/users")
                .header("content-type", "application/json")
                .header("cookie", admin_cookie)
                .body(Body::from(r#"{"username":"bob","password":"bobs-password-1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let res = app.clone().oneshot(login_req("bob", "bobs-password-1")).await.unwrap();
    let bob_cookie = cookie_from(&res);

    let res = app
        .oneshot(
            Request::post("/api/auth/users")
                .header("content-type", "application/json")
                .header("cookie", bob_cookie)
                .body(Body::from(r#"{"username":"carol","password":"carols-password"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.admin_required");
}

#[tokio::test]
async fn delete_user_removes_dir_and_sessions() {
    let (state, data_dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app.clone().oneshot(setup_req("admin", "hunter22-plus")).await.unwrap();
    let admin_cookie = cookie_from(&res);

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/users")
                .header("content-type", "application/json")
                .header("cookie", admin_cookie.clone())
                .body(Body::from(r#"{"username":"bob","password":"bobs-password-1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app.clone().oneshot(login_req("bob", "bobs-password-1")).await.unwrap();
    let bob_cookie = cookie_from(&res);

    // Touch an rpc route so bob's per-user runtime (and on-disk dir) exists.
    let res = app.clone().oneshot(rpc_req("list_accounts", &bob_cookie)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/auth/users")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(res).await;
    let bob_id = body
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "bob")
        .expect("bob in the admin's user list")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let bob_dir = data_dir.join("users").join(&bob_id);
    assert!(bob_dir.exists(), "bob's data dir should exist before delete");

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/auth/users/{bob_id}"))
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    assert!(!bob_dir.exists(), "bob's data dir should be removed on delete");

    let res = app.oneshot(rpc_req("list_accounts", &bob_cookie)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

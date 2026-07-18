//! Auth integration tests: setup, login/logout, session-cookie enforcement,
//! admin user management, and per-user data isolation. Runs the real router
//! (`build_router`) over `tower::ServiceExt::oneshot` — no network socket.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use finsight_server::router::build_router;
use finsight_server::sessions::LoginThrottle;
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

fn recover_req(username: &str, recovery_key: &str, new_password: &str) -> Request<Body> {
    Request::post("/api/auth/recover")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": username,
                "recoveryKey": recovery_key,
                "newPassword": new_password,
            })
            .to_string(),
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

// ------------------------------------------------- recovery redemption ---

/// The headline flow: a user who forgot their password redeems their recovery
/// key, lands logged in with a new password, and gets a REPLACEMENT key. The
/// four things that must all hold afterwards are asserted together because
/// each on its own is satisfiable by a broken implementation.
#[tokio::test]
async fn recover_resets_password_rotates_key_and_logs_in() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app
        .clone()
        .oneshot(setup_req("alice", "original-password"))
        .await
        .unwrap();
    let setup_cookie = cookie_from(&res);
    let old_recovery = json_body(res).await["recoveryKey"].as_str().unwrap().to_string();

    // Prove the account owns real data, so we can prove recovery preserves it
    // (the db key is re-wrapped, never regenerated).
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/rpc/create_account")
                .header("content-type", "application/json")
                .header("cookie", setup_cookie)
                .body(Body::from(new_account_payload("Pre-Recovery Acct").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Redeem.
    let res = app
        .clone()
        .oneshot(recover_req("alice", &old_recovery, "brand-new-password"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let recovered_cookie = cookie_from(&res);
    let new_recovery = json_body(res).await["recoveryKey"]
        .as_str()
        .expect("recovery returns a replacement key")
        .to_string();
    assert_eq!(new_recovery.split('-').count(), 8);
    assert_ne!(new_recovery, old_recovery, "the recovery key must ROTATE");

    // 1. The session handed back is live AND opens the SAME database — the
    //    pre-existing account is still there, so the db key survived intact.
    let res = app
        .clone()
        .oneshot(rpc_req("list_accounts", &recovered_cookie))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let accounts = json_body(res).await;
    assert_eq!(
        accounts.as_array().unwrap().len(),
        1,
        "recovery must re-wrap the existing db key, not mint a fresh empty database"
    );
    assert_eq!(accounts[0]["name"], "Pre-Recovery Acct");

    // 2. The new password works.
    let res = app
        .clone()
        .oneshot(login_req("alice", "brand-new-password"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. The old password does not.
    let res = app
        .clone()
        .oneshot(login_req("alice", "original-password"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(res).await["code"], "auth.bad_credentials");

    // 4. The OLD recovery key is spent — replaying it must not work.
    let res = app
        .oneshot(recover_req("alice", &old_recovery, "third-password-here"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "a redeemed recovery key must stop working"
    );
    assert_eq!(json_body(res).await["code"], "auth.bad_recovery_key");
}

/// Wrong key and unknown username must be INDISTINGUISHABLE — same status,
/// same code, same message. Otherwise recover becomes the username oracle that
/// login is careful not to be.
#[tokio::test]
async fn recover_wrong_key_and_unknown_user_are_identical_401s() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());
    app.clone()
        .oneshot(setup_req("alice", "original-password"))
        .await
        .unwrap();

    let wrong_key = "deadbeef-".repeat(7) + "deadbeef";
    let res = app
        .clone()
        .oneshot(recover_req("alice", &wrong_key, "brand-new-password"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let wrong_key_body = json_body(res).await;
    assert_eq!(wrong_key_body["code"], "auth.bad_recovery_key");

    let res = app
        .oneshot(recover_req("nobody-here", &wrong_key, "brand-new-password"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let unknown_user_body = json_body(res).await;

    assert_eq!(
        unknown_user_body, wrong_key_body,
        "unknown username must be byte-identical to a wrong key — no username oracle"
    );
}

/// Recovery must not be a side door around the password policy.
#[tokio::test]
async fn weak_password_rejected_on_setup_create_user_and_recover() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    // setup
    let res = app.clone().oneshot(setup_req("alice", "short-pw")).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.weak_password");
    assert!(
        body["message"].as_str().unwrap().contains("10"),
        "the error must state the minimum, got: {}",
        body["message"]
    );

    // A compliant setup succeeds, giving us an admin + a recovery key.
    let res = app
        .clone()
        .oneshot(setup_req("alice", "original-password"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let admin_cookie = cookie_from(&res);
    let recovery = json_body(res).await["recoveryKey"].as_str().unwrap().to_string();

    // create_user
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/users")
                .header("content-type", "application/json")
                .header("cookie", admin_cookie)
                .body(Body::from(r#"{"username":"bob","password":"short-pw"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "auth.weak_password");

    // recover — checked BEFORE the key is consumed, so a rejected attempt
    // leaves the recovery key still usable.
    let res = app
        .clone()
        .oneshot(recover_req("alice", &recovery, "short-pw"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "auth.weak_password");

    let res = app
        .oneshot(recover_req("alice", &recovery, "a-compliant-password"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a policy rejection must not burn the recovery key"
    );
}

// ------------------------------------------------------ login throttle ---

/// Five consecutive failures lock the username; the lock lifts on its own once
/// the cooldown elapses. Built on a 250ms cooldown via `bootstrap_with_throttle`
/// so the expiry half is testable without a 60-second sleep.
#[tokio::test]
async fn login_locks_out_after_five_failures_then_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep();
    let state = ServerState::bootstrap_with_throttle(
        &path,
        LoginThrottle::new(std::time::Duration::from_millis(250)),
    )
    .unwrap();
    let app = build_router(state, &test_ui_dir());

    app.clone()
        .oneshot(setup_req("alice", "original-password"))
        .await
        .unwrap();

    // Failures 1..=4 stay 401 — the budget isn't spent yet.
    for i in 1..5 {
        let res = app.clone().oneshot(login_req("alice", "wrong-password")).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "attempt {i} should be 401");
    }

    // The 5th trips the lock.
    let res = app.clone().oneshot(login_req("alice", "wrong-password")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Now even the CORRECT password is refused, with 429 — proving it's a
    // lockout and not just another credential rejection.
    let res = app.clone().oneshot(login_req("alice", "original-password")).await.unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json_body(res).await["code"], "auth.too_many_attempts");

    // Recovery is locked out on the same budget.
    let res = app
        .clone()
        .oneshot(recover_req("alice", "whatever-key", "a-new-password-x"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);

    // After the window, the correct password works again.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let res = app.clone().oneshot(login_req("alice", "original-password")).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "the lock must lift once the cooldown elapses"
    );
}

/// The lockout must key on the submitted string whether or not the account
/// exists — an existence-dependent lockout would leak account existence
/// through the very mechanism added to protect it.
#[tokio::test]
async fn lockout_does_not_reveal_whether_the_account_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep();
    let state = ServerState::bootstrap_with_throttle(
        &path,
        LoginThrottle::new(std::time::Duration::from_millis(250)),
    )
    .unwrap();
    let app = build_router(state, &test_ui_dir());
    app.clone()
        .oneshot(setup_req("alice", "original-password"))
        .await
        .unwrap();

    for _ in 0..5 {
        app.clone()
            .oneshot(login_req("ghost-account", "wrong-password"))
            .await
            .unwrap();
    }

    let res = app
        .clone()
        .oneshot(login_req("ghost-account", "wrong-password"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "a nonexistent username must lock out exactly like a real one"
    );

    // And a real account is unaffected by another name's exhausted budget.
    let res = app.oneshot(login_req("alice", "original-password")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// A success mid-streak must clear the budget, or a user who mistypes a few
/// times, logs in, then mistypes once more would be locked out unexpectedly.
#[tokio::test]
async fn successful_login_clears_the_failure_budget() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());
    app.clone()
        .oneshot(setup_req("alice", "original-password"))
        .await
        .unwrap();

    for _ in 0..4 {
        app.clone().oneshot(login_req("alice", "wrong-password")).await.unwrap();
    }
    let res = app.clone().oneshot(login_req("alice", "original-password")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Budget reset: four more failures still don't lock.
    for _ in 0..4 {
        let res = app.clone().oneshot(login_req("alice", "wrong-password")).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
    let res = app.oneshot(login_req("alice", "original-password")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// Legacy-migration failure must fail LOUD and RETRYABLE, never silently orphan
/// the user's real Phase 1 data behind a fresh empty DB. Injection: place a
/// Phase 1-style `data.sqlcipher` + valid 64-hex `db.key`, then pre-create
/// `<data>/users` as a plain FILE so the migration's `create_dir_all(users/<id>)`
/// fails deterministically (independent of the random admin UUID). Assert setup
/// returns 500 `auth.migration_failed`, issues NO session cookie, and leaves the
/// registry empty so the operator can retry after fixing the cause.
#[tokio::test]
async fn migration_failure_is_loud_and_retryable() {
    let (state, data_dir) = fresh_state();

    // A Phase 1 single-user install: an (opaque) encrypted DB file plus its
    // plaintext hex keyfile (must be 64 hex chars = 32 bytes to parse).
    std::fs::write(data_dir.join("data.sqlcipher"), b"legacy-db-bytes").unwrap();
    std::fs::write(data_dir.join("db.key"), "aa".repeat(32)).unwrap();
    // Break the migration destination: `users` as a file, so create_dir_all
    // of `users/<admin-uuid>` cannot succeed for ANY generated id.
    std::fs::write(data_dir.join("users"), b"not a directory").unwrap();

    let app = build_router(state.clone(), &test_ui_dir());

    let res = app
        .oneshot(setup_req("admin", "hunter22-plus"))
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    // No session cookie handed out on a failed setup.
    assert!(
        res.headers().get(header::SET_COOKIE).is_none(),
        "a failed migration must not issue a session cookie"
    );
    let body = json_body(res).await;
    assert_eq!(body["code"], "auth.migration_failed");

    // Retryable: the admin row was rolled back, so setup can run again.
    assert!(
        state.users.is_empty().unwrap(),
        "admin account must be rolled back so users.is_empty() stays true (retryable)"
    );
    // The user's real DB was NOT deleted — it's still at the old path.
    assert!(data_dir.join("data.sqlcipher").exists());
}

/// The end of the story the previous test starts: after the operator clears
/// the fault, a RETRY must find the original ledger and migrate it — not
/// create an empty database over it. This is the user-visible half of the
/// partial-migration fix.
///
/// The multi-file sidecar case (main DB moves, `-wal` then fails, both get
/// restored) is covered by `move_all_or_rollback_restores_earlier_moves_when_a_later_one_fails`
/// in `auth.rs`. It cannot be injected here: the destination is
/// `users/<random-uuid>/`, so no path a test can pre-create will break
/// specifically the second move for an unknown UUID.
#[tokio::test]
async fn setup_retry_after_a_failed_migration_still_migrates_the_original_data() {
    let (state, data_dir) = fresh_state();

    // Distinctive bytes so we can prove THE ORIGINAL file arrived, rather than
    // a fresh empty DB that merely occupies the right path.
    std::fs::write(data_dir.join("data.sqlcipher"), b"ORIGINAL-LEDGER-BYTES").unwrap();
    std::fs::write(data_dir.join("data.sqlcipher-wal"), b"ORIGINAL-WAL-BYTES").unwrap();
    std::fs::write(data_dir.join("db.key"), "aa".repeat(32)).unwrap();
    std::fs::write(data_dir.join("users"), b"not a directory").unwrap();

    let app = build_router(state.clone(), &test_ui_dir());

    let res = app
        .clone()
        .oneshot(setup_req("admin", "hunter22-plus"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(state.users.is_empty().unwrap());

    // Nothing was consumed by the failed attempt: both DB files AND the
    // keyfile are still where a retry needs them. (The keyfile matters — it is
    // deleted only after every move commits; losing it would leave the legacy
    // DB unreadable forever.)
    assert!(data_dir.join("data.sqlcipher").exists());
    assert!(data_dir.join("data.sqlcipher-wal").exists());
    assert!(data_dir.join("db.key").exists(), "the legacy keyfile must survive a failed migration");

    // Operator clears the fault and retries.
    std::fs::remove_file(data_dir.join("users")).unwrap();

    let res = app.oneshot(setup_req("admin", "hunter22-plus")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "the retry must succeed");
    let admin_id = state.users.list_users().unwrap()[0].id.clone();

    let user_dir = data_dir.join("users").join(&admin_id);
    assert_eq!(
        std::fs::read(user_dir.join("data.sqlcipher")).unwrap(),
        b"ORIGINAL-LEDGER-BYTES",
        "the retry must migrate the ORIGINAL ledger, not create an empty database"
    );
    assert_eq!(
        std::fs::read(user_dir.join("data.sqlcipher-wal")).unwrap(),
        b"ORIGINAL-WAL-BYTES",
        "the -wal sidecar must travel with its main DB file"
    );
    // Old locations are cleared, including the now-redundant plaintext keyfile.
    assert!(!data_dir.join("data.sqlcipher").exists());
    assert!(!data_dir.join("db.key").exists());
}

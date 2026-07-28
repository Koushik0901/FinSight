//! OAuth 2.1 flow tests: dynamic client registration, consent, and the PKCE
//! code exchange a cloud connector (claude.ai, ChatGPT) runs to obtain a bearer
//! token — plus the ways that exchange must fail.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use base64::Engine as _;
use finsight_server::router::build_router;
use finsight_server::state::ServerState;
use sha2::Digest;
use std::path::PathBuf;
use tower::util::ServiceExt;

type App = axum::Router;

const REDIRECT_URI: &str = "https://claude.ai/api/mcp/auth_callback";
const VERIFIER: &str = "a-high-entropy-code-verifier-value-at-least-43-chars";

fn test_ui_dir() -> PathBuf {
    tempfile::tempdir().unwrap().keep()
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

fn cookie_from(res: &axum::response::Response) -> String {
    res.headers()
        .get(header::SET_COOKIE)
        .expect("expected a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn challenge_for(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()))
}

async fn setup() -> (App, String) {
    let dir = tempfile::tempdir().unwrap().keep();
    let state = ServerState::bootstrap(&dir).unwrap();
    let app = build_router(state, &test_ui_dir());
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/setup")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"alice","password":"hunter22-plus"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = cookie_from(&res);
    (app, cookie)
}

async fn register_client(app: &App, redirect_uris: serde_json::Value) -> serde_json::Value {
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/oauth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "client_name": "Claude",
                        "redirect_uris": redirect_uris,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = json_body(res).await;
    assert_eq!(status, StatusCode::CREATED, "registration failed: {body}");
    body
}

fn approve_req(
    cookie: &str,
    client_id: &str,
    scope: &str,
    challenge: &str,
    state: Option<&str>,
) -> Request<Body> {
    let mut body = serde_json::json!({
        "clientId": client_id,
        "redirectUri": REDIRECT_URI,
        "scope": scope,
        "codeChallenge": challenge,
        "codeChallengeMethod": "S256",
    });
    if let Some(s) = state {
        body["state"] = serde_json::json!(s);
    }
    Request::post("/api/oauth/approve")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn token_req(code: &str, client_id: &str, verifier: &str, redirect_uri: &str) -> Request<Body> {
    let form = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}&client_id={client_id}&code_verifier={verifier}",
        redirect_uri.replace(':', "%3A").replace('/', "%2F")
    );
    Request::post("/api/oauth/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap()
}

/// Pull `code` out of the redirect URL the consent step hands back.
fn code_from(redirect_to: &str) -> String {
    redirect_to
        .split(['?', '&'])
        .find_map(|p| p.strip_prefix("code="))
        .expect("consent should return a code")
        .to_string()
}

// ------------------------------------------------------------ happy path ---

#[tokio::test]
async fn register_consent_exchange_yields_a_working_mcp_token() {
    let (app, cookie) = setup().await;

    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    assert_eq!(client["token_endpoint_auth_method"], "none");

    // The consent screen names the app asking for access...
    let info = json_body(
        app.clone()
            .oneshot(
                Request::get(format!("/api/oauth/client?client_id={client_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(info["clientName"], "Claude");
    // ...but never leaks where that client is allowed to redirect.
    assert!(info.get("redirect_uris").is_none());

    let res = app
        .clone()
        .oneshot(approve_req(
            &cookie,
            &client_id,
            "full",
            &challenge_for(VERIFIER),
            Some("opaque&state=x"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let redirect_to = json_body(res).await["redirectTo"].as_str().unwrap().to_string();
    assert!(redirect_to.starts_with(REDIRECT_URI));
    // Opaque client state must not be able to inject its own parameters.
    assert!(
        redirect_to.contains("state=opaque%26state%3Dx"),
        "state must be percent-encoded, got {redirect_to}"
    );
    let code = code_from(&redirect_to);

    let res = app
        .clone()
        .oneshot(token_req(&code, &client_id, VERIFIER, REDIRECT_URI))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store",
        "a token response is a credential and must not be cached"
    );
    let body = json_body(res).await;
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["scope"], "full");
    let access_token = body["access_token"].as_str().unwrap().to_string();
    assert!(access_token.starts_with("finsight_pat_"));

    // The issued token actually works against /mcp.
    let res = app
        .clone()
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::from(
                    serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(json_body(res).await["result"]["tools"].as_array().unwrap().len() > 40);

    // And it shows up in Settings under the client's name, so the user can
    // revoke the connector later.
    let listed = json_body(
        app.oneshot(
            Request::get("/api/auth/tokens")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["name"], "Claude");
    assert_eq!(listed[0]["scope"], "full");
}

#[tokio::test]
async fn a_read_scoped_consent_issues_a_read_token() {
    let (app, cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();

    let redirect_to = json_body(
        app.clone()
            .oneshot(approve_req(&cookie, &client_id, "read", &challenge_for(VERIFIER), None))
            .await
            .unwrap(),
    )
    .await["redirectTo"]
        .as_str()
        .unwrap()
        .to_string();

    let body = json_body(
        app.oneshot(token_req(&code_from(&redirect_to), &client_id, VERIFIER, REDIRECT_URI))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["scope"], "read");
}

// ---------------------------------------------------------- failure modes ---

#[tokio::test]
async fn pkce_mismatch_is_rejected_and_burns_the_code() {
    let (app, cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let redirect_to = json_body(
        app.clone()
            .oneshot(approve_req(&cookie, &client_id, "full", &challenge_for(VERIFIER), None))
            .await
            .unwrap(),
    )
    .await["redirectTo"]
        .as_str()
        .unwrap()
        .to_string();
    let code = code_from(&redirect_to);

    let res = app
        .clone()
        .oneshot(token_req(&code, &client_id, "the-wrong-verifier-entirely", REDIRECT_URI))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["error"], "invalid_grant");

    // The code is spent even though the exchange failed: a code presented with
    // a bad verifier is evidence of interception, not a typo worth retrying.
    let res = app
        .oneshot(token_req(&code, &client_id, VERIFIER, REDIRECT_URI))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["error"], "invalid_grant");
}

#[tokio::test]
async fn an_authorization_code_is_single_use() {
    let (app, cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let redirect_to = json_body(
        app.clone()
            .oneshot(approve_req(&cookie, &client_id, "full", &challenge_for(VERIFIER), None))
            .await
            .unwrap(),
    )
    .await["redirectTo"]
        .as_str()
        .unwrap()
        .to_string();
    let code = code_from(&redirect_to);

    let res = app
        .clone()
        .oneshot(token_req(&code, &client_id, VERIFIER, REDIRECT_URI))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(token_req(&code, &client_id, VERIFIER, REDIRECT_URI))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["error"], "invalid_grant");
}

#[tokio::test]
async fn the_exchange_binds_to_the_client_and_redirect_it_was_issued_for() {
    let (app, cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let other = register_client(&app, serde_json::json!(["https://other.example/cb"])).await;
    let other_id = other["client_id"].as_str().unwrap().to_string();

    // Wrong client_id.
    let code = code_from(
        json_body(
            app.clone()
                .oneshot(approve_req(&cookie, &client_id, "full", &challenge_for(VERIFIER), None))
                .await
                .unwrap(),
        )
        .await["redirectTo"]
            .as_str()
            .unwrap(),
    );
    let res = app
        .clone()
        .oneshot(token_req(&code, &other_id, VERIFIER, REDIRECT_URI))
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_grant");

    // Wrong redirect_uri.
    let code = code_from(
        json_body(
            app.clone()
                .oneshot(approve_req(&cookie, &client_id, "full", &challenge_for(VERIFIER), None))
                .await
                .unwrap(),
        )
        .await["redirectTo"]
            .as_str()
            .unwrap(),
    );
    let res = app
        .oneshot(token_req(&code, &client_id, VERIFIER, "https://other.example/cb"))
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_grant");
}

/// A loose redirect check is the classic open redirect that leaks codes.
#[tokio::test]
async fn consent_requires_an_exactly_registered_redirect_uri() {
    let (app, cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();

    let mut req = serde_json::json!({
        "clientId": client_id,
        "redirectUri": "https://claude.ai/api/mcp/auth_callback/extra",
        "scope": "full",
        "codeChallenge": challenge_for(VERIFIER),
        "codeChallengeMethod": "S256",
    });
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/oauth/approve")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["error"], "invalid_request");

    // An unknown client is refused too.
    req["clientId"] = serde_json::json!("00000000-0000-0000-0000-000000000000");
    req["redirectUri"] = serde_json::json!(REDIRECT_URI);
    let res = app
        .oneshot(
            Request::post("/api/oauth/approve")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_client");
}

#[tokio::test]
async fn consent_rejects_plain_pkce_and_bad_scopes() {
    let (app, cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();

    // OAuth 2.1 forbids `plain`.
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/oauth/approve")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "clientId": client_id, "redirectUri": REDIRECT_URI, "scope": "full",
                        "codeChallenge": challenge_for(VERIFIER), "codeChallengeMethod": "plain",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_request");

    // A malformed challenge (not 43 base64url chars) is refused.
    let res = app
        .clone()
        .oneshot(approve_req(&cookie, &client_id, "full", "too-short", None))
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_request");

    let res = app
        .oneshot(approve_req(&cookie, &client_id, "admin", &challenge_for(VERIFIER), None))
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_scope");
}

#[tokio::test]
async fn consent_requires_a_logged_in_session() {
    let (app, _cookie) = setup().await;
    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();

    let res = app
        .oneshot(
            Request::post("/api/oauth/approve")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "clientId": client_id, "redirectUri": REDIRECT_URI, "scope": "full",
                        "codeChallenge": challenge_for(VERIFIER), "codeChallengeMethod": "S256",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn registration_enforces_the_redirect_uri_policy() {
    let (app, _cookie) = setup().await;

    for bad in [
        serde_json::json!([]),
        serde_json::json!(["http://evil.example.com/cb"]),
        serde_json::json!(["javascript:alert(1)"]),
        serde_json::json!(["https://example.com/cb#frag"]),
        serde_json::json!(["not a url"]),
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/oauth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"client_name": "x", "redirect_uris": bad}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "should reject {bad}");
        assert_eq!(json_body(res).await["error"], "invalid_redirect_uri");
    }

    // Loopback HTTP is allowed — local bridges can't get a certificate.
    register_client(&app, serde_json::json!(["http://127.0.0.1:33418/oauth/callback"])).await;
}

#[tokio::test]
async fn unsupported_grant_types_are_refused() {
    let (app, _cookie) = setup().await;
    // `authorization_code` and `refresh_token` are the two we implement.
    // Anything else — notably the ones that skip user consent entirely — must be
    // refused as a grant type rather than falling through to a token lookup.
    for grant in ["client_credentials", "password", "implicit", "urn:ietf:params:oauth:grant-type:device_code"] {
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/oauth/token")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!("grant_type={grant}")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{grant} must be refused");
        assert_eq!(
            json_body(res).await["error"],
            "unsupported_grant_type",
            "{grant} must be refused as a grant type, not as a bad credential"
        );
    }
}

#[tokio::test]
async fn an_unknown_code_is_invalid_grant() {
    let (app, _cookie) = setup().await;
    let res = app
        .oneshot(token_req(
            "finsight_code_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "some-client",
            VERIFIER,
            REDIRECT_URI,
        ))
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"], "invalid_grant");
}

/// Recovery must kill OAuth-issued tokens too — they are ordinary PATs, and the
/// point of recovery is that every outstanding credential stops working.
#[tokio::test]
async fn recovery_revokes_oauth_issued_tokens() {
    let dir = tempfile::tempdir().unwrap().keep();
    let state = ServerState::bootstrap(&dir).unwrap();
    let app = build_router(state, &test_ui_dir());
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/setup")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"alice","password":"hunter22-plus"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = cookie_from(&res);
    let recovery_key = json_body(res).await["recoveryKey"].as_str().unwrap().to_string();

    let client = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let redirect_to = json_body(
        app.clone()
            .oneshot(approve_req(&cookie, &client_id, "full", &challenge_for(VERIFIER), None))
            .await
            .unwrap(),
    )
    .await["redirectTo"]
        .as_str()
        .unwrap()
        .to_string();
    let access_token = json_body(
        app.clone()
            .oneshot(token_req(&code_from(&redirect_to), &client_id, VERIFIER, REDIRECT_URI))
            .await
            .unwrap(),
    )
    .await["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/recover")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "recoveryKey": recovery_key,
                        "newPassword": "brand-new-password-9",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::from(
                    serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "a connector's token must not survive password recovery"
    );
}

// ------------------------------------------------------- refresh tokens ---

/// Run the full consent + code exchange and return the whole token response, so
/// refresh tests start from a real grant rather than a hand-built row.
async fn grant_token_pair(app: &App, cookie: &str) -> (String, serde_json::Value) {
    let client = register_client(app, serde_json::json!([REDIRECT_URI])).await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let approved = json_body(
        app.clone()
            .oneshot(approve_req(cookie, &client_id, "full", &challenge_for(VERIFIER), None))
            .await
            .unwrap(),
    )
    .await;
    let code = code_from(approved["redirectTo"].as_str().unwrap());
    let body = json_body(
        app.clone()
            .oneshot(token_req(&code, &client_id, VERIFIER, REDIRECT_URI))
            .await
            .unwrap(),
    )
    .await;
    (client_id, body)
}

fn refresh_req(refresh_token: &str, client_id: &str) -> Request<Body> {
    Request::post("/api/oauth/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!(
            "grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}"
        )))
        .unwrap()
}

async fn mcp_ping(app: &App, token: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({"jsonrpc":"2.0","id":1,"method":"ping"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn code_exchange_issues_a_short_lived_token_with_a_refresh_token() {
    let (app, cookie) = setup().await;
    let (_client_id, body) = grant_token_pair(&app, &cookie).await;

    assert!(
        body["access_token"].as_str().unwrap().starts_with("finsight_pat_"),
        "access token keeps the PAT shape: {body}"
    );
    assert!(
        body["refresh_token"].as_str().unwrap().starts_with("finsight_rt_"),
        "a refresh token must be issued so expiry is recoverable: {body}"
    );
    // Short-lived access tokens are only acceptable because renewal is silent;
    // a missing or absurd expiry would mean one of those two halves is broken.
    let expires_in = body["expires_in"].as_i64().expect("expires_in must be present");
    assert!(
        (60..=86_400).contains(&expires_in),
        "expires_in should be short-lived but usable, got {expires_in}"
    );
}

#[tokio::test]
async fn refreshing_returns_a_new_working_pair_and_retires_the_old_one() {
    let (app, cookie) = setup().await;
    let (client_id, first) = grant_token_pair(&app, &cookie).await;
    let old_access = first["access_token"].as_str().unwrap().to_string();
    let old_refresh = first["refresh_token"].as_str().unwrap().to_string();

    assert_eq!(mcp_ping(&app, &old_access).await, StatusCode::OK);

    let second = json_body(
        app.clone().oneshot(refresh_req(&old_refresh, &client_id)).await.unwrap(),
    )
    .await;
    let new_access = second["access_token"].as_str().unwrap().to_string();
    let new_refresh = second["refresh_token"].as_str().unwrap().to_string();

    assert_ne!(new_access, old_access, "refresh must mint a NEW access token");
    assert_ne!(new_refresh, old_refresh, "refresh tokens must rotate");
    assert_eq!(second["scope"], "full", "scope carries across a refresh");

    assert_eq!(
        mcp_ping(&app, &new_access).await,
        StatusCode::OK,
        "the refreshed token must work"
    );
    // Retiring the old access token is what stops every renewal widening the
    // set of live credentials.
    assert_eq!(
        mcp_ping(&app, &old_access).await,
        StatusCode::UNAUTHORIZED,
        "the superseded access token must stop working"
    );
}

#[tokio::test]
async fn a_refresh_token_is_single_use() {
    let (app, cookie) = setup().await;
    let (client_id, first) = grant_token_pair(&app, &cookie).await;
    let refresh = first["refresh_token"].as_str().unwrap().to_string();

    let ok = app.clone().oneshot(refresh_req(&refresh, &client_id)).await.unwrap();
    assert_eq!(ok.status(), StatusCode::OK);

    // Rotation (OAuth 2.1 §4.3.1 for public clients): replaying a refresh token
    // is the signature of a stolen one, so it must never work twice.
    let replay = app.clone().oneshot(refresh_req(&refresh, &client_id)).await.unwrap();
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(replay).await["error"], "invalid_grant");
}

#[tokio::test]
async fn a_refresh_token_is_bound_to_its_client() {
    let (app, cookie) = setup().await;
    let (_client_id, first) = grant_token_pair(&app, &cookie).await;
    let refresh = first["refresh_token"].as_str().unwrap().to_string();

    let other = register_client(&app, serde_json::json!([REDIRECT_URI])).await;
    let other_id = other["client_id"].as_str().unwrap();

    let res = app.clone().oneshot(refresh_req(&refresh, other_id)).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "another registered client must not redeem this grant"
    );
    assert_eq!(json_body(res).await["error"], "invalid_grant");
}

#[tokio::test]
async fn garbage_refresh_tokens_are_rejected() {
    let (app, cookie) = setup().await;
    let (client_id, _) = grant_token_pair(&app, &cookie).await;

    for bogus in ["finsight_rt_not-real", "not-even-prefixed", ""] {
        let res = app.clone().oneshot(refresh_req(bogus, &client_id)).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{bogus} must be refused");
    }
}

#[tokio::test]
async fn revoking_the_access_token_also_kills_its_refresh_token() {
    let (app, cookie) = setup().await;
    let (client_id, first) = grant_token_pair(&app, &cookie).await;
    let refresh = first["refresh_token"].as_str().unwrap().to_string();

    // Find the connector's token in the user's list and revoke it, the way the
    // Settings screen does.
    let list = json_body(
        app.clone()
            .oneshot(
                Request::get("/api/auth/tokens")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let id = list.as_array().unwrap()[0]["id"].as_str().unwrap().to_string();
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/auth/tokens/{id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Revocation that leaves a usable refresh token behind is not revocation:
    // the connector would mint itself a fresh access token minutes later.
    let res = app.clone().oneshot(refresh_req(&refresh, &client_id)).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "a revoked connector must not be able to refresh its way back in"
    );
}

#[tokio::test]
async fn a_refresh_grant_requires_the_client_id() {
    let (app, cookie) = setup().await;
    let (_client_id, first) = grant_token_pair(&app, &cookie).await;
    let refresh = first["refresh_token"].as_str().unwrap().to_string();

    // Omitting client_id must not skip the binding. A check you can bypass by
    // leaving out the field is not a check.
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/oauth/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "grant_type=refresh_token&refresh_token={refresh}"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "a refresh grant with no client_id must be refused, not silently accepted"
    );
    assert_eq!(json_body(res).await["error"], "invalid_grant");
}

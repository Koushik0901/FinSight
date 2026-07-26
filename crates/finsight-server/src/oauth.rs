//! OAuth 2.1 authorization-server surface for MCP clients.
//!
//! This module currently carries the *discovery* half: the origin derivation
//! every absolute URL depends on, plus the two metadata documents an MCP client
//! fetches before it will attempt authorization (RFC 8414 authorization-server
//! metadata and RFC 9728 protected-resource metadata). They ship alongside
//! `/mcp` rather than with the interactive flow so the `WWW-Authenticate`
//! challenge on a 401 always points at a document that actually exists —
//! otherwise a client's discovery chain dead-ends on a 404 instead of
//! explaining how to authorize.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;

use crate::state::ServerState;

/// Operator override for the externally-visible origin. Needed whenever the
/// server can't infer it from request headers — the usual case being a reverse
/// proxy that rewrites `Host` or terminates TLS without setting
/// `X-Forwarded-Proto`, where inference yields `http://` and cloud connectors
/// refuse to authorize against a non-HTTPS issuer.
pub const PUBLIC_ORIGIN_ENV: &str = "FINSIGHT_PUBLIC_ORIGIN";

/// The externally-visible origin (`scheme://authority`), no trailing slash.
///
/// Falls back to `X-Forwarded-Proto` + `Host` because FinSight is designed to
/// sit behind Caddy/Tailscale/nginx (see `docs/self-hosting.md`) — the process
/// itself only ever speaks plain HTTP on its port, so it cannot know its own
/// public scheme without being told.
pub fn public_origin(headers: &HeaderMap) -> String {
    if let Ok(explicit) = std::env::var(PUBLIC_ORIGIN_ENV) {
        let trimmed = explicit.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        // A proxy chain can send a comma-separated list; the first hop is ours.
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("http");
    let authority = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("localhost");
    format!("{scheme}://{authority}")
}

/// RFC 8414 — what the client needs to run the authorization code flow.
pub(crate) async fn as_metadata(headers: HeaderMap) -> Response {
    let origin = public_origin(&headers);
    Json(serde_json::json!({
        "issuer": origin,
        // The authorize endpoint is an SPA route, not a server route: consent
        // needs a logged-in browser session and a rendered card, both of which
        // already exist in the frontend.
        "authorization_endpoint": format!("{origin}/oauth/authorize"),
        "token_endpoint": format!("{origin}/api/oauth/token"),
        "registration_endpoint": format!("{origin}/api/oauth/register"),
        "response_types_supported": ["code"],
        // `refresh_token` is what lets access tokens be short-lived, which the
        // 2025-11-25 spec asks for; a client that sees it here knows expiry is
        // recoverable rather than a dead connection.
        "grant_types_supported": ["authorization_code", "refresh_token"],
        // PKCE is mandatory in OAuth 2.1 and `plain` is forbidden.
        "code_challenge_methods_supported": ["S256"],
        // Public clients only: a desktop app or browser extension cannot keep
        // a secret, so we never issue or check one.
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": [crate::tokens::SCOPE_READ, crate::tokens::SCOPE_FULL],
        "service_documentation": "https://github.com/Koushik0901/FinSight/blob/main/docs/self-hosting.md",
    }))
    .into_response()
}

/// RFC 9728 — which authorization server guards `/mcp`. This is the document
/// the `WWW-Authenticate` challenge on a 401 points at.
pub(crate) async fn prm_metadata(headers: HeaderMap) -> Response {
    let origin = public_origin(&headers);
    Json(serde_json::json!({
        "resource": format!("{origin}/mcp"),
        "authorization_servers": [origin],
        "bearer_methods_supported": ["header"],
        "scopes_supported": [crate::tokens::SCOPE_READ, crate::tokens::SCOPE_FULL],
    }))
    .into_response()
}

// ------------------------------------------------- client registration ---

/// RFC 7591 open registration is unauthenticated by design — a connector must
/// be able to register before any user has consented to anything — so the table
/// needs a ceiling to stop it being a free write primitive.
const MAX_REGISTERED_CLIENTS: i64 = 500;
/// Authorization codes are exchanged within seconds of being issued; ten
/// minutes is the RFC 6749 §4.1.2 recommended maximum and is generous here.
const CODE_TTL_SECS: i64 = 600;
const CODE_PREFIX: &str = "finsight_code_";

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error": error, "error_description": description})),
    )
        .into_response()
}

/// A redirect URI we're willing to send an authorization code to.
///
/// HTTPS anywhere, or plain HTTP only on loopback — that carve-out exists for
/// local bridges like `mcp-remote`, which spin up a throwaway listener on
/// 127.0.0.1 and cannot obtain a certificate for it.
fn redirect_uri_is_allowed(uri: &str) -> bool {
    let Ok(parsed) = url::Url::parse(uri) else {
        return false;
    };
    // A fragment is forbidden on a redirect URI (RFC 6749 §3.1.2) — it would be
    // dropped by the browser and silently break the callback.
    if parsed.fragment().is_some() {
        return false;
    }
    match parsed.scheme() {
        "https" => parsed.host().is_some(),
        "http" => matches!(
            parsed.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        ),
        _ => false,
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct RegisterRequest {
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
}

pub(crate) async fn register(
    State(st): State<Arc<ServerState>>,
    Json(body): Json<RegisterRequest>,
) -> Response {
    if body.redirect_uris.is_empty() {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "at least one redirect_uri is required",
        );
    }
    if body.redirect_uris.len() > 10 {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "too many redirect URIs",
        );
    }
    for uri in &body.redirect_uris {
        if !redirect_uri_is_allowed(uri) {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_redirect_uri",
                "redirect URIs must be https, or http on loopback",
            );
        }
    }
    match st.users.count_oauth_clients() {
        Ok(n) if n >= MAX_REGISTERED_CLIENTS => {
            tracing::warn!(count = n, "OAuth client registration refused: registry is full");
            return oauth_error(
                StatusCode::TOO_MANY_REQUESTS,
                "invalid_client_metadata",
                "this server has reached its registered-client limit",
            );
        }
        Err(e) => {
            return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string())
        }
        Ok(_) => {}
    }

    let client_id = uuid::Uuid::new_v4().to_string();
    let client_name = body
        .client_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unnamed client")
        .chars()
        .take(64)
        .collect::<String>();
    let uris_json = serde_json::to_string(&body.redirect_uris).unwrap_or_else(|_| "[]".into());

    if let Err(e) = st
        .users
        .insert_oauth_client(&client_id, &client_name, &uris_json)
    {
        return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string());
    }
    tracing::info!(%client_id, %client_name, "registered OAuth client");

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "client_id": client_id,
            "client_name": client_name,
            "redirect_uris": body.redirect_uris,
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
        })),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
pub(crate) struct ClientQuery {
    client_id: String,
}

/// Lets the consent screen name the app asking for access. Deliberately returns
/// only the display name — never the redirect URIs, which would turn this into
/// a way to enumerate what a registered client is allowed to do.
pub(crate) async fn client_info(
    State(st): State<Arc<ServerState>>,
    axum::extract::Query(q): axum::extract::Query<ClientQuery>,
) -> Response {
    match st.users.get_oauth_client(&q.client_id) {
        Ok(Some(c)) => Json(serde_json::json!({"clientName": c.client_name})).into_response(),
        Ok(None) => oauth_error(StatusCode::NOT_FOUND, "invalid_client", "unknown client_id"),
        Err(e) => oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string()),
    }
}

// -------------------------------------------------------------- consent ---

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApproveRequest {
    client_id: String,
    redirect_uri: String,
    scope: String,
    state: Option<String>,
    code_challenge: String,
    code_challenge_method: String,
}

/// The user has consented in the SPA. Mint a single-use authorization code and
/// hand back where to send the browser.
///
/// Session-cookie authenticated: consent is an act by a logged-in human, and
/// the DB key that ends up wrapped into the code comes from their session.
pub(crate) async fn approve(
    State(st): State<Arc<ServerState>>,
    user: crate::auth::AuthedUser,
    Json(body): Json<ApproveRequest>,
) -> Response {
    // OAuth 2.1 requires PKCE and forbids `plain`.
    if body.code_challenge_method != "S256" {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "code_challenge_method must be S256",
        );
    }
    // 43 chars is the fixed length of base64url(SHA-256) without padding.
    if body.code_challenge.len() != 43
        || !body
            .code_challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "code_challenge must be base64url-encoded SHA-256",
        );
    }
    if body.scope != crate::tokens::SCOPE_READ && body.scope != crate::tokens::SCOPE_FULL {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_scope", "scope must be 'read' or 'full'");
    }

    let client = match st.users.get_oauth_client(&body.client_id) {
        Ok(Some(c)) => c,
        Ok(None) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_client", "unknown client_id"),
        Err(e) => {
            return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string())
        }
    };
    // Exact string match, not a prefix or host comparison: anything looser is
    // the classic open-redirect that leaks authorization codes.
    if !client.redirect_uris.iter().any(|u| u == &body.redirect_uri) {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "redirect_uri does not match this client's registration",
        );
    }

    let Some(dbkey) = crate::tokens::db_key_from_hex(&user.db_key_hex) else {
        return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "session key is malformed");
    };
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    let code = format!(
        "{CODE_PREFIX}{}",
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, raw)
    );
    // The DB key travels through the exchange wrapped under the code's own
    // bytes, so a stolen oauth_codes row is useless without the code itself.
    let wrapped = match crate::crypto::wrap_key_with_token(&raw, &dbkey) {
        Ok(w) => w,
        Err(e) => {
            return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string())
        }
    };

    if let Err(e) = st.users.insert_oauth_code(
        &crate::crypto::hash_session_token(&code),
        &body.client_id,
        &body.redirect_uri,
        &body.code_challenge,
        &user.user_id,
        &wrapped,
        &body.scope,
        chrono::Utc::now().timestamp() + CODE_TTL_SECS,
    ) {
        return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string());
    }

    let sep = if body.redirect_uri.contains('?') { '&' } else { '?' };
    let mut redirect_to = format!("{}{sep}code={code}", body.redirect_uri);
    if let Some(state) = body.state.as_deref() {
        redirect_to.push_str(&format!("&state={}", urlencode(state)));
    }
    tracing::info!(user_id = %user.user_id, client_id = %body.client_id, scope = %body.scope, "OAuth consent granted");
    Json(serde_json::json!({ "redirectTo": redirect_to })).into_response()
}

/// Percent-encodes everything outside the RFC 3986 unreserved set. `state` is
/// opaque client data, so it can legitimately contain `&`, `=`, or `#` — none
/// of which may survive into the query string unescaped.
fn urlencode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for b in raw.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// -------------------------------------------------------------- tokens ---

#[derive(serde::Deserialize)]
pub(crate) struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    code_verifier: String,
    #[serde(default)]
    refresh_token: String,
}

/// Exchange an authorization code for a bearer token. Form-encoded because
/// that is what OAuth clients send; unknown fields (`resource`, etc.) are
/// ignored rather than rejected.
pub(crate) async fn token(
    State(st): State<Arc<ServerState>>,
    axum::extract::Form(body): axum::extract::Form<TokenRequest>,
) -> Response {
    match body.grant_type.as_str() {
        "authorization_code" => {}
        "refresh_token" => return refresh_grant(st, body),
        _ => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
                "only authorization_code and refresh_token are supported",
            )
        }
    }

    let invalid_grant = || {
        oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "the authorization code is invalid, expired, or already used",
        )
    };

    // Consume first: a failed exchange must burn the code rather than leave it
    // available for another attempt (RFC 6749 §4.1.2 — a code presented with a
    // bad verifier is evidence of interception, not of a typo worth retrying).
    let code_hash = crate::crypto::hash_session_token(&body.code);
    let rec = match st
        .users
        .consume_oauth_code(&code_hash, chrono::Utc::now().timestamp())
    {
        Ok(Some(r)) => r,
        Ok(None) => return invalid_grant(),
        Err(e) => {
            return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string())
        }
    };

    if rec.client_id != body.client_id || rec.redirect_uri != body.redirect_uri {
        tracing::warn!(client_id = %body.client_id, "OAuth token exchange with mismatched client_id/redirect_uri");
        return invalid_grant();
    }

    // PKCE: proves the caller is the same party that started the flow, which is
    // the only thing standing in for a client secret here.
    let digest = <sha2::Sha256 as sha2::Digest>::digest(body.code_verifier.as_bytes());
    let expected =
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, digest);
    if expected != rec.code_challenge {
        tracing::warn!(client_id = %body.client_id, "OAuth token exchange failed PKCE verification");
        return invalid_grant();
    }

    let Some(raw) = parse_code(&body.code) else {
        return invalid_grant();
    };
    let Ok(dbkey) = crate::crypto::unwrap_key_with_token(&raw, &rec.wrapped_db_key) else {
        return invalid_grant();
    };

    let client_name = st
        .users
        .get_oauth_client(&rec.client_id)
        .ok()
        .flatten()
        .map(|c| c.client_name)
        .unwrap_or_else(|| "OAuth client".to_string());

    match crate::tokens::issue_token_pair(
        &st,
        &rec.user_id,
        &rec.client_id,
        &client_name,
        &rec.scope,
        &dbkey,
    ) {
        Ok((access_token, refresh_token, expires_in)) => {
            tracing::info!(user_id = %rec.user_id, client_id = %rec.client_id, scope = %rec.scope, "OAuth token issued");
            token_response(&access_token, &refresh_token, expires_in, &rec.scope)
        }
        // `issue_token` already produced an AppError-shaped response, but this
        // endpoint owes the client an OAuth-shaped one.
        Err(_) => oauth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "could not issue an access token",
        ),
    }
}

fn token_response(
    access_token: &str,
    refresh_token: &str,
    expires_in: i64,
    scope: &str,
) -> Response {
    (
        StatusCode::OK,
        // Never cached: this response body is a credential.
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": expires_in,
            "refresh_token": refresh_token,
            "scope": scope,
        })),
    )
        .into_response()
}

/// `grant_type=refresh_token`. Rotates: the presented refresh token is consumed
/// and the access token it minted is retired, both before the replacement pair
/// exists, so a leaked refresh token buys exactly one renewal and the theft
/// surfaces as the real client being logged out.
fn refresh_grant(st: Arc<ServerState>, body: TokenRequest) -> Response {
    let invalid_grant = || {
        oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "the refresh token is invalid, already used, or revoked",
        )
    };

    let Some(raw) = crate::tokens::parse_refresh_token(&body.refresh_token) else {
        return invalid_grant();
    };
    let hash = crate::crypto::hash_session_token(&body.refresh_token);
    let rec = match st.users.consume_refresh_token(&hash) {
        Ok(Some(r)) => r,
        Ok(None) => return invalid_grant(),
        Err(e) => {
            return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", &e.to_string())
        }
    };

    // A public client is identified only by the token it presents, so the
    // client_id must match the one the grant was issued to; otherwise any
    // registered client could redeem another's stolen refresh token.
    if !body.client_id.is_empty() && body.client_id != rec.client_id {
        tracing::warn!(client_id = %body.client_id, "refresh token presented by a different client");
        return invalid_grant();
    }

    // The AEAD tag is the real check: only the genuine token bytes unwrap it.
    let Ok(dbkey) = crate::crypto::unwrap_key_with_token(&raw, &rec.wrapped_db_key) else {
        return invalid_grant();
    };

    let client_name = st
        .users
        .get_oauth_client(&rec.client_id)
        .ok()
        .flatten()
        .map(|c| c.client_name)
        .unwrap_or_else(|| "OAuth client".to_string());

    match crate::tokens::issue_token_pair(
        &st,
        &rec.user_id,
        &rec.client_id,
        &client_name,
        &rec.scope,
        &dbkey,
    ) {
        Ok((access_token, refresh_token, expires_in)) => {
            tracing::info!(user_id = %rec.user_id, client_id = %rec.client_id, "OAuth token refreshed");
            token_response(&access_token, &refresh_token, expires_in, &rec.scope)
        }
        Err(_) => oauth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "could not issue an access token",
        ),
    }
}

fn parse_code(code: &str) -> Option<[u8; 32]> {
    let body = code.strip_prefix(CODE_PREFIX)?;
    let bytes =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, body).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    // These tests mutate a process-global env var, so they must not run
    // concurrently with each other; `cargo test` threads them by default.
    // Serialized by keeping every env-touching assertion in ONE test.
    #[test]
    fn public_origin_prefers_the_explicit_override() {
        std::env::set_var(PUBLIC_ORIGIN_ENV, "https://fin.example.com/");
        assert_eq!(
            public_origin(&headers(&[("host", "internal:8674")])),
            "https://fin.example.com",
            "the override wins over headers, and its trailing slash is trimmed"
        );

        // Blank override is treated as unset rather than yielding "".
        std::env::set_var(PUBLIC_ORIGIN_ENV, "   ");
        assert_eq!(
            public_origin(&headers(&[("host", "fin.example.com")])),
            "http://fin.example.com"
        );

        std::env::remove_var(PUBLIC_ORIGIN_ENV);
        assert_eq!(
            public_origin(&headers(&[
                ("host", "fin.example.com"),
                ("x-forwarded-proto", "https"),
            ])),
            "https://fin.example.com"
        );
        // Proxy chains send a list; the first hop is the one facing the client.
        assert_eq!(
            public_origin(&headers(&[
                ("host", "fin.example.com"),
                ("x-forwarded-proto", "https, http"),
            ])),
            "https://fin.example.com"
        );
        // No proxy headers at all: plain HTTP on the bound port.
        assert_eq!(
            public_origin(&headers(&[("host", "localhost:8674")])),
            "http://localhost:8674"
        );
        assert_eq!(public_origin(&HeaderMap::new()), "http://localhost");
    }

    #[test]
    fn redirect_uri_policy_allows_https_and_loopback_http_only() {
        assert!(redirect_uri_is_allowed("https://claude.ai/api/mcp/auth_callback"));
        assert!(redirect_uri_is_allowed("https://example.com:8443/cb"));
        // Local bridges (mcp-remote) listen on an ephemeral loopback port and
        // cannot get a certificate for it.
        assert!(redirect_uri_is_allowed("http://localhost:33418/oauth/callback"));
        assert!(redirect_uri_is_allowed("http://127.0.0.1:5000/cb"));

        // Plain HTTP to anywhere else would leak the code over the wire.
        assert!(!redirect_uri_is_allowed("http://evil.example.com/cb"));
        // A fragment is forbidden — the browser drops it, so the callback would
        // silently never receive the code.
        assert!(!redirect_uri_is_allowed("https://example.com/cb#frag"));
        assert!(!redirect_uri_is_allowed("javascript:alert(1)"));
        assert!(!redirect_uri_is_allowed("data:text/html,x"));
        assert!(!redirect_uri_is_allowed("not a url"));
        assert!(!redirect_uri_is_allowed(""));
    }

    #[test]
    fn state_is_percent_encoded_so_it_cannot_forge_query_parameters() {
        assert_eq!(urlencode("abc-123_x.y~z"), "abc-123_x.y~z");
        assert_eq!(urlencode("a b"), "a%20b");
        // The important case: an opaque state must not be able to inject its
        // own parameters into the redirect.
        assert_eq!(
            urlencode("x&code=stolen"),
            "x%26code%3Dstolen"
        );
        assert_eq!(urlencode("#frag"), "%23frag");
    }

    #[test]
    fn code_parsing_round_trips_and_rejects_junk() {
        let raw = [7u8; 32];
        let code = format!(
            "{CODE_PREFIX}{}",
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, raw)
        );
        assert_eq!(parse_code(&code), Some(raw));
        assert!(parse_code("finsight_code_short").is_none());
        assert!(parse_code("wrong_prefix_AAAA").is_none());
        assert!(parse_code("").is_none());
    }
}

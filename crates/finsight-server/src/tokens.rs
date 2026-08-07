//! Personal access tokens (PATs) for MCP clients, plus the session-authed REST
//! endpoints that manage them (`/api/auth/tokens`). Plain REST like `auth.rs` —
//! never appears in `bindings.ts`.
//!
//! Token design: 32 random bytes, printed with a recognizable prefix. Those raw
//! bytes ARE the key-encryption key that wraps the user's SQLCipher key (see
//! `crypto::wrap_key_with_token`), so a bearer token alone unlocks the account's
//! data — and the server stores only the token's SHA-256 hash, so a stolen
//! `users.db` yields neither a usable token nor a DB key. That is the same
//! tradeoff the recovery key already makes, and it is why a token is shown
//! exactly once and revocation must be easy.

use crate::auth::AuthedUser;
use crate::state::ServerState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use finsight_api::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub(crate) const PAT_PREFIX: &str = "finsight_pat_";
const MAX_TOKEN_NAME_LEN: usize = 64;

/// The two scopes a token can carry. `read` is the safe default to hand an
/// external assistant: it cannot draft, annotate, approve, or execute anything.
pub(crate) const SCOPE_READ: &str = "read";
pub(crate) const SCOPE_FULL: &str = "full";

fn err_response(status: StatusCode, code: &str, msg: impl Into<String>) -> Response {
    (status, Json(AppError::new(code, msg.into()))).into_response()
}

/// Mint a fresh token: `(printable token, raw KEK bytes)`. The caller wraps the
/// DB key with the bytes and stores only `crypto::hash_session_token(&token)`.
pub(crate) fn generate_pat() -> (String, [u8; 32]) {
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    (format!("{PAT_PREFIX}{}", URL_SAFE_NO_PAD.encode(raw)), raw)
}

/// Recover the KEK bytes from a presented token string. `None` for anything
/// that isn't our prefix + exactly 32 base64url bytes — the cheap structural
/// check before any DB work.
pub(crate) fn parse_pat(token: &str) -> Option<[u8; 32]> {
    let body = token.strip_prefix(PAT_PREFIX)?;
    let bytes = URL_SAFE_NO_PAD.decode(body).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}

/// Decode the session's 64-hex SQLCipher key back to bytes so it can be
/// re-wrapped under a new KEK.
pub(crate) fn db_key_from_hex(hex_key: &str) -> Option<[u8; crate::crypto::DB_KEY_LEN]> {
    let bytes = hex::decode(hex_key).ok()?;
    <[u8; crate::crypto::DB_KEY_LEN]>::try_from(bytes.as_slice()).ok()
}

/// Prefix for refresh tokens. Distinct from `PAT_PREFIX` so a client that
/// swaps the two gets a clean rejection at the structural check rather than a
/// confusing hash miss.
pub(crate) const REFRESH_PREFIX: &str = "finsight_rt_";

/// How long an OAuth-issued access token lives. Short-lived access tokens are a
/// SHOULD in the MCP authorization spec, and they only cost the user something
/// if renewal is manual — which is exactly what the paired refresh token
/// removes. An hour is long enough that a chatty session never notices and
/// short enough that a leaked token is a limited window.
pub(crate) const ACCESS_TOKEN_TTL_SECS: i64 = 3600;

/// Mint a refresh token: `(printable token, raw KEK bytes)`.
pub(crate) fn generate_refresh_token() -> (String, [u8; 32]) {
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    (
        format!("{REFRESH_PREFIX}{}", URL_SAFE_NO_PAD.encode(raw)),
        raw,
    )
}

pub(crate) fn parse_refresh_token(token: &str) -> Option<[u8; 32]> {
    let body = token.strip_prefix(REFRESH_PREFIX)?;
    let bytes = URL_SAFE_NO_PAD.decode(body).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}

/// Shared by the REST create handler and the OAuth token endpoint: wrap `dbkey`
/// under a fresh PAT and record it. Returns the printable token, shown once.
///
/// `expires_unix` is `None` for a Settings-created PAT (a human pasted it into
/// a config file and expects it to keep working) and `Some` for anything issued
/// through OAuth, where a refresh token makes expiry invisible.
pub(crate) fn issue_token(
    st: &ServerState,
    user_id: &str,
    name: &str,
    scope: &str,
    dbkey: &[u8; crate::crypto::DB_KEY_LEN],
    expires_unix: Option<i64>,
) -> Result<(String, crate::users::ApiTokenRecord), Box<Response>> {
    let (token, raw) = generate_pat();
    let wrapped = crate::crypto::wrap_key_with_token(&raw, dbkey).map_err(|e| {
        Box::new(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "auth.crypto",
            e.to_string(),
        ))
    })?;
    let hash = crate::crypto::hash_session_token(&token);
    let rec = st
        .users
        .insert_api_token(user_id, name, scope, &hash, &wrapped, expires_unix)
        .map_err(|e| {
            Box::new(err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth.db",
                e.to_string(),
            ))
        })?;
    Ok((token, rec))
}

/// Issue an access token paired with a refresh token, the shape an OAuth client
/// gets. The refresh token wraps its own copy of the DB key, so renewal never
/// requires the server to have kept the key in the clear between calls.
pub(crate) fn issue_token_pair(
    st: &ServerState,
    user_id: &str,
    client_id: &str,
    name: &str,
    scope: &str,
    dbkey: &[u8; crate::crypto::DB_KEY_LEN],
) -> Result<(String, String, i64), Box<Response>> {
    let expires_at = chrono::Utc::now().timestamp() + ACCESS_TOKEN_TTL_SECS;
    let (access_token, rec) = issue_token(st, user_id, name, scope, dbkey, Some(expires_at))?;

    let (refresh_token, refresh_raw) = generate_refresh_token();
    let wrapped = crate::crypto::wrap_key_with_token(&refresh_raw, dbkey).map_err(|e| {
        Box::new(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "auth.crypto",
            e.to_string(),
        ))
    })?;
    st.users
        .insert_refresh_token(
            &crate::crypto::hash_session_token(&refresh_token),
            user_id,
            client_id,
            scope,
            &wrapped,
            &rec.id,
        )
        .map_err(|e| {
            Box::new(err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth.db",
                e.to_string(),
            ))
        })?;

    Ok((access_token, refresh_token, ACCESS_TOKEN_TTL_SECS))
}

// ------------------------------------------------------------- handlers ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateTokenRequest {
    name: String,
    /// Omitted defaults to `read`: handing out a full-access token should be a
    /// deliberate choice, not what you get by forgetting the field.
    scope: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatedToken {
    id: String,
    name: String,
    scope: String,
    created_at: String,
    /// The only time this value ever leaves the server.
    token: String,
}

pub(crate) async fn list(State(st): State<Arc<ServerState>>, user: AuthedUser) -> Response {
    match st.users.list_api_tokens(&user.user_id) {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    }
}

pub(crate) async fn create(
    State(st): State<Arc<ServerState>>,
    user: AuthedUser,
    Json(body): Json<CreateTokenRequest>,
) -> Response {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > MAX_TOKEN_NAME_LEN {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.invalid_input",
            format!("token name must be 1-{MAX_TOKEN_NAME_LEN} characters"),
        );
    }
    let scope = body.scope.as_deref().unwrap_or(SCOPE_READ);
    if scope != SCOPE_READ && scope != SCOPE_FULL {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.invalid_input",
            "scope must be 'read' or 'full'",
        );
    }

    let Some(dbkey) = db_key_from_hex(&user.db_key_hex) else {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "auth.crypto",
            "session key is malformed",
        );
    };

    // `None`: a token created here is pasted into a CLI config by hand, with no
    // refresh token to renew it. Expiring it would just break silently later.
    match issue_token(&st, &user.user_id, name, scope, &dbkey, None) {
        Ok((token, rec)) => {
            tracing::info!(user_id = %user.user_id, token_id = %rec.id, scope = %scope, "API token created");
            (
                StatusCode::OK,
                Json(CreatedToken {
                    id: rec.id,
                    name: rec.name,
                    scope: rec.scope,
                    created_at: rec.created_at,
                    token,
                }),
            )
                .into_response()
        }
        Err(resp) => *resp,
    }
}

pub(crate) async fn revoke(
    State(st): State<Arc<ServerState>>,
    user: AuthedUser,
    Path(id): Path<String>,
) -> Response {
    match st.users.delete_api_token(&user.user_id, &id) {
        Ok(0) => err_response(
            StatusCode::NOT_FOUND,
            "auth.token_not_found",
            "no such token",
        ),
        Ok(_) => {
            // Kill the refresh token too. Revoking a connector in Settings must
            // mean it stops working — leaving the refresh token alive would let
            // it mint a replacement access token minutes later, which reads to
            // the user as revocation silently failing.
            let _ = st.users.delete_refresh_tokens_for_access_token(&id);
            tracing::info!(user_id = %user.user_id, token_id = %id, "API token revoked");
            (StatusCode::OK, Json(serde_json::json!({}))).into_response()
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_pat_round_trips_through_parse() {
        let (token, raw) = generate_pat();
        assert!(token.starts_with(PAT_PREFIX));
        assert_eq!(parse_pat(&token), Some(raw));
    }

    #[test]
    fn generated_pats_are_distinct() {
        let (a, _) = generate_pat();
        let (b, _) = generate_pat();
        assert_ne!(a, b);
    }

    #[test]
    fn parse_pat_rejects_malformed_input() {
        assert!(parse_pat("").is_none());
        assert!(parse_pat("bearer-something").is_none());
        // Right prefix, wrong payload length (16 bytes, not 32).
        let short = format!("{PAT_PREFIX}{}", URL_SAFE_NO_PAD.encode([0u8; 16]));
        assert!(parse_pat(&short).is_none());
        // Right prefix, not base64url at all.
        assert!(parse_pat(&format!("{PAT_PREFIX}!!!!")).is_none());
        // A session cookie value (64 hex chars) must never parse as a PAT.
        assert!(parse_pat(&hex::encode([0u8; 32])).is_none());
    }

    #[test]
    fn pat_unwraps_the_key_it_wrapped() {
        let (token, raw) = generate_pat();
        let dbkey = crate::crypto::generate_db_key();
        let wrapped = crate::crypto::wrap_key_with_token(&raw, &dbkey).unwrap();

        let parsed = parse_pat(&token).unwrap();
        assert_eq!(
            crate::crypto::unwrap_key_with_token(&parsed, &wrapped).unwrap(),
            dbkey
        );
    }

    #[test]
    fn db_key_hex_round_trip() {
        let dbkey = crate::crypto::generate_db_key();
        let hex_key = crate::crypto::db_key_to_hex(&dbkey);
        assert_eq!(db_key_from_hex(&hex_key), Some(dbkey));
        assert!(db_key_from_hex("not-hex").is_none());
        assert!(db_key_from_hex("aabb").is_none());
    }
}

//! Auth endpoints (`/api/auth/*`) and the `AuthedUser` extractor every other
//! route resolves through. NOT Tauri commands — plain REST, never appears in
//! `bindings.ts`.
//!
//! Security invariants (see plan): unwrapped DB keys never touch disk or a
//! `tracing`/`println` line; unknown-username and wrong-password both return
//! `auth.bad_credentials` (no username oracle); the recovery key is returned
//! exactly once, in the setup/admin-create response body, and never stored.

use crate::sessions::SESSION_COOKIE;
use crate::state::ServerState;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::{header, request::Parts, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::CookieJar;
use finsight_api::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use std::sync::Arc;

fn err_response(status: StatusCode, code: &str, msg: impl Into<String>) -> Response {
    (status, Json(AppError::new(code, msg.into()))).into_response()
}

fn auth_required() -> Response {
    err_response(StatusCode::UNAUTHORIZED, "auth.required", "authentication required")
}

fn bad_credentials() -> Response {
    err_response(
        StatusCode::UNAUTHORIZED,
        "auth.bad_credentials",
        "invalid username or password",
    )
}

/// Resolves the session cookie to `(user_id, db_key_hex, is_admin)`. 401
/// `auth.required` when the cookie is missing, unknown, or expired — the UI
/// shim keys its login redirect off this code.
pub struct AuthedUser {
    pub user_id: String,
    pub db_key_hex: String,
    pub is_admin: bool,
}

impl FromRequestParts<Arc<ServerState>> for AuthedUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<ServerState>,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE)
            .map(|c| c.value().to_string())
            .ok_or_else(auth_required)?;
        let (user_id, db_key_hex, is_admin) = state.sessions.get(&token).ok_or_else(auth_required)?;
        Ok(AuthedUser {
            user_id,
            db_key_hex: db_key_hex.to_string(),
            is_admin,
        })
    }
}

/// Admin-gated variant: `AuthedUser` plus a 403 `auth.admin_required` guard.
pub struct AdminUser(pub AuthedUser);

impl FromRequestParts<Arc<ServerState>> for AdminUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<ServerState>,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthedUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(err_response(
                StatusCode::FORBIDDEN,
                "auth.admin_required",
                "admin privileges required",
            ));
        }
        Ok(AdminUser(user))
    }
}

fn cookie_secure_flag() -> &'static str {
    if std::env::var("FINSIGHT_COOKIE_SECURE").ok().as_deref() == Some("1") {
        "; Secure"
    } else {
        ""
    }
}

fn set_cookie_header(token: &str) -> (header::HeaderName, String) {
    (
        header::SET_COOKIE,
        format!(
            "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/{}",
            cookie_secure_flag()
        ),
    )
}

fn clear_cookie_header() -> (header::HeaderName, String) {
    (
        header::SET_COOKIE,
        format!(
            "{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/{}; Max-Age=0",
            cookie_secure_flag()
        ),
    )
}

// ---------------------------------------------------------------- status ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    needs_setup: bool,
    authenticated: bool,
    username: Option<String>,
    is_admin: Option<bool>,
}

pub(crate) async fn status(State(st): State<Arc<ServerState>>, jar: CookieJar) -> Response {
    let needs_setup = st.users.is_empty().unwrap_or(false);
    let session = jar
        .get(SESSION_COOKIE)
        .and_then(|c| st.sessions.get(c.value()));
    let body = match session {
        Some((user_id, _key, is_admin)) => {
            let username = st
                .users
                .get_by_id(&user_id)
                .ok()
                .flatten()
                .map(|u| u.username);
            AuthStatus {
                needs_setup,
                authenticated: true,
                username,
                is_admin: Some(is_admin),
            }
        }
        None => AuthStatus {
            needs_setup,
            authenticated: false,
            username: None,
            is_admin: None,
        },
    };
    Json(body).into_response()
}

// ----------------------------------------------------------------- setup ---

#[derive(Deserialize)]
pub(crate) struct Credentials {
    username: String,
    password: String,
}

/// A stable, precomputed dummy PHC verifier used to keep the "unknown
/// username" path doing the same Argon2id work as a real failed-password
/// check — this doesn't make the endpoint constant-time, but it avoids the
/// cheapest, most obvious timing tell (skipping the hash entirely).
fn dummy_phc() -> &'static str {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DUMMY.get_or_init(|| {
        crate::crypto::hash_password("finsight-dummy-timing-guard")
            .expect("hashing a fixed string cannot fail")
    })
}

/// If `<data>/data.sqlcipher` AND `<data>/db.key` both exist, this is a
/// Phase 1 single-user install: read its keyfile hex so the new admin
/// inherits the existing key (instead of a fresh random one), and report
/// that a file migration is needed.
fn read_legacy_key(data_dir: &FsPath) -> Option<[u8; 32]> {
    let db_path = data_dir.join("data.sqlcipher");
    let key_path = data_dir.join("db.key");
    if !db_path.exists() || !key_path.exists() {
        return None;
    }
    let hex_str = std::fs::read_to_string(&key_path).ok()?;
    let bytes = hex::decode(hex_str.trim()).ok()?;
    bytes.try_into().ok()
}

/// `rename(2)` reports EXDEV (Unix errno 18) / ERROR_NOT_SAME_DEVICE (Windows
/// 17) when source and destination straddle filesystems. Matched by raw code
/// rather than `ErrorKind::CrossesDevices` so this stays within the crate's
/// declared MSRV (that variant only stabilized in Rust 1.85).
fn is_cross_device(e: &std::io::Error) -> bool {
    match e.raw_os_error() {
        #[cfg(unix)]
        Some(code) => code == 18, // EXDEV
        #[cfg(windows)]
        Some(code) => code == 17, // ERROR_NOT_SAME_DEVICE
        #[cfg(not(any(unix, windows)))]
        Some(_) => false,
        None => false,
    }
}

/// Move a single file, tolerant of cross-filesystem boundaries. `std::fs::rename`
/// fails outright when `src` and `dst` live on different mounts — a real case
/// for this project's Docker deploy target, where `<data>` is often a bind
/// mount and `users/<id>/` may resolve to a different device. On that specific
/// error, fall back to copy-then-remove.
fn move_file(src: &FsPath, dst: &FsPath) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device(&e) => {
            std::fs::copy(src, dst)?;
            std::fs::remove_file(src)
        }
        Err(e) => Err(e),
    }
}

/// Moves `data.sqlcipher{,-wal,-shm}` into `users/<admin_id>/` and deletes
/// the now-unnecessary `db.key`. Called only after the admin's wrapped keys
/// have been derived from the SAME key these files are encrypted with.
///
/// Returns `Err` if any DB file move fails — the caller MUST treat that as a
/// setup failure (the user's real data would otherwise be silently orphaned
/// at the old path behind a fresh empty admin DB). The `db.key` unlink is
/// best-effort but NOT silent: a lingering plaintext key that still decrypts
/// the now-moved database is a security regression, so a failure to remove it
/// is logged loudly rather than swallowed.
fn migrate_legacy_files(data_dir: &FsPath, admin_id: &str) -> std::io::Result<()> {
    let user_dir = crate::registry::user_data_dir(data_dir, admin_id);
    std::fs::create_dir_all(&user_dir)?;
    for suffix in ["", "-wal", "-shm"] {
        let src = data_dir.join(format!("data.sqlcipher{suffix}"));
        if src.exists() {
            move_file(&src, &user_dir.join(format!("data.sqlcipher{suffix}")))?;
        }
    }
    let key_path = data_dir.join("db.key");
    if let Err(e) = std::fs::remove_file(&key_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(
                path = %key_path.display(),
                "legacy db.key could not be deleted after migration — a plaintext DB key \
                 that decrypts the migrated database still exists on disk and must be \
                 removed manually: {e}"
            );
        }
    }
    Ok(())
}

pub(crate) async fn setup(State(st): State<Arc<ServerState>>, Json(body): Json<Credentials>) -> Response {
    match st.users.is_empty() {
        Ok(true) => {}
        Ok(false) => {
            return err_response(StatusCode::CONFLICT, "auth.already_setup", "setup already completed")
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    }
    if body.username.trim().is_empty() || body.password.is_empty() {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.invalid_input",
            "username and password are required",
        );
    }

    let legacy_key = read_legacy_key(&st.data_dir);
    let dbkey: [u8; 32] = legacy_key.unwrap_or_else(crate::crypto::generate_db_key);
    let db_key_hex = crate::crypto::db_key_to_hex(&dbkey);

    let phc = match crate::crypto::hash_password(&body.password) {
        Ok(p) => p,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let salt = crate::crypto::generate_salt();
    let wrapped_pw = match crate::crypto::wrap_key_with_password(&body.password, &salt, &dbkey) {
        Ok(w) => w,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let recovery = crate::crypto::generate_recovery_key();
    let wrapped_recovery = match crate::crypto::wrap_key_with_recovery(&recovery.bytes, &dbkey) {
        Ok(w) => w,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };

    let rec = match st
        .users
        .create_user(&body.username, &phc, &salt, &wrapped_pw, &wrapped_recovery, true)
    {
        Ok(r) => r,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    };

    if legacy_key.is_some() {
        match migrate_legacy_files(&st.data_dir, &rec.id) {
            Ok(()) => {
                tracing::info!(
                    admin_id = %rec.id,
                    "migrated legacy single-user database into the new admin account"
                );
            }
            Err(e) => {
                // The migration failed AFTER the admin row was created. Left
                // as-is, `users.is_empty()` would now be false — so setup
                // could never re-run — while the user's real Phase 1 data
                // sits orphaned at the old path behind a fresh empty DB, and
                // we'd hand back a working session to that empty DB. Instead:
                // roll back the admin row (making setup retryable once the
                // operator fixes the cause), issue NO session cookie, and
                // return a distinct, path-naming error.
                let legacy_path = st.data_dir.join("data.sqlcipher");
                tracing::error!(
                    admin_id = %rec.id,
                    legacy_path = %legacy_path.display(),
                    "legacy database migration failed; rolling back admin account so setup can be retried: {e}"
                );
                if let Err(del_err) = st.users.delete_user(&rec.id) {
                    tracing::error!(
                        admin_id = %rec.id,
                        "failed to roll back admin account after migration failure — setup may be stuck; delete the user row in users.db manually: {del_err}"
                    );
                }
                return err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "auth.migration_failed",
                    format!(
                        "could not migrate the existing database at {} into the new account: {e}",
                        legacy_path.display()
                    ),
                );
            }
        }
    }

    let token = st.sessions.create(&rec.id, db_key_hex, true);
    (
        StatusCode::OK,
        [set_cookie_header(&token)],
        Json(serde_json::json!({ "recoveryKey": recovery.display })),
    )
        .into_response()
}

// ----------------------------------------------------------------- login ---

pub(crate) async fn login(State(st): State<Arc<ServerState>>, Json(body): Json<Credentials>) -> Response {
    let rec = match st.users.get_by_username(&body.username) {
        Ok(Some(r)) => r,
        Ok(None) => {
            // No such user: still pay the Argon2id cost so this path isn't
            // trivially distinguishable from a real wrong-password check.
            let _ = crate::crypto::verify_password(&body.password, dummy_phc());
            return bad_credentials();
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    };
    if !crate::crypto::verify_password(&body.password, &rec.password_phc) {
        return bad_credentials();
    }
    let dbkey =
        match crate::crypto::unwrap_key_with_password(&body.password, &rec.kek_salt, &rec.wrapped_key_pw) {
            Ok(k) => k,
            // Verifier and KEK are independent (see crypto.rs docs); a mismatch
            // here means corrupted state, not a legitimate login — still
            // surfaced as bad_credentials so it isn't a distinct oracle.
            Err(_) => return bad_credentials(),
        };
    let db_key_hex = crate::crypto::db_key_to_hex(&dbkey);
    let token = st.sessions.create(&rec.id, db_key_hex, rec.is_admin);
    (StatusCode::OK, [set_cookie_header(&token)], Json(serde_json::json!({}))).into_response()
}

pub(crate) async fn logout(State(st): State<Arc<ServerState>>, jar: CookieJar) -> Response {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        st.sessions.remove(c.value());
    }
    (StatusCode::OK, [clear_cookie_header()], Json(serde_json::json!({}))).into_response()
}

// ------------------------------------------------------- admin: users ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserSummary {
    id: String,
    username: String,
    is_admin: bool,
    created_at: String,
}

pub(crate) async fn list_users(State(st): State<Arc<ServerState>>, _admin: AdminUser) -> Response {
    match st.users.list_users() {
        Ok(users) => {
            let out: Vec<UserSummary> = users
                .into_iter()
                .map(|u| UserSummary {
                    id: u.id,
                    username: u.username,
                    is_admin: u.is_admin,
                    created_at: u.created_at,
                })
                .collect();
            (StatusCode::OK, Json(out)).into_response()
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    }
}

pub(crate) async fn create_user(
    State(st): State<Arc<ServerState>>,
    _admin: AdminUser,
    Json(body): Json<Credentials>,
) -> Response {
    if body.username.trim().is_empty() || body.password.is_empty() {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.invalid_input",
            "username and password are required",
        );
    }
    let dbkey = crate::crypto::generate_db_key();
    let phc = match crate::crypto::hash_password(&body.password) {
        Ok(p) => p,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let salt = crate::crypto::generate_salt();
    let wrapped_pw = match crate::crypto::wrap_key_with_password(&body.password, &salt, &dbkey) {
        Ok(w) => w,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let recovery = crate::crypto::generate_recovery_key();
    let wrapped_recovery = match crate::crypto::wrap_key_with_recovery(&recovery.bytes, &dbkey) {
        Ok(w) => w,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    match st
        .users
        .create_user(&body.username, &phc, &salt, &wrapped_pw, &wrapped_recovery, false)
    {
        Ok(_rec) => (
            StatusCode::OK,
            Json(serde_json::json!({ "recoveryKey": recovery.display })),
        )
            .into_response(),
        Err(e) => err_response(StatusCode::CONFLICT, "auth.username_taken", e.to_string()),
    }
}

pub(crate) async fn delete_user(
    State(st): State<Arc<ServerState>>,
    admin: AdminUser,
    Path(id): Path<String>,
) -> Response {
    if id == admin.0.user_id {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.cannot_delete_self",
            "cannot delete your own account",
        );
    }
    st.sessions.remove_user(&id);
    st.registry.evict(&id);
    if let Err(e) = st.users.delete_user(&id) {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string());
    }
    let user_dir = crate::registry::user_data_dir(&st.data_dir, &id);
    remove_dir_with_retry(&user_dir).await;
    (StatusCode::OK, Json(serde_json::json!({}))).into_response()
}

/// `evict` drops our `Arc<UserRuntime>`, but the SQLCipher pool's file
/// handles (and the agent's dedicated background thread, which holds its
/// own `Db` clone) close asynchronously on a different OS thread — so an
/// immediate `remove_dir_all` can race a still-open WAL/SHM file, most
/// visibly on Windows where an open file simply can't be unlinked. Retry
/// briefly (a few hundred ms, worst case) rather than surfacing a spurious
/// failure on an otherwise-successful delete.
async fn remove_dir_with_retry(dir: &FsPath) {
    for _ in 0..20 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
    if let Err(e) = std::fs::remove_dir_all(dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(dir = %dir.display(), "failed to remove user data dir after delete: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_file_relocates_within_the_same_filesystem() {
        // The copy-then-remove fallback needs two real filesystems to trigger,
        // which a unit test can't stage portably — so this covers the common
        // `rename` path (a tempdir is always same-device) and proves the
        // helper actually moves: dst gains the bytes, src is gone.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("data.sqlcipher");
        let dst_dir = dir.path().join("users").join("some-id");
        std::fs::create_dir_all(&dst_dir).unwrap();
        let dst = dst_dir.join("data.sqlcipher");
        std::fs::write(&src, b"encrypted-bytes").unwrap();

        move_file(&src, &dst).unwrap();

        assert!(!src.exists(), "source should be gone after a move");
        assert_eq!(std::fs::read(&dst).unwrap(), b"encrypted-bytes");
    }

    #[test]
    fn move_file_surfaces_non_cross_device_errors() {
        // A missing source is not EXDEV, so it must propagate as-is (not get
        // silently retried through the copy fallback).
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("does-not-exist");
        let dst = dir.path().join("dst");
        assert!(move_file(&src, &dst).is_err());
    }
}

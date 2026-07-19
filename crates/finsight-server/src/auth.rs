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
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::Zeroizing;

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
    /// Stays `Zeroizing` all the way from `SessionStore::get`. It was
    /// previously copied out with `.to_string()`, which dropped a plaintext
    /// 64-char SQLCipher key into unzeroed heap on EVERY authenticated
    /// request — the exact leak the store's `Zeroizing` exists to prevent.
    /// Consumers (`dispatch.rs`, `events.rs`) pass `&user.db_key_hex` where
    /// `&str` is wanted; deref coercion carries that through unchanged.
    pub db_key_hex: Zeroizing<String>,
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
            db_key_hex,
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
    // A failed read USED to collapse to `unwrap_or(false)` — i.e. "setup is
    // done, go log in" — so a corrupt or unreadable users.db showed a login
    // screen for an account that could not possibly authenticate, and hid the
    // real fault. `setup` already surfaces this same Err as 500 auth.db;
    // status now matches it so the UI reports a broken server as broken.
    let needs_setup = match st.users.is_empty() {
        Ok(v) => v,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    };
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

/// Minimum password length for any route that SETS a password (setup,
/// create_user, recover). Deliberately a length floor only — no character-class
/// rules, which push users toward `Passw0rd!` and are no longer recommended
/// (NIST SP 800-63B). The previous check was `is_empty()`, so a 1-character
/// password was accepted.
pub const MIN_PASSWORD_LEN: usize = 10;

/// Upper bound, measured in BYTES. Argon2id's cost is driven by the input it
/// hashes, so an unbounded password is a free CPU-amplification handle on an
/// unauthenticated endpoint: a multi-megabyte body would occupy a blocking
/// thread indefinitely. 1 KiB is far above any real passphrase.
pub const MAX_PASSWORD_LEN: usize = 1024;

/// Length policy for password-SETTING routes. Never call this on `login`:
/// rejecting a short password at sign-in would both break existing accounts
/// and leak policy through a distinguishable error, so login keeps returning
/// the uniform `bad_credentials`.
fn check_password_policy(password: &str) -> Result<(), Response> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "auth.weak_password",
            format!("password must be at least {MIN_PASSWORD_LEN} characters"),
        ));
    }
    if password.len() > MAX_PASSWORD_LEN {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "auth.weak_password",
            format!("password must be at most {MAX_PASSWORD_LEN} bytes"),
        ));
    }
    Ok(())
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
/// that a file migration is needed. An incomplete or unreadable legacy pair
/// is an error, never "no legacy install": generating a fresh key in that
/// state would strand the existing ledger permanently.
fn read_legacy_key(data_dir: &FsPath) -> Result<Option<[u8; 32]>, String> {
    let db_path = data_dir.join("data.sqlcipher");
    let key_path = data_dir.join("db.key");
    match (db_path.exists(), key_path.exists()) {
        (false, false) => return Ok(None),
        (true, false) => {
            return Err(format!(
                "existing legacy database at {} has no db.key; restore the key file before retrying setup",
                db_path.display()
            ))
        }
        (false, true) => {
            return Err(format!(
                "legacy key at {} has no matching data.sqlcipher; restore or remove the orphaned key before retrying setup",
                key_path.display()
            ))
        }
        (true, true) => {}
    }
    let hex_str = std::fs::read_to_string(&key_path)
        .map_err(|e| format!("could not read legacy key at {}: {e}", key_path.display()))?;
    let bytes = hex::decode(hex_str.trim()).map_err(|e| {
        format!(
            "legacy key at {} is not valid hexadecimal: {e}",
            key_path.display()
        )
    })?;
    let key = bytes.try_into().map_err(|bytes: Vec<u8>| {
        format!(
            "legacy key at {} is {} bytes; expected 32 bytes",
            key_path.display(),
            bytes.len()
        )
    })?;
    Ok(Some(key))
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
/// Apply a list of `(src, dst)` moves as a unit. If any move fails, every move
/// already applied is put BACK before returning the error, so the source tree
/// is left exactly as it was found.
///
/// This is the fix for a partial-migration data-loss bug. Previously the moves
/// ran unguarded: if `data.sqlcipher` moved but `-wal` then failed, the caller
/// rolled back only the users.db row. A retry would then find no legacy DB at
/// the old path, generate a FRESH key, and create an EMPTY database — while
/// the real ledger sat orphaned under `users/<dead-uuid>/`, and its `-wal`
/// (holding the most recent committed transactions) sat at the old path,
/// separated from the main file it belongs to.
///
/// A failed rollback is logged with both paths, since at that point only a
/// human can reunite the files.
fn move_all_or_rollback(moves: &[(PathBuf, PathBuf)]) -> std::io::Result<()> {
    let mut applied: Vec<&(PathBuf, PathBuf)> = Vec::new();
    for m in moves {
        match move_file(&m.0, &m.1) {
            Ok(()) => applied.push(m),
            Err(e) => {
                for done in applied.iter().rev() {
                    if let Err(back_err) = move_file(&done.1, &done.0) {
                        tracing::error!(
                            moved_to = %done.1.display(),
                            original = %done.0.display(),
                            "could not restore a migrated database file after a failed \
                             migration — move it back manually before retrying setup, or \
                             the retry will create an empty database: {back_err}"
                        );
                    }
                }
                return Err(e);
            }
        }
    }
    Ok(())
}

fn migrate_legacy_files(data_dir: &FsPath, admin_id: &str) -> std::io::Result<()> {
    let user_dir = crate::registry::user_data_dir(data_dir, admin_id);
    std::fs::create_dir_all(&user_dir)?;

    // Stage every move first, then commit them as a unit. `-wal`/`-shm` are
    // sidecars of the main DB file; a state where only some of them have moved
    // is not a database.
    let moves: Vec<(PathBuf, PathBuf)> = ["", "-wal", "-shm"]
        .iter()
        .map(|suffix| {
            (
                data_dir.join(format!("data.sqlcipher{suffix}")),
                user_dir.join(format!("data.sqlcipher{suffix}")),
            )
        })
        .filter(|(src, _)| src.exists())
        .collect();
    move_all_or_rollback(&moves)?;

    // Only now, with every DB file safely relocated, drop the legacy keyfile.
    // Order matters: unlinking it earlier would leave a rolled-back retry
    // facing a legacy DB it can no longer read the key for.
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
    if body.username.trim().is_empty() {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.invalid_input",
            "username and password are required",
        );
    }
    if let Err(resp) = check_password_policy(&body.password) {
        return resp;
    }

    // Make the empty-check and eventual insert a single-flight transition.
    // The guard intentionally spans the async password/key derivation: a
    // second setup waits, then observes the first administrator and gets 409.
    let _setup_guard = st.setup_lock.lock().await;
    match st.users.is_empty() {
        Ok(true) => {}
        Ok(false) => {
            return err_response(StatusCode::CONFLICT, "auth.already_setup", "setup already completed")
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    }

    let legacy_key = match read_legacy_key(&st.data_dir) {
        Ok(key) => key,
        Err(e) => {
            return err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth.migration_failed",
                e,
            )
        }
    };
    let dbkey: [u8; 32] = legacy_key.unwrap_or_else(crate::crypto::generate_db_key);
    let db_key_hex = crate::crypto::db_key_to_hex(&dbkey);

    let phc = match crate::crypto::hash_password_async(body.password.clone()).await {
        Ok(p) => p,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let salt = crate::crypto::generate_salt();
    let wrapped_pw = match crate::crypto::wrap_key_with_password_async(
        body.password.clone(),
        salt.to_vec(),
        dbkey,
    )
    .await
    {
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

/// 429 for a username inside its lockout window. Deliberately does NOT say
/// whether the account exists — the throttle keys on the submitted string
/// either way, so this response is identical for a real and a fictional user.
fn too_many_attempts() -> Response {
    err_response(
        StatusCode::TOO_MANY_REQUESTS,
        "auth.too_many_attempts",
        "too many failed attempts; try again later",
    )
}

pub(crate) async fn login(State(st): State<Arc<ServerState>>, Json(body): Json<Credentials>) -> Response {
    // Throttle check comes FIRST — before the DB lookup and before any Argon2
    // work — so a credential-stuffing run is shed cheaply instead of buying
    // the attacker ~20ms of blocking-pool CPU per guess.
    if st.throttle.is_locked(&body.username) {
        return too_many_attempts();
    }

    let rec = match st.users.get_by_username(&body.username) {
        Ok(Some(r)) => r,
        Ok(None) => {
            // No such user: still pay the Argon2id cost so this path isn't
            // trivially distinguishable from a real wrong-password check.
            // Via spawn_blocking like every other Argon2 call here — running
            // the dummy inline would leave the timing guard itself blocking
            // the runtime, and would make the unknown-user path measurably
            // different under load.
            let _ =
                crate::crypto::verify_password_async(body.password.clone(), dummy_phc().to_string())
                    .await;
            st.throttle.record_failure(&body.username);
            return bad_credentials();
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    };
    if !crate::crypto::verify_password_async(body.password.clone(), rec.password_phc.clone()).await {
        st.throttle.record_failure(&body.username);
        return bad_credentials();
    }
    let dbkey = match crate::crypto::unwrap_key_with_password_async(
        body.password.clone(),
        rec.kek_salt.clone(),
        rec.wrapped_key_pw.clone(),
    )
    .await
    {
        Ok(k) => k,
        // Verifier and KEK are independent (see crypto.rs docs); a mismatch
        // here means corrupted state, not a legitimate login — still
        // surfaced as bad_credentials so it isn't a distinct oracle.
        Err(_) => {
            st.throttle.record_failure(&body.username);
            return bad_credentials();
        }
    };
    st.throttle.record_success(&body.username);
    let db_key_hex = crate::crypto::db_key_to_hex(&dbkey);
    let token = st.sessions.create(&rec.id, db_key_hex, rec.is_admin);
    (StatusCode::OK, [set_cookie_header(&token)], Json(serde_json::json!({}))).into_response()
}

pub(crate) async fn logout(State(st): State<Arc<ServerState>>, jar: CookieJar) -> Response {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        // Note the user BEFORE dropping the session so we can tear their
        // runtime down once their last session is gone. Without this the
        // runtime survived until the 30-minute idle sweep, keeping the
        // unwrapped SQLCipher key resident in the r2d2 pool and letting the
        // background SimpleFin sync keep writing to a signed-out user's DB —
        // contradicting the invariant that background work is only possible
        // while a session holds the key. (`delete_user` already evicts; this
        // path was the omission.) Multi-device safe: only the LAST session out
        // evicts, and any surviving session simply re-bootstraps on demand.
        let user_id = st.sessions.get(c.value()).map(|(uid, _, _)| uid);
        st.sessions.remove(c.value());
        if let Some(uid) = user_id {
            if !st.sessions.has_user_sessions(&uid) {
                st.registry.evict(&uid);
            }
        }
    }
    (StatusCode::OK, [clear_cookie_header()], Json(serde_json::json!({}))).into_response()
}

// -------------------------------------------------------------- recover ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoverRequest {
    username: String,
    recovery_key: String,
    new_password: String,
}

fn bad_recovery_key() -> Response {
    err_response(
        StatusCode::UNAUTHORIZED,
        "auth.bad_recovery_key",
        "invalid username or recovery key",
    )
}

/// A fixed wrapped blob used to make the unknown-username path do the same
/// AEAD work as a real wrong-key attempt.
///
/// Note this is NOT the Argon2 dummy `login` uses, and that difference is the
/// point: the recovery key IS the KEK (32 high-entropy bytes, no KDF — see
/// crypto.rs), so redemption failures resolve in a cheap XChaCha20-Poly1305
/// open, not a password hash. Mirroring `dummy_phc()` here would be timing
/// theatre against the wrong operation.
fn dummy_wrapped_recovery() -> &'static [u8] {
    static DUMMY: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    DUMMY.get_or_init(|| {
        let key = crate::crypto::generate_recovery_key();
        crate::crypto::wrap_key_with_recovery(&key.bytes, &crate::crypto::generate_db_key())
            .expect("wrapping a fresh key cannot fail")
    })
}

/// `POST /api/auth/recover` — redeem a recovery key to set a new password.
///
/// This is the endpoint that makes the setup screen's promise true ("your
/// recovery key is the only way back into your data"). It works because the
/// SQLCipher key is stored wrapped TWICE — under the password KEK and under
/// the recovery key — so redeeming unwraps with copy #2 and re-wraps the very
/// same db key under a new password. The user's data is never re-encrypted and
/// never leaves the process.
///
/// On success it also ROTATES the recovery key: the one just submitted has
/// been used, possibly typed into an untrusted place, and is single-use by
/// design. The response carries the replacement, shown exactly once.
///
/// Both failure modes — unknown username and wrong key — return an identical
/// 401 `auth.bad_recovery_key`, and the unknown-username branch does dummy
/// unwrap work first so the two aren't separable by timing either. Otherwise
/// this endpoint would be the username oracle that `login` carefully isn't.
pub(crate) async fn recover(
    State(st): State<Arc<ServerState>>,
    Json(body): Json<RecoverRequest>,
) -> Response {
    if st.throttle.is_locked(&body.username) {
        return too_many_attempts();
    }
    // Policy applies here as much as at signup — recovery is a password-SETTING
    // route, and skipping the check would leave it as a way to install a weak
    // password that `setup` would have refused.
    if let Err(resp) = check_password_policy(&body.new_password) {
        return resp;
    }

    let rec = match st.users.get_by_username(&body.username) {
        Ok(Some(r)) => r,
        Ok(None) => {
            let _ = crate::crypto::unwrap_key_with_recovery_display_async(
                body.recovery_key.clone(),
                dummy_wrapped_recovery().to_vec(),
            )
            .await;
            st.throttle.record_failure(&body.username);
            return bad_recovery_key();
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string()),
    };

    let dbkey = match crate::crypto::unwrap_key_with_recovery_display_async(
        body.recovery_key.clone(),
        rec.wrapped_key_recovery.clone(),
    )
    .await
    {
        Ok(k) => k,
        Err(_) => {
            st.throttle.record_failure(&body.username);
            return bad_recovery_key();
        }
    };

    // Re-wrap the SAME db key under the new password, with a FRESH salt. A
    // reused salt would let anyone holding the old wrapped blob test password
    // guesses against precomputed work for this account.
    let new_salt = crate::crypto::generate_salt();
    let phc = match crate::crypto::hash_password_async(body.new_password.clone()).await {
        Ok(p) => p,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let wrapped_pw = match crate::crypto::wrap_key_with_password_async(
        body.new_password.clone(),
        new_salt.to_vec(),
        dbkey,
    )
    .await
    {
        Ok(w) => w,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };

    let new_recovery = crate::crypto::generate_recovery_key();
    let wrapped_recovery =
        match crate::crypto::wrap_key_with_recovery(&new_recovery.bytes, &dbkey) {
            Ok(w) => w,
            Err(e) => {
                return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string())
            }
        };

    // One UPDATE moves all four credential columns together — a partial write
    // here could leave the account unopenable by either password or key.
    if let Err(e) =
        st.users
            .update_credentials(&rec.id, &phc, &new_salt, &wrapped_pw, &wrapped_recovery)
    {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.db", e.to_string());
    }

    // Existing sessions are dropped: recovery is the flow you run when you may
    // have lost control of the account, so any session opened under the old
    // password must not survive it. The user's own new session is created
    // after this sweep.
    st.sessions.remove_user(&rec.id);
    // A runtime owns the event broadcaster and background sync task. Revoking
    // only cookies leaves an already-open SSE connection subscribed to that
    // runtime, so recovery must evict it as part of the same revocation.
    st.registry.evict(&rec.id);
    st.throttle.record_success(&body.username);

    tracing::info!(
        user_id = %rec.id,
        "recovery key redeemed: password reset, recovery key rotated, prior sessions revoked"
    );

    let db_key_hex = crate::crypto::db_key_to_hex(&dbkey);
    let token = st.sessions.create(&rec.id, db_key_hex, rec.is_admin);
    (
        StatusCode::OK,
        [set_cookie_header(&token)],
        Json(serde_json::json!({ "recoveryKey": new_recovery.display })),
    )
        .into_response()
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
    if body.username.trim().is_empty() {
        return err_response(
            StatusCode::BAD_REQUEST,
            "auth.invalid_input",
            "username and password are required",
        );
    }
    if let Err(resp) = check_password_policy(&body.password) {
        return resp;
    }
    let dbkey = crate::crypto::generate_db_key();
    let phc = match crate::crypto::hash_password_async(body.password.clone()).await {
        Ok(p) => p,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "auth.crypto", e.to_string()),
    };
    let salt = crate::crypto::generate_salt();
    let wrapped_pw = match crate::crypto::wrap_key_with_password_async(
        body.password.clone(),
        salt.to_vec(),
        dbkey,
    )
    .await
    {
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

    /// The data-loss regression guard for finding 7. Fails the SECOND move,
    /// after the first has already succeeded — the exact shape the old code
    /// mishandled (main DB moved, `-wal` move failed, rollback deleted only
    /// the users.db row, retry created an empty database over orphaned data).
    ///
    /// Injected at the helper rather than through `setup` on purpose: the real
    /// destination is `users/<random-uuid>/`, so there is no path a test can
    /// pre-create to break the second move for an unknown UUID. Here the
    /// second destination's parent is a FILE, which fails deterministically on
    /// every platform.
    #[test]
    fn move_all_or_rollback_restores_earlier_moves_when_a_later_one_fails() {
        let dir = tempfile::tempdir().unwrap();
        let src_a = dir.path().join("data.sqlcipher");
        let src_b = dir.path().join("data.sqlcipher-wal");
        std::fs::write(&src_a, b"main-db").unwrap();
        std::fs::write(&src_b, b"write-ahead-log").unwrap();

        let dst_dir = dir.path().join("users").join("some-id");
        std::fs::create_dir_all(&dst_dir).unwrap();
        // Make the SECOND destination unwritable: its parent is a regular file.
        let blocked_parent = dst_dir.join("blocked");
        std::fs::write(&blocked_parent, b"not a directory").unwrap();

        let moves = vec![
            (src_a.clone(), dst_dir.join("data.sqlcipher")),
            (src_b.clone(), blocked_parent.join("data.sqlcipher-wal")),
        ];

        assert!(move_all_or_rollback(&moves).is_err(), "the second move must fail");

        // Everything is back where a retry will look for it, contents intact.
        assert!(src_a.exists(), "the first move must be rolled back");
        assert!(src_b.exists(), "the failed move's source is untouched");
        assert_eq!(std::fs::read(&src_a).unwrap(), b"main-db");
        assert_eq!(std::fs::read(&src_b).unwrap(), b"write-ahead-log");
        assert!(
            !dst_dir.join("data.sqlcipher").exists(),
            "nothing may be left orphaned at the destination"
        );
    }

    #[test]
    fn move_all_or_rollback_commits_when_every_move_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let src_a = dir.path().join("data.sqlcipher");
        let src_b = dir.path().join("data.sqlcipher-wal");
        std::fs::write(&src_a, b"main-db").unwrap();
        std::fs::write(&src_b, b"write-ahead-log").unwrap();
        let dst_dir = dir.path().join("users").join("some-id");
        std::fs::create_dir_all(&dst_dir).unwrap();

        let moves = vec![
            (src_a.clone(), dst_dir.join("data.sqlcipher")),
            (src_b.clone(), dst_dir.join("data.sqlcipher-wal")),
        ];
        move_all_or_rollback(&moves).unwrap();

        assert!(!src_a.exists() && !src_b.exists());
        assert_eq!(std::fs::read(dst_dir.join("data.sqlcipher")).unwrap(), b"main-db");
        assert_eq!(
            std::fs::read(dst_dir.join("data.sqlcipher-wal")).unwrap(),
            b"write-ahead-log"
        );
    }

    #[test]
    fn password_policy_bounds() {
        assert!(check_password_policy(&"a".repeat(MIN_PASSWORD_LEN)).is_ok());
        assert!(check_password_policy(&"a".repeat(MIN_PASSWORD_LEN - 1)).is_err());
        assert!(check_password_policy("").is_err());
        assert!(check_password_policy(&"a".repeat(MAX_PASSWORD_LEN)).is_ok());
        assert!(check_password_policy(&"a".repeat(MAX_PASSWORD_LEN + 1)).is_err());
        // Counted in chars, not bytes, so a short multi-byte passphrase isn't
        // waved through by its byte length.
        assert!(check_password_policy("héllo-wör").is_err()); // 9 chars
        assert!(check_password_policy("héllo-wörld").is_ok()); // 11 chars
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

//! Password verification and DB-key wrapping.
//!
//! Design (Bitwarden pattern, per the spec): each user's SQLCipher key is a
//! RANDOM 32-byte key. It is stored only in WRAPPED form, twice:
//!   - under KEK1 = Argon2id(password, kek_salt)   → password changes re-wrap, not re-encrypt
//!   - under KEK2 = the recovery key bytes directly → recovery key IS high-entropy, no KDF needed
//!
//! Password *verification* uses a separate Argon2id PHC string (its own salt) so
//! the verifier can't be used to derive the KEK.

use argon2::password_hash::rand_core::OsRng as PasswordHashOsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroizing;

pub const DB_KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24; // XChaCha20

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("password hashing failed: {0}")]
    Hash(String),
    #[error("key wrapping failed")]
    Wrap,
    #[error("wrong password or corrupted wrapped key")]
    Unwrap,
    #[error("malformed recovery key")]
    BadRecoveryKey,
}

// NOTE: hash_password/verify_password deliberately stay on Argon2::default() —
// the PHC string is self-describing (params travel with the hash), so verifier
// params CAN evolve safely across argon2 upgrades. The KEK derivation below
// cannot: its params are pinned (see kek_argon2).
pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    let salt = SaltString::generate(&mut PasswordHashOsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| CryptoError::Hash(e.to_string()))
}

pub fn verify_password(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

pub fn generate_db_key() -> [u8; DB_KEY_LEN] {
    let mut k = [0u8; DB_KEY_LEN];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut s = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut s);
    s
}

pub fn db_key_to_hex(key: &[u8; DB_KEY_LEN]) -> String {
    hex::encode(key)
}

fn kek_argon2() -> Argon2<'static> {
    // PINNED: these parameters are part of the on-disk key-wrapping format.
    // Changing ANY of them breaks unwrapping of every existing wrapped key —
    // never change without a re-wrap migration. (Argon2id v19, m=19456 KiB,
    // t=2, p=1, 32-byte output — the argon2 0.5 defaults, frozen explicitly.)
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(19_456, 2, 1, Some(32)).expect("valid pinned params"),
    )
}

fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut kek = [0u8; 32];
    kek_argon2()
        .hash_password_into(password.as_bytes(), salt, &mut kek)
        .map_err(|e| CryptoError::Hash(e.to_string()))?;
    Ok(kek)
}

fn wrap_with_kek(kek: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(kek.into());
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| CryptoError::Wrap)?;
    let mut out = nonce.to_vec();
    out.extend(ct);
    Ok(out)
}

/// Returns the decrypted plaintext in a `Zeroizing` buffer so intermediate
/// key material is wiped when the caller's copy-out completes.
fn unwrap_with_kek(kek: &[u8; 32], wrapped: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if wrapped.len() < NONCE_LEN + 16 {
        return Err(CryptoError::Unwrap);
    }
    let (nonce, ct) = wrapped.split_at(NONCE_LEN);
    XChaCha20Poly1305::new(kek.into())
        .decrypt(XNonce::from_slice(nonce), ct)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Unwrap)
}

pub fn wrap_key_with_password(
    password: &str,
    kek_salt: &[u8],
    dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let kek = Zeroizing::new(derive_kek(password, kek_salt)?);
    wrap_with_kek(&kek, dbkey)
}

pub fn unwrap_key_with_password(
    password: &str,
    kek_salt: &[u8],
    wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let kek = Zeroizing::new(derive_kek(password, kek_salt)?);
    let v = unwrap_with_kek(&kek, wrapped)?;
    v.as_slice().try_into().map_err(|_| CryptoError::Unwrap)
}

/// Recovery key: 32 random bytes, shown once as 8 dash-separated hex groups.
pub struct RecoveryKey {
    pub bytes: [u8; 32],
    pub display: String,
}

pub fn generate_recovery_key() -> RecoveryKey {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    let h = hex::encode(b);
    let display = h
        .as_bytes()
        .chunks(8)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("-");
    RecoveryKey { bytes: b, display }
}

pub fn recovery_display_to_bytes(display: &str) -> Result<[u8; 32], CryptoError> {
    let h: String = display.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let v = hex::decode(&h).map_err(|_| CryptoError::BadRecoveryKey)?;
    v.try_into().map_err(|_| CryptoError::BadRecoveryKey)
}

pub fn wrap_key_with_recovery(
    recovery_bytes: &[u8; 32],
    dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    wrap_with_kek(recovery_bytes, dbkey)
}

pub fn unwrap_key_with_recovery_display(
    display: &str,
    wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let bytes = Zeroizing::new(recovery_display_to_bytes(display)?);
    let v = unwrap_with_kek(&bytes, wrapped)?;
    v.as_slice().try_into().map_err(|_| CryptoError::Unwrap)
}

// ------------------------------------------------- off-runtime wrappers ---
//
// Every function below runs Argon2id at the PINNED cost (m=19456 KiB, t=2) —
// tens of milliseconds of solid CPU per call. Called inline from an async
// handler that work sits on a tokio WORKER thread, so a handful of concurrent
// logins starve the whole runtime: RPC dispatch and the SSE event stream stall
// behind them on a 1–2 core self-host box. These wrappers hand the work to
// `spawn_blocking` so only the blocking pool feels it.
//
// They take owned arguments because the closure must be `'static`. The
// `expect` on the join handle only fires if the closure panicked (Argon2id on
// valid pinned params doesn't) or the runtime is shutting down.

const BLOCKING_PANIC: &str = "argon2 blocking task panicked";

pub async fn hash_password_async(password: String) -> Result<String, CryptoError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .expect(BLOCKING_PANIC)
}

pub async fn verify_password_async(password: String, phc: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password(&password, &phc))
        .await
        .expect(BLOCKING_PANIC)
}

pub async fn wrap_key_with_password_async(
    password: String,
    kek_salt: Vec<u8>,
    dbkey: [u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    tokio::task::spawn_blocking(move || wrap_key_with_password(&password, &kek_salt, &dbkey))
        .await
        .expect(BLOCKING_PANIC)
}

pub async fn unwrap_key_with_password_async(
    password: String,
    kek_salt: Vec<u8>,
    wrapped: Vec<u8>,
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    tokio::task::spawn_blocking(move || unwrap_key_with_password(&password, &kek_salt, &wrapped))
        .await
        .expect(BLOCKING_PANIC)
}

/// `/api/auth/recover` is unauthenticated, exactly like `login`, so its Argon2id
/// work is the same DoS surface and must not run on a runtime worker either —
/// including the unknown-user dummy unwrap, which otherwise both blocks the
/// runtime AND makes the guard path measurably cheaper than the real one.
pub async fn unwrap_key_with_recovery_display_async(
    display: String,
    wrapped: Vec<u8>,
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    tokio::task::spawn_blocking(move || unwrap_key_with_recovery_display(&display, &wrapped))
        .await
        .expect(BLOCKING_PANIC)
}

// --------------------------------------------------------- API tokens ---
//
// An MCP/API access token is 32 random bytes (like the recovery key), so it is
// used DIRECTLY as the KEK that wraps the user's DB key — no KDF needed. The
// token itself is stored only as a SHA-256 hash (lookup index); the AEAD tag on
// the wrapped key is the real verification, so a stolen users.db yields neither
// a usable token nor a DB key.

pub fn wrap_key_with_token(
    token_bytes: &[u8; 32],
    dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    wrap_with_kek(token_bytes, dbkey)
}

pub fn unwrap_key_with_token(
    token_bytes: &[u8; 32],
    wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let v = unwrap_with_kek(token_bytes, wrapped)?;
    v.as_slice().try_into().map_err(|_| CryptoError::Unwrap)
}

// ------------------------------------------------- server session key ---
//
// Persistent sessions (survive server restarts) need the server to recover a
// session's unwrapped DB key WITHOUT the user's password. Each persisted
// session stores its DB key wrapped under a SERVER master key (SMK) — a 32-byte
// secret held for the process's whole lifetime.
//
// SECURITY TRADEOFF, stated plainly: whoever holds the SMK *and* a persisted
// session row can decrypt that session's SQLCipher DB at rest — and since the
// primary user stays logged in, the SMK effectively always decrypts their DB.
// That is the cost of "stay logged in across restarts" for an encrypted-at-rest
// app, and it matches the self-hosted idiom (a server-held secret, like
// Immich's JWT/DB credentials). So the SMK can be supplied via the
// `FINSIGHT_SESSION_KEY` env var — letting an operator keep it OFF the data
// volume, which restores meaningful at-rest protection — and is only generated
// into `<data_dir>/session.key` when that override is absent.

pub const SERVER_KEY_ENV: &str = "FINSIGHT_SESSION_KEY";

/// Resolve the server session master key: `FINSIGHT_SESSION_KEY` (64 hex chars)
/// if set, else the persisted `session.key`, else a freshly generated key
/// written to that path (0600 on Unix). The env path never touches disk, so an
/// operator can keep the master key entirely off the data volume.
pub fn load_or_create_server_key(session_key_path: &std::path::Path) -> std::io::Result<[u8; 32]> {
    if let Ok(hex_key) = std::env::var(SERVER_KEY_ENV) {
        if let Ok(bytes) = hex::decode(hex_key.trim()) {
            if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(key);
            }
        }
        // A set-but-malformed override is almost certainly an operator mistake
        // (truncated paste, wrong length). Fail loudly rather than silently
        // minting a fresh key that would invalidate every persisted session.
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{SERVER_KEY_ENV} must be exactly 64 hex characters (32 bytes)"),
        ));
    }
    if let Ok(existing) = std::fs::read(session_key_path) {
        if let Ok(key) = <[u8; 32]>::try_from(existing.as_slice()) {
            return Ok(key);
        }
        // A corrupt/short key file is unrecoverable — sessions wrapped under the
        // old key can't be read regardless — so regenerating is the only way
        // forward. The only cost is a one-time re-login for everyone.
    }
    let key = generate_db_key(); // same CSPRNG, 32 bytes
    write_key_file(session_key_path, &key)?;
    Ok(key)
}

fn write_key_file(path: &std::path::Path, key: &[u8; 32]) -> std::io::Result<()> {
    std::fs::write(path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Wrap a 32-byte DB key under the server master key for at-rest storage in the
/// sessions table. Output is `nonce || ciphertext` (XChaCha20-Poly1305), the
/// same envelope shape as the password/recovery wrappers above.
pub fn wrap_key_with_server_key(
    server_key: &[u8; 32],
    dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    wrap_with_kek(server_key, dbkey)
}

/// Reverse of [`wrap_key_with_server_key`]. Returns `Unwrap` on a wrong key or
/// tampered ciphertext (AEAD tag mismatch).
pub fn unwrap_key_with_server_key(
    server_key: &[u8; 32],
    wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let v = unwrap_with_kek(server_key, wrapped)?;
    v.as_slice().try_into().map_err(|_| CryptoError::Unwrap)
}

/// SHA-256 of an opaque session token, used as the persisted primary key. The
/// token is 256-bit random, so a plain hash (no salt, no KDF) suffices: a
/// stolen `users.db` yields only hashes, never a usable session cookie.
pub fn hash_session_token(token: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(token.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_wrap_unwrap_round_trip_and_rejects_wrong_token() {
        let token = generate_db_key(); // same shape: 32 random bytes
        let dbkey = generate_db_key();
        let wrapped = wrap_key_with_token(&token, &dbkey).unwrap();
        assert_eq!(unwrap_key_with_token(&token, &wrapped).unwrap(), dbkey);
        let other = generate_db_key();
        assert!(unwrap_key_with_token(&other, &wrapped).is_err());
        // Tampered ciphertext must fail the AEAD tag, not decrypt garbage.
        let mut tampered = wrapped.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(unwrap_key_with_token(&token, &tampered).is_err());
    }

    #[test]
    fn server_key_wrap_unwrap_round_trip() {
        let smk = generate_db_key();
        let dbkey = generate_db_key();
        let wrapped = wrap_key_with_server_key(&smk, &dbkey).unwrap();
        assert_eq!(unwrap_key_with_server_key(&smk, &wrapped).unwrap(), dbkey);
        // A different server key must NOT unwrap it.
        let other = generate_db_key();
        assert!(unwrap_key_with_server_key(&other, &wrapped).is_err());
    }

    #[test]
    fn load_or_create_server_key_generates_then_reuses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.key");
        assert!(!path.exists());
        let k1 = load_or_create_server_key(&path).unwrap();
        assert!(path.exists(), "first call must persist the generated key");
        let k2 = load_or_create_server_key(&path).unwrap();
        assert_eq!(
            k1, k2,
            "second call must read the same key back, not regenerate"
        );
    }

    #[test]
    fn hash_session_token_is_deterministic_and_distinct() {
        assert_eq!(hash_session_token("abc"), hash_session_token("abc"));
        assert_ne!(hash_session_token("abc"), hash_session_token("abd"));
        assert_eq!(hash_session_token("abc").len(), 32); // SHA-256 = 32 bytes
    }

    #[tokio::test]
    async fn async_wrappers_match_their_blocking_counterparts() {
        // The spawn_blocking hop must be behaviour-preserving: same PHC
        // semantics, same wrap/unwrap round trip.
        let phc = hash_password_async("hunter2-and-more".to_string())
            .await
            .unwrap();
        assert!(verify_password_async("hunter2-and-more".to_string(), phc.clone()).await);
        assert!(!verify_password_async("wrong".to_string(), phc).await);

        let dbkey = generate_db_key();
        let salt = generate_salt();
        let wrapped =
            wrap_key_with_password_async("hunter2-and-more".to_string(), salt.to_vec(), dbkey)
                .await
                .unwrap();
        let back =
            unwrap_key_with_password_async("hunter2-and-more".to_string(), salt.to_vec(), wrapped)
                .await
                .unwrap();
        assert_eq!(back, dbkey);
    }

    #[test]
    fn password_verify_round_trip() {
        let phc = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &phc));
        assert!(!verify_password("wrong", &phc));
    }

    #[test]
    fn wrap_unwrap_round_trip_with_password_kek() {
        let dbkey = generate_db_key(); // 32 bytes
        let salt = generate_salt(); // 16 bytes
        let wrapped = wrap_key_with_password("hunter2", &salt, &dbkey).unwrap();
        let back = unwrap_key_with_password("hunter2", &salt, &wrapped).unwrap();
        assert_eq!(back, dbkey);
    }

    #[test]
    fn wrong_password_fails_to_unwrap() {
        let dbkey = generate_db_key();
        let salt = generate_salt();
        let wrapped = wrap_key_with_password("hunter2", &salt, &dbkey).unwrap();
        assert!(unwrap_key_with_password("wrong", &salt, &wrapped).is_err());
    }

    #[test]
    fn recovery_key_wraps_and_unwraps() {
        let dbkey = generate_db_key();
        let recovery = generate_recovery_key(); // RecoveryKey { bytes, display }
        let wrapped = wrap_key_with_recovery(&recovery.bytes, &dbkey).unwrap();
        let back = unwrap_key_with_recovery_display(&recovery.display, &wrapped).unwrap();
        assert_eq!(back, dbkey);
        // display form is 8 groups of 8 hex chars, dash separated
        assert_eq!(recovery.display.split('-').count(), 8);
        assert!(unwrap_key_with_recovery_display("bad-key", &wrapped).is_err());
    }

    #[test]
    fn db_key_is_64_hex_for_sqlcipher() {
        let k = generate_db_key();
        assert_eq!(db_key_to_hex(&k).len(), 64); // Db::open requires 64 hex chars
    }
}

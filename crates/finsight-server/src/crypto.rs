//! Password verification and DB-key wrapping.
//!
//! Design (Bitwarden pattern, per the spec): each user's SQLCipher key is a
//! RANDOM 32-byte key. It is stored only in WRAPPED form, twice:
//!   - under KEK1 = Argon2id(password, kek_salt)   → password changes re-wrap, not re-encrypt
//!   - under KEK2 = the recovery key bytes directly → recovery key IS high-entropy, no KDF needed
//! Password *verification* uses a separate Argon2id PHC string (its own salt) so
//! the verifier can't be used to derive the KEK.

use argon2::password_hash::rand_core::OsRng as PasswordHashOsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;

pub const DB_KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24; // XChaCha20

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("password hashing failed: {0}")]
    Hash(String),
    #[error("wrong password or corrupted wrapped key")]
    Unwrap,
    #[error("malformed recovery key")]
    BadRecoveryKey,
}

pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    let salt = SaltString::generate(&mut PasswordHashOsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| CryptoError::Hash(e.to_string()))
}

pub fn verify_password(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
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

fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut kek = [0u8; 32];
    Argon2::default()
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
        .map_err(|_| CryptoError::Unwrap)?;
    let mut out = nonce.to_vec();
    out.extend(ct);
    Ok(out)
}

fn unwrap_with_kek(kek: &[u8; 32], wrapped: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if wrapped.len() < NONCE_LEN + 16 {
        return Err(CryptoError::Unwrap);
    }
    let (nonce, ct) = wrapped.split_at(NONCE_LEN);
    XChaCha20Poly1305::new(kek.into())
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| CryptoError::Unwrap)
}

pub fn wrap_key_with_password(
    password: &str,
    kek_salt: &[u8],
    dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    wrap_with_kek(&derive_kek(password, kek_salt)?, dbkey)
}

pub fn unwrap_key_with_password(
    password: &str,
    kek_salt: &[u8],
    wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let v = unwrap_with_kek(&derive_kek(password, kek_salt)?, wrapped)?;
    v.try_into().map_err(|_| CryptoError::Unwrap)
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
    let bytes = recovery_display_to_bytes(display)?;
    let v = unwrap_with_kek(&bytes, wrapped)?;
    v.try_into().map_err(|_| CryptoError::Unwrap)
}

#[cfg(test)]
mod tests {
    use super::*;

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

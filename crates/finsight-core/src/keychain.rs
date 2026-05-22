use crate::error::CoreResult;
use keyring::Entry;
use rand::RngCore;
use zeroize::Zeroizing;

/// Returns a 64-char hex string (32 random bytes) wrapped in `Zeroizing` so
/// the in-memory copy is securely wiped when dropped.
/// If a key already exists for (service, user), returns it; otherwise creates one.
pub fn load_or_create_key(service: &str, user: &str) -> CoreResult<Zeroizing<String>> {
    let entry = Entry::new(service, user)?;
    match entry.get_password() {
        Ok(existing) => Ok(Zeroizing::new(existing)),
        Err(keyring::Error::NoEntry) => {
            let hex = generate_random_key();
            entry.set_password(&*hex)?;
            Ok(hex)
        }
        Err(e) => Err(e.into()),
    }
}

/// Removes the keychain entry, if any. Returns Ok(()) whether or not an entry existed.
pub fn delete_key(service: &str, user: &str) -> CoreResult<()> {
    let entry = Entry::new(service, user)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Generate a fresh random 64-char hex key (32 bytes) without touching the OS keychain.
/// Intended for tests. In production code, use `load_or_create_key` instead.
#[doc(hidden)]
pub fn generate_random_key() -> Zeroizing<String> {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hex = bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    // Zero raw bytes before returning.
    for b in bytes.iter_mut() {
        *b = 0;
    }
    Zeroizing::new(hex)
}

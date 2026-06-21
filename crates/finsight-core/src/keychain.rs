use crate::error::CoreResult;
use keyring::Entry;
use rand::RngCore;
use zeroize::Zeroizing;

pub const SIMPLEFIN_SERVICE: &str = "com.finsight.simplefin";
pub const SIMPLEFIN_USER: &str = "default";

/// Returns a 64-char hex string (32 random bytes) wrapped in `Zeroizing` so
/// the in-memory copy is securely wiped when dropped.
/// If a key already exists for (service, user), returns it; otherwise creates one.
pub fn load_or_create_key(service: &str, user: &str) -> CoreResult<Zeroizing<String>> {
    let entry = Entry::new(service, user)?;
    match entry.get_password() {
        Ok(existing) => Ok(Zeroizing::new(existing)),
        Err(keyring::Error::NoEntry) => {
            let hex = generate_random_key();
            entry.set_password(&hex)?;
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

/// Store a user-supplied string value in the OS keychain.
pub fn set_key(service: &str, user: &str, value: &str) -> CoreResult<()> {
    let entry = Entry::new(service, user)?;
    entry.set_password(value)?;
    Ok(())
}

/// Retrieve a previously stored value. Returns None if not found.
pub fn get_key(service: &str, user: &str) -> CoreResult<Option<String>> {
    let entry = Entry::new(service, user)?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
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

#[cfg(test)]
mod tests {
    use super::*;

    // On Linux the Secret Service (gnome-keyring) requires a real login
    // session to initialise its default collection, which headless CI runners
    // don't provide.  Tests run on macOS and Windows where a native keychain
    // is always available.
    #[test]
    #[cfg_attr(target_os = "linux", ignore)]
    fn get_key_returns_none_when_absent() {
        let svc = "com.finsight.test.keychain";
        let usr = &format!("test-absent-{}", uuid::Uuid::new_v4());
        let _ = delete_key(svc, usr);
        let got = get_key(svc, usr).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    #[cfg_attr(target_os = "linux", ignore)]
    fn set_key_round_trip() {
        let svc = "com.finsight.test.keychain";
        let usr = &format!("test-rt-{}", uuid::Uuid::new_v4());
        set_key(svc, usr, "sk-test-value").unwrap();
        let got = get_key(svc, usr).unwrap();
        assert_eq!(got.as_deref(), Some("sk-test-value"));
        delete_key(svc, usr).unwrap();
    }
}

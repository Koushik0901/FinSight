use crate::error::CoreResult;
use keyring::Entry;
use rand::RngCore;

/// Returns a 64-char hex string (32 random bytes).
/// If a key already exists for (service, user), returns it; otherwise creates one.
pub fn load_or_create_key(service: &str, user: &str) -> CoreResult<String> {
    let entry = Entry::new(service, user)?;
    match entry.get_password() {
        Ok(existing) => Ok(existing),
        Err(keyring::Error::NoEntry) => {
            let mut bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut bytes);
            let hex = bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            entry.set_password(&hex)?;
            Ok(hex)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn delete_key(service: &str, user: &str) -> CoreResult<()> {
    let entry = Entry::new(service, user)?;
    entry.delete_credential()?;
    Ok(())
}

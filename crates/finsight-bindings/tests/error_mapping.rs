//! Pins the contract between CoreError variants and the AppError code strings
//! the frontend relies on. If a CoreError variant is added or the mapping is
//! reordered, this test will fail with a clear diff.

use finsight_bindings::error::AppError;
use finsight_core::CoreError;

#[test]
fn core_error_invalid_state_maps_to_core_invalid_state() {
    let e: AppError = CoreError::InvalidState("oops".into()).into();
    assert_eq!(e.code, "core.invalid_state");
    assert!(e.message.contains("oops"));
}

#[test]
fn core_error_keychain_maps_to_core_keychain() {
    // Construct a keyring NoEntry error via the public keyring API.
    let e: AppError = CoreError::Keychain(keyring::Error::NoEntry).into();
    assert_eq!(e.code, "core.keychain");
}

#[test]
fn core_error_database_maps_to_core_db() {
    let e: AppError = CoreError::Database(rusqlite::Error::QueryReturnedNoRows).into();
    assert_eq!(e.code, "core.db");
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("keychain error: {0}")]
    Keychain(#[from] keyring::Error),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("migration error: {0}")]
    Migration(#[from] refinery::Error),

    #[error("invalid state: {0}")]
    InvalidState(String),

    /// A proposed action failed validation before touching state (malformed
    /// payload, target not found). Distinct from [`CoreError::InvalidState`]
    /// so callers can classify outcomes by variant instead of matching on
    /// message substrings. Display still carries the historical
    /// "validation: " prefix those messages always had.
    #[error("validation: {0}")]
    Validation(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

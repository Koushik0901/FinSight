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
}

pub type CoreResult<T> = Result<T, CoreError>;

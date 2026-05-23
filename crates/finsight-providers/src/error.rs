use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("file is empty")]
    EmptyFile,
    #[error("file too large: {bytes} bytes (max {cap})")]
    FileTooLarge { bytes: u64, cap: u64 },
    #[error("file is not readable: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv parse: {0}")]
    Csv(#[from] csv::Error),
    #[error("encoding: could not decode as UTF-8 or Windows-1252")]
    UndecodableEncoding,
    #[error("invalid mapping: {0}")]
    InvalidMapping(String),
    #[error("database: {0}")]
    Core(#[from] finsight_core::CoreError),
    #[error("internal: {0}")]
    Internal(String),
}

pub type ProviderResult<T> = std::result::Result<T, ProviderError>;

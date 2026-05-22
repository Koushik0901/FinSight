//! FinSight core: domain types, SQLCipher storage, repositories.

pub mod db;
pub mod error;
pub mod keychain;

pub use db::Db;
pub use error::{CoreError, CoreResult};

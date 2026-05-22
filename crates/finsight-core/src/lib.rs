//! FinSight core: domain types, SQLCipher storage, repositories.

pub mod db;
pub mod error;
pub mod keychain;
pub mod models;
pub mod repos;
pub mod seed;
pub mod settings;

pub use db::Db;
pub use error::{CoreError, CoreResult};

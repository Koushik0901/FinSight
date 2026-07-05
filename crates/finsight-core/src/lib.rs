//! FinSight core: domain types, SQLCipher storage, repositories.

pub mod anomaly;
pub mod categorize;
pub mod db;
pub mod error;
pub mod merchant;
pub mod recurring;
pub mod forecast;
pub mod keychain;
pub mod models;
pub mod palette;
pub mod repos;
pub mod reset_barrier;
pub mod sample;
pub mod seed;
pub mod settings;

pub use db::Db;
pub use error::{CoreError, CoreResult};
pub use reset_barrier::{ResetBarrier, ResetGuard, WriterLease};

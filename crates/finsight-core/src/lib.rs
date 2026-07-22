//! FinSight core: domain types, SQLCipher storage, repositories.

pub mod anomaly;
pub mod cashflow;
pub mod categorize;
pub mod currency;
pub mod db;
pub mod error;
pub mod merchant;
pub mod recurring;
pub mod forecast;
pub mod investments;
pub mod keychain;
pub mod metrics;
pub mod models;
pub mod notify;
pub mod provenance;
pub mod palette;
pub mod repos;
pub mod reset_barrier;
pub mod routes;
pub mod sample;
pub mod seed;
pub mod settings;
pub mod spending;

pub use db::Db;
pub use error::{CoreError, CoreResult};
pub use reset_barrier::{ResetBarrier, ResetGuard, WriterLease};

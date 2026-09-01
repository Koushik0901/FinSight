//! FinSight core: domain types, SQLCipher storage, repositories.

pub mod anomaly;
pub mod cashflow;
pub mod categorize;
pub mod currency;
pub mod db;
pub mod error;
pub mod forecast;
pub mod investments;
pub mod keychain;
pub mod merchant;
pub mod metrics;
pub mod models;
pub mod notify;
pub mod palette;
pub mod provenance;
pub mod recurring;
pub mod repos;
pub mod reset_barrier;
pub mod routes;
#[cfg(any(test, feature = "dev-seed"))]
pub mod sample;
#[cfg(any(test, feature = "dev-seed"))]
pub mod seed;
pub mod settings;
pub mod spending;
pub mod subscriptions;
/// Fast database fixtures. Public rather than `#[cfg(test)]` because the
/// integration tests of every other crate need them too, and a `cfg(test)`
/// module is not visible across crate boundaries. Feature-gated so the extra
/// dependency never reaches a shipped binary.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use db::Db;
pub use error::{CoreError, CoreResult};
pub use reset_barrier::{ResetBarrier, ResetGuard, WriterLease};

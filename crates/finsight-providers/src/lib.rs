//! finsight-providers — pluggable transaction sources.
//! Phase 2 ships the csv module; Plaid/SimpleFin land in later phases.

pub mod csv;
pub mod error;
pub mod provider;

pub use error::{ProviderError, ProviderResult};
pub use provider::SyncProvider;

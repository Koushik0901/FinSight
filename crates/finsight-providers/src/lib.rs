//! finsight-providers — pluggable transaction sources.
//! Phase 2 ships the csv module; Plaid/SimpleFin land in later phases.

pub mod csv;
pub mod error;
pub mod provider;

pub use csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
pub use csv::{CsvPreview, CsvProvider, ImportSummary, RowError};
pub use error::{ProviderError, ProviderResult};
pub use provider::SyncProvider;

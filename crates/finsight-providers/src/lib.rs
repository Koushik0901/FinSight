//! finsight-providers — pluggable transaction sources.

pub mod csv;
pub mod error;
pub mod provider;
pub mod simplefin;

pub use csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
pub use csv::{CsvPreview, CsvProvider, ImportSummary, RowError};
pub use error::{ProviderError, ProviderResult};
pub use provider::SyncProvider;

//! Replaces the Phase 1 stub. Sync providers are pluggable backends that
//! produce transactions on the same shape CsvProvider does.

use crate::error::ProviderResult;
use finsight_core::models::NewTransaction;

/// A SyncProvider pulls transactions and yields them as parsed rows.
pub trait SyncProvider {
    /// Human-readable id (e.g. "csv"); used in the imports.source column.
    fn id(&self) -> &'static str;

    /// Stream rows for the given account. Lazy — callers may stop early.
    fn rows(&self) -> Box<dyn Iterator<Item = ProviderResult<NewTransaction>> + '_>;
}

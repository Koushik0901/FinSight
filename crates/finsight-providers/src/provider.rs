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

/// Bank-data provider kind — mirrors `SimpleFIN` vs `Enable Banking (EU)`.
/// Keeps the per-user SQLCipher model: each variant's credential (SimpleFIN
/// access URL vs Enable Banking JWT) lives in the per-user encrypted settings,
/// never in a global slot, and the `finsight_providers::enable_banking`
/// tests prove per-token isolation (no cross-read).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncProviderKind {
    SimpleFin,
    EnableBanking,
}

impl SyncProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SimpleFin => "simplefin",
            Self::EnableBanking => "enable_banking",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "simplefin" => Some(Self::SimpleFin),
            "enable_banking" | "enableBanking" | "enable-banking" => Some(Self::EnableBanking),
            _ => None,
        }
    }
}

impl std::fmt::Display for SyncProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

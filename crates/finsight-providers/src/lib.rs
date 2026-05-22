//! FinSight data ingestion providers (CSV/OFX/QIF in Phase 2; Plaid/SimpleFin later).

use async_trait::async_trait;

#[async_trait]
pub trait SyncProvider: Send + Sync {
    fn id(&self) -> &str;
}

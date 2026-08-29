//! Enable Banking (EU) provider — second bank data source behind the
//! `SyncProvider` trait.
//!
//! Architecture: reuses the same `import_sync` shape as SimpleFIN
//! (`finsight-providers/src/simplefin/sync.rs:40`): a bearer-auth `Client`
//! that lists accounts and fetches transactions, plus a `fetch_enable_data`
//! free function that the scheduler/commands call. Keeps the per-user SQLCipher
//! model: the bearer JWT (like the SimpleFIN access URL) lives in the
//! per-user encrypted settings, never in a process-global slot, and tests
//! prove per-token isolation (no cross-read).

pub mod client;
pub mod models;
pub mod sync;

pub use client::EnableBankingClient;
pub use models::{EnableBankingAccount, EnableBankingBalance, EnableBankingTransaction};
pub use sync::{
    fetch_enable_account_data, fetch_enable_data, fetch_enable_data_with_base_url,
    EnableBankingSyncData, EnablePendingImport,
};

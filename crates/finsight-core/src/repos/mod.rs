pub mod accounts;
pub mod budgets;
pub mod categories;
pub mod categorizations;
pub mod goals;
pub mod imports;
pub mod liabilities;
pub mod manual_assets;
pub mod net_worth;
pub mod rules;
pub mod scenarios;
pub mod transactions;

use crate::error::CoreResult;
use crate::Db;
use tokio::task::spawn_blocking;

/// Helper: run a blocking closure against a fresh pool connection on a Tokio blocking thread.
pub async fn run<F, T>(db: &Db, f: F) -> CoreResult<T>
where
    F: FnOnce(&mut rusqlite::Connection) -> CoreResult<T> + Send + 'static,
    T: Send + 'static,
{
    let db = db.clone();
    spawn_blocking(move || {
        let mut conn = db.get()?;
        f(&mut conn)
    })
    .await
    .map_err(|e| crate::CoreError::InvalidState(format!("join error: {e}")))?
}

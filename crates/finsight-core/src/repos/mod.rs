pub mod accounts;
pub mod agent_memory;
pub mod alerts;
pub mod budgets;
pub mod categories;
pub mod categorizations;
pub mod connections;
pub mod conversations;
pub mod copilot_actions;
pub mod copilot_sessions;
pub mod goals;
pub mod holdings;
pub mod household;
pub mod import_candidates;
pub mod imports;
pub mod institutions;
pub mod manual_assets;
pub mod net_worth;
pub mod planned_transactions;
pub mod push;
pub mod recipes;
pub mod reset;
pub mod rule_proposals;
pub mod rules;
pub mod scenarios;
pub mod securities;
pub mod splits;
pub mod sync_runs;
pub mod transactions;
pub mod transfers;

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

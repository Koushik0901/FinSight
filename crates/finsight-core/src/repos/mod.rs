pub mod accounts;
pub mod agent_memory;
pub mod alerts;
pub mod balance;
pub mod budgets;
pub mod categories;
pub mod categorizations;
pub mod category_centroids;
pub mod category_examples;
pub mod category_proposals;
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
pub mod restoration;
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
use chrono::{DateTime, Utc};
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

/// Parse a stored RFC3339 timestamp inside a row mapper. Malformed values (DB
/// corruption, external tampering) surface as a rusqlite conversion error on
/// the mapped column instead of panicking the whole list/get call.
pub(super) fn rfc3339(idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
        })
}

/// Run `f` atomically against `conn`: all of its writes commit together or
/// none of them land.
///
/// This mirrors [`rusqlite::Connection::transaction`] but passes a plain
/// `&mut Connection` to `f`, which is what every repo helper takes —
/// rusqlite's `Transaction` only derefs immutably, so a scoped `tx` cannot be
/// handed to existing `&mut Connection` functions. Do not nest: `f` must not
/// itself open a transaction (`BEGIN` inside `BEGIN` is a SQLite error).
pub fn atomic<F, T>(conn: &mut rusqlite::Connection, f: F) -> CoreResult<T>
where
    F: FnOnce(&mut rusqlite::Connection) -> CoreResult<T>,
{
    // IMMEDIATE grabs the write lock up front: a concurrent writer fails fast
    // instead of hitting SQLITE_BUSY upgrading a read transaction mid-flight.
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match f(conn) {
        Ok(value) => {
            conn.execute_batch("COMMIT")?;
            Ok(value)
        }
        Err(e) => {
            // Best-effort rollback; the original error matters more than a
            // failure while discarding (e.g. the connection already died).
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

use crate::error::CoreResult;
use crate::models::SyncRun;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn start(conn: &mut Connection, trigger: &str) -> CoreResult<SyncRun> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO sync_runs (id, trigger, status, started_at) VALUES (?1, ?2, 'running', ?3)",
        params![&id, trigger, now.to_rfc3339()],
    )?;
    get(conn, &id)
}

pub struct SyncRunFinish {
    pub status: String,
    pub accounts_total: i64,
    pub accounts_succeeded: i64,
    pub accounts_failed: i64,
    pub added: i64,
    pub updated: i64,
    pub skipped: i64,
    pub queued_for_review: i64,
    pub error_summary: Option<String>,
}

pub fn finish(conn: &mut Connection, id: &str, input: SyncRunFinish) -> CoreResult<SyncRun> {
    conn.execute(
        "UPDATE sync_runs SET \
            status = ?1, finished_at = ?2, accounts_total = ?3, accounts_succeeded = ?4, \
            accounts_failed = ?5, added = ?6, updated = ?7, skipped = ?8, \
            queued_for_review = ?9, error_summary = ?10 \
         WHERE id = ?11",
        params![
            &input.status,
            Utc::now().to_rfc3339(),
            input.accounts_total,
            input.accounts_succeeded,
            input.accounts_failed,
            input.added,
            input.updated,
            input.skipped,
            input.queued_for_review,
            &input.error_summary,
            id,
        ],
    )?;
    get(conn, id)
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<SyncRun> {
    conn.query_row(
        "SELECT id, trigger, status, started_at, finished_at, accounts_total, accounts_succeeded, \
                accounts_failed, added, updated, skipped, queued_for_review, error_summary \
         FROM sync_runs WHERE id = ?1",
        params![id],
        sync_run_from_row,
    )
    .map_err(Into::into)
}

fn sync_run_from_row(r: &rusqlite::Row) -> rusqlite::Result<SyncRun> {
    let started_at: String = r.get(3)?;
    let finished_at: Option<String> = r.get(4)?;
    Ok(SyncRun {
        id: r.get(0)?,
        trigger: r.get(1)?,
        status: r.get(2)?,
        started_at: parse_dt(&started_at, 3)?,
        finished_at: finished_at.as_deref().map(|s| parse_dt(s, 4)).transpose()?,
        accounts_total: r.get(5)?,
        accounts_succeeded: r.get(6)?,
        accounts_failed: r.get(7)?,
        added: r.get(8)?,
        updated: r.get(9)?,
        skipped: r.get(10)?,
        queued_for_review: r.get(11)?,
        error_summary: r.get(12)?,
    })
}

fn parse_dt(s: &str, idx: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
        })
}

use crate::error::CoreResult;
use crate::models::NetWorthPoint;
use crate::repos::accounts;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn record_snapshot(conn: &mut Connection, total_cents: i64) -> CoreResult<()> {
    let id = Uuid::new_v4().to_string();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO net_worth_snapshots(id, date, total_cents, created_at) \
         VALUES(?1, ?2, ?3, ?4) \
         ON CONFLICT(date) DO UPDATE SET total_cents = excluded.total_cents",
        params![id, today, total_cents, now],
    )?;
    Ok(())
}

/// Sum all account balances and upsert today's snapshot. (Decision C: accounts
/// only for now — manual assets/liabilities fold in when those screens ship.)
pub fn record_today(conn: &mut Connection) -> CoreResult<()> {
    let total: i64 = accounts::list_summaries(conn)?
        .iter()
        .map(|a| a.balance_cents)
        .sum();
    record_snapshot(conn, total)
}

pub fn list_history(conn: &mut Connection, days: u32) -> CoreResult<Vec<NetWorthPoint>> {
    let cutoff = format!("-{} days", days);
    let mut stmt = conn.prepare(
        "SELECT date, total_cents FROM net_worth_snapshots \
         WHERE date >= date('now', ?1) ORDER BY date ASC",
    )?;
    let rows = stmt.query_map(params![cutoff], |r| {
        Ok(NetWorthPoint { date: r.get(0)?, total_cents: r.get(1)? })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("nw.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn record_snapshot_upserts_one_row_per_day() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        record_snapshot(&mut conn, 100_000).unwrap();
        record_snapshot(&mut conn, 250_000).unwrap();
        let hist = list_history(&mut conn, 30).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].total_cents, 250_000);
    }
}

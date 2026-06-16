use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Set (upsert) a budget for a category in a given month (format: "YYYY-MM").
pub fn set(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
    amount_cents: i64,
) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(category_id, month) DO UPDATE SET amount_cents = excluded.amount_cents, updated_at = excluded.updated_at",
        params![id, category_id, month, amount_cents, now],
    )?;
    Ok(())
}

/// Return a map of category_id → amount_cents for the given month.
pub fn list_for_month(conn: &mut Connection, month: &str) -> CoreResult<Vec<(String, i64)>> {
    let mut stmt =
        conn.prepare("SELECT category_id, amount_cents FROM budgets WHERE month = ?1")?;
    let rows = stmt.query_map(params![month], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

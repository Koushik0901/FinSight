use crate::error::CoreResult;
use crate::models::AgentMemory;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list(conn: &mut Connection) -> CoreResult<Vec<AgentMemory>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, description, merchant_key, created_at \
         FROM agent_memory ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AgentMemory {
            id: r.get(0)?,
            kind: r.get(1)?,
            description: r.get(2)?,
            merchant_key: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn upsert_correction(
    conn: &mut Connection,
    merchant_key: &str,
    description: &str,
) -> CoreResult<()> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_memory(id, kind, description, merchant_key, created_at) \
         VALUES(?1, 'correction', ?2, ?3, ?4) \
         ON CONFLICT(kind, merchant_key) DO UPDATE SET \
            description = excluded.description, created_at = excluded.created_at",
        params![id, description, merchant_key, now],
    )?;
    Ok(())
}

pub fn forget(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM agent_memory WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("am.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn upsert_correction_dedupes_by_merchant() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        upsert_correction(&mut conn, "amzn mktpl", "AMZN -> Shopping (1x)").unwrap();
        upsert_correction(&mut conn, "amzn mktpl", "AMZN -> Shopping (2x)").unwrap();
        let all = list(&mut conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].description, "AMZN -> Shopping (2x)");
        assert_eq!(all[0].kind, "correction");
        forget(&mut conn, &all[0].id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}

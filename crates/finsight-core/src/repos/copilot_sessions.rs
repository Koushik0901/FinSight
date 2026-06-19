use crate::error::CoreResult;
use crate::models::AgentSession;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<AgentSession> {
    Ok(AgentSession {
        id: r.get(0)?,
        title: r.get(1)?,
        status: r.get(2)?,
        task_type: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

pub fn list(conn: &mut Connection, limit: u32) -> CoreResult<Vec<AgentSession>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, status, task_type, created_at, updated_at
         FROM agent_sessions
         ORDER BY created_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], map_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert(conn: &mut Connection, title: &str, task_type: &str) -> CoreResult<AgentSession> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_sessions(id, title, status, task_type, created_at, updated_at)
         VALUES(?1, ?2, 'active', ?3, ?4, ?4)",
        params![id, title, task_type, now],
    )?;
    Ok(AgentSession {
        id,
        title: title.to_string(),
        status: "active".to_string(),
        task_type: task_type.to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn set_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agent_sessions
         SET status = ?1, updated_at = ?2
         WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<AgentSession>> {
    match conn.query_row(
        "SELECT id, title, status, task_type, created_at, updated_at
         FROM agent_sessions
         WHERE id = ?1",
        params![id],
        map_row,
    ) {
        Ok(session) => Ok(Some(session)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn save_context_snapshot(
    conn: &mut Connection,
    session_id: Option<&str>,
    context_json: &str,
) -> CoreResult<()> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    conn.execute(
        "INSERT INTO agent_context_snapshots(id, session_id, context_json, created_at)
         VALUES(?1, ?2, ?3, ?4)",
        params![
            Uuid::new_v4().to_string(),
            session_id,
            context_json,
            Utc::now().to_rfc3339()
        ],
    )?;
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
        let db = Db::open(&dir.path().join("copilot-sessions.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_get_and_update_session() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let session = insert(&mut conn, "Quarterly planning", "planning").unwrap();
        assert_eq!(session.status, "active");
        assert_eq!(session.task_type, "planning");

        let fetched = get(&mut conn, &session.id).unwrap().unwrap();
        assert_eq!(fetched.title, "Quarterly planning");

        set_status(&mut conn, &session.id, "closed").unwrap();
        let updated = get(&mut conn, &session.id).unwrap().unwrap();
        assert_eq!(updated.status, "closed");
        assert!(updated.updated_at >= updated.created_at);
    }

    #[test]
    fn list_orders_by_created_at_desc() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let first = insert(&mut conn, "First", "general").unwrap();
        let second = insert(&mut conn, "Second", "general").unwrap();
        conn.execute(
            "UPDATE agent_sessions SET created_at = '2024-01-01T00:00:00Z', updated_at = '2024-01-01T00:00:00Z' WHERE id = ?1",
            params![first.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET created_at = '2024-01-02T00:00:00Z', updated_at = '2024-01-02T00:00:00Z' WHERE id = ?1",
            params![second.id],
        )
        .unwrap();

        let sessions = list(&mut conn, 10).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].title, "Second");
        assert_eq!(sessions[1].title, "First");
        assert!(get(&mut conn, "missing").unwrap().is_none());
    }
}

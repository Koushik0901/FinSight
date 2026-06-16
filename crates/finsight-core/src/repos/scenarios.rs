use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ScenarioRow {
    pub id: String,
    pub description: String,
    pub result_json: String,
    pub created_at: String,
}

pub fn insert(
    conn: &mut Connection,
    description: &str,
    result_json: &str,
) -> CoreResult<ScenarioRow> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO scenarios(id, description, result_json, created_at) VALUES(?1, ?2, ?3, ?4)",
        params![id, description, result_json, now],
    )?;
    Ok(ScenarioRow {
        id,
        description: description.to_string(),
        result_json: result_json.to_string(),
        created_at: now,
    })
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<ScenarioRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, description, result_json, created_at FROM scenarios ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ScenarioRow {
            id: r.get(0)?,
            description: r.get(1)?,
            result_json: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM scenarios WHERE id = ?1", params![id])?;
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
        let db = Db::open(&dir.path().join("a.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_list_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let row = insert(&mut conn, "What if I buy a car?", r#"{"verdict":true}"#).unwrap();
        let listed = list(&mut conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].description, "What if I buy a car?");
        assert_eq!(listed[0].result_json, r#"{"verdict":true}"#);
        delete(&mut conn, &row.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}

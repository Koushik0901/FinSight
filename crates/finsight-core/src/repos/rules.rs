use crate::error::CoreResult;
use crate::models::{NewRule, Rule};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list_active(conn: &mut Connection) -> CoreResult<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, category_id, enabled, source, created_at \
         FROM rules WHERE enabled = 1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let created_s: String = r.get(5)?;
        Ok(Rule {
            id: r.get(0)?,
            pattern: r.get(1)?,
            category_id: r.get(2)?,
            enabled: r.get::<_, i64>(3)? != 0,
            source: r.get(4)?,
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&Utc),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert(conn: &mut Connection, rule: NewRule) -> CoreResult<Rule> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO rules(id, pattern, category_id, enabled, source, created_at) \
         VALUES(?1, ?2, ?3, 1, ?4, ?5)",
        params![
            id,
            rule.pattern,
            rule.category_id,
            rule.source,
            now.to_rfc3339()
        ],
    )?;
    Ok(Rule {
        id,
        pattern: rule.pattern,
        category_id: rule.category_id,
        enabled: true,
        source: rule.source,
        created_at: now,
    })
}

pub fn set_enabled(conn: &mut Connection, id: &str, enabled: bool) -> CoreResult<()> {
    conn.execute(
        "UPDATE rules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, id],
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
        let db = Db::open(&dir.path().join("r.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_and_list_active_rules() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();

        let rule = NewRule {
            pattern: "%amazon%".to_string(),
            category_id: "cat1".to_string(),
            source: "user".to_string(),
        };
        let r = insert(&mut conn, rule).unwrap();
        assert_eq!(r.pattern, "%amazon%");

        let active = list_active(&mut conn).unwrap();
        assert_eq!(active.len(), 1);

        set_enabled(&mut conn, &r.id, false).unwrap();
        let active2 = list_active(&mut conn).unwrap();
        assert_eq!(active2.len(), 0);
    }
}

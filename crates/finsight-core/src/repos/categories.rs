use crate::error::CoreResult;
use crate::models::{Category, CategoryGroup};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

pub fn list_groups(conn: &mut Connection) -> CoreResult<Vec<CategoryGroup>> {
    let mut stmt = conn.prepare(
        "SELECT id, label, hint, sort_order FROM category_groups ORDER BY sort_order, label",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(CategoryGroup {
            id: r.get(0)?,
            label: r.get(1)?,
            hint: r.get(2)?,
            sort_order: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, group_id, label, color, icon, spending_type, sort_order, archived_at \
         FROM categories ORDER BY sort_order, label",
    )?;
    let rows = stmt.query_map([], |r| {
        let archived_at_s: Option<String> = r.get(7)?;
        let archived_at = match archived_at_s {
            Some(s) => Some(
                DateTime::parse_from_rfc3339(&s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
            ),
            None => None,
        };
        Ok(Category {
            id: r.get(0)?,
            group_id: r.get(1)?,
            label: r.get(2)?,
            color: r.get(3)?,
            icon: r.get(4)?,
            spending_type: r.get(5)?,
            sort_order: r.get(6)?,
            archived_at,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update_color(conn: &mut Connection, id: &str, color: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE categories SET color = ?1 WHERE id = ?2",
        rusqlite::params![color, id],
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
        let db = Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn update_color_changes_category_color() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label) VALUES('daily', 'Daily')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) \
             VALUES('food', 'daily', 'Food', '#94A3B8', 0)",
            [],
        )
        .unwrap();

        update_color(&mut conn, "food", "#FF0000").unwrap();

        let color: String = conn
            .query_row("SELECT color FROM categories WHERE id = 'food'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(color, "#FF0000");
    }

    #[test]
    fn update_color_is_noop_for_missing_category() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // No rows should be updated, but the call must not fail.
        update_color(&mut conn, "missing", "#FF0000").unwrap();
    }
}

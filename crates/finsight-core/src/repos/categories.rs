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

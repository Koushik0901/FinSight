use crate::error::CoreResult;
use crate::models::{Category, CategoryGroup};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

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

/// Create a new category group. Returns the generated group (a slug of the
/// label, de-duplicated the same way `create()` de-duplicates category ids).
pub fn create_group(
    conn: &mut Connection,
    label: &str,
    hint: Option<&str>,
) -> CoreResult<CategoryGroup> {
    let label = label.trim();
    if label.is_empty() {
        return Err(crate::error::CoreError::InvalidState(
            "group label must not be empty".into(),
        ));
    }
    let base: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() { "group".to_string() } else { base };
    let mut id = base.clone();
    let mut n = 1;
    while conn
        .query_row("SELECT 1 FROM category_groups WHERE id = ?1", [&id], |_| Ok(()))
        .is_ok()
    {
        n += 1;
        id = format!("{base}-{n}");
    }

    let next_sort: i32 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM category_groups", [], |r| r.get(0))
        .unwrap_or(0);
    let hint = hint.map(str::trim).filter(|s| !s.is_empty());
    conn.execute(
        "INSERT INTO category_groups(id, label, hint, sort_order) VALUES(?1, ?2, ?3, ?4)",
        rusqlite::params![id, label, hint, next_sort],
    )?;
    Ok(CategoryGroup {
        id,
        label: label.to_string(),
        hint: hint.map(str::to_string),
        sort_order: next_sort,
    })
}

/// Move a category to a different (existing) group.
pub fn set_group(conn: &mut Connection, category_id: &str, group_id: &str) -> CoreResult<()> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM category_groups WHERE id = ?1",
            [group_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !exists {
        return Err(crate::error::CoreError::InvalidState(
            "category group not found".into(),
        ));
    }
    conn.execute(
        "UPDATE categories SET group_id = ?1 WHERE id = ?2",
        rusqlite::params![group_id, category_id],
    )?;
    Ok(())
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, group_id, label, color, icon, spending_type, guidance, sort_order, archived_at \
         FROM categories ORDER BY sort_order, label",
    )?;
    let rows = stmt.query_map([], |r| {
        let archived_at_s: Option<String> = r.get(8)?;
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
            guidance: r.get(6)?,
            sort_order: r.get(7)?,
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

/// Create a new user category. Returns its generated id (a slug of the label,
/// de-duplicated). The category joins the given group (or the first group).
pub fn create(
    conn: &mut Connection,
    label: &str,
    group_id: Option<&str>,
    color: &str,
) -> CoreResult<Category> {
    let label = label.trim();
    if label.is_empty() {
        return Err(crate::error::CoreError::InvalidState(
            "category label must not be empty".into(),
        ));
    }
    // Resolve group: given group, else first existing group.
    let group_id: String = match group_id {
        Some(g) if !g.is_empty() => g.to_string(),
        _ => conn
            .query_row(
                "SELECT id FROM category_groups ORDER BY sort_order, label LIMIT 1",
                [],
                |r| r.get(0),
            )
            .map_err(|_| {
                crate::error::CoreError::InvalidState("no category group exists".into())
            })?,
    };

    // Slug id from the label, de-duplicated against existing ids.
    let base: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() { "category".to_string() } else { base };
    let mut id = base.clone();
    let mut n = 1;
    while conn
        .query_row("SELECT 1 FROM categories WHERE id = ?1", [&id], |_| Ok(()))
        .is_ok()
    {
        n += 1;
        id = format!("{base}-{n}");
    }

    let next_sort: i32 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories", [], |r| r.get(0))
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, group_id, label, color, next_sort],
    )?;
    Ok(Category {
        id,
        group_id,
        label: label.to_string(),
        color: color.to_string(),
        icon: None,
        spending_type: None,
        guidance: None,
        sort_order: next_sort,
        archived_at: None,
    })
}

/// Rename a category. No-op if the id does not exist.
pub fn rename(conn: &mut Connection, id: &str, label: &str) -> CoreResult<()> {
    let label = label.trim();
    if label.is_empty() {
        return Err(crate::error::CoreError::InvalidState(
            "category label must not be empty".into(),
        ));
    }
    conn.execute(
        "UPDATE categories SET label = ?1 WHERE id = ?2",
        rusqlite::params![label, id],
    )?;
    Ok(())
}

/// Archive a category (soft delete). Transactions keep their category_id but the
/// category is hidden from active lists. Its rules are disabled.
pub fn archive(conn: &mut Connection, id: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE categories SET archived_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )?;
    conn.execute(
        "UPDATE rules SET enabled = 0 WHERE category_id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

/// Set (or clear, with None) the free-text categorizer/Copilot guidance.
pub fn set_guidance(conn: &mut Connection, id: &str, guidance: Option<&str>) -> CoreResult<()> {
    let g = guidance.map(str::trim).filter(|s| !s.is_empty());
    conn.execute(
        "UPDATE categories SET guidance = ?1 WHERE id = ?2",
        rusqlite::params![g, id],
    )?;
    Ok(())
}

/// All active categories that carry non-empty guidance, as (label, guidance).
/// Used to enrich the LLM categorizer prompt and Copilot context.
pub fn guidance_hints(conn: &mut Connection) -> CoreResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT label, guidance FROM categories \
         WHERE archived_at IS NULL AND guidance IS NOT NULL AND TRIM(guidance) <> '' \
         ORDER BY label",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
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

    fn seed_group(conn: &mut Connection) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('daily', 'Daily', 0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn create_rename_archive_and_guidance_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);

        let cat = create(&mut conn, "Coffee Shops", Some("daily"), "#FF0000").unwrap();
        assert_eq!(cat.id, "coffee-shops");
        assert_eq!(cat.label, "Coffee Shops");

        // Rename.
        rename(&mut conn, &cat.id, "Cafés").unwrap();
        // Guidance set + surfaced via guidance_hints.
        set_guidance(&mut conn, &cat.id, Some("Use for any coffee shop or café; exclude grocery stores.")).unwrap();
        let hints = guidance_hints(&mut conn).unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].0, "Cafés");
        assert!(hints[0].1.contains("coffee shop"));

        // Archive hides it from active guidance + the active list.
        archive(&mut conn, &cat.id).unwrap();
        assert!(guidance_hints(&mut conn).unwrap().is_empty());
        let active = list(&mut conn)
            .unwrap()
            .into_iter()
            .filter(|c| c.archived_at.is_none())
            .count();
        assert_eq!(active, 0);
    }

    #[test]
    fn create_deduplicates_slug_ids() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let a = create(&mut conn, "Travel", None, "#111").unwrap();
        let b = create(&mut conn, "Travel", None, "#222").unwrap();
        assert_eq!(a.id, "travel");
        assert_eq!(b.id, "travel-2");
    }

    #[test]
    fn create_rejects_empty_label() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        assert!(create(&mut conn, "   ", None, "#111").is_err());
    }

    #[test]
    fn create_group_slugs_and_dedups_like_create() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let a = create_group(&mut conn, "Side Hustle", Some("freelance income and costs")).unwrap();
        assert_eq!(a.id, "side-hustle");
        assert_eq!(a.hint.as_deref(), Some("freelance income and costs"));
        let b = create_group(&mut conn, "Side Hustle", None).unwrap();
        assert_eq!(b.id, "side-hustle-2");
    }

    #[test]
    fn create_group_rejects_empty_label() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        assert!(create_group(&mut conn, "   ", None).is_err());
    }

    #[test]
    fn set_group_moves_a_category() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        let new_group = create_group(&mut conn, "Lifestyle", None).unwrap();
        set_group(&mut conn, &cat.id, &new_group.id).unwrap();
        let moved = list(&mut conn).unwrap().into_iter().find(|c| c.id == cat.id).unwrap();
        assert_eq!(moved.group_id, new_group.id);
    }

    #[test]
    fn set_group_rejects_nonexistent_group() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        assert!(set_group(&mut conn, &cat.id, "does-not-exist").is_err());
    }
}

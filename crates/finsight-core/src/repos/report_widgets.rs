use crate::error::{CoreError, CoreResult};
use crate::models::report_widget::{
    validate_chart_type, validate_period, validate_split_by, ReportWidget,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn row_to_widget(r: &rusqlite::Row) -> rusqlite::Result<ReportWidget> {
    Ok(ReportWidget {
        id: r.get(0)?,
        position: r.get(1)?,
        title: r.get(2)?,
        chart_type: r.get(3)?,
        split_by: r.get(4)?,
        period: r.get(5)?,
        filters_json: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

fn validate_title(title: &str) -> CoreResult<()> {
    let t = title.trim();
    if t.is_empty() {
        return Err(CoreError::Validation("title must be non-empty".to_string()));
    }
    if t.len() > 120 {
        return Err(CoreError::Validation(
            "title must be <= 120 characters".to_string(),
        ));
    }
    Ok(())
}

fn normalize_filters(filters_json: Option<&str>) -> CoreResult<String> {
    let raw = filters_json.unwrap_or("{}").trim();
    let s = if raw.is_empty() { "{}" } else { raw };
    // Must be valid JSON object.
    let v: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| CoreError::Validation(format!("filters_json must be valid JSON: {e}")))?;
    if !v.is_object() {
        return Err(CoreError::Validation(
            "filters_json must be a JSON object".to_string(),
        ));
    }
    // Canonicalize.
    Ok(serde_json::to_string(&v).unwrap())
}

/// List all widgets ordered by position ASC, id ASC.
pub fn list_widgets(conn: &Connection) -> CoreResult<Vec<ReportWidget>> {
    // Lazy-seed default report when empty so a fresh user sees familiar Reports.
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM report_widgets", [], |r| r.get(0))?;
    if count == 0 {
        // Seed is best-effort; if it fails (e.g. concurrent writer seeded first),
        // fall through to normal list.
        let _ = seed_default_widgets(conn);
    }
    let mut stmt = conn.prepare(
        "SELECT id, position, title, chart_type, split_by, period, filters_json, created_at, updated_at \
         FROM report_widgets ORDER BY position ASC, id ASC",
    )?;
    let rows = stmt.query_map([], row_to_widget)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn seed_default_widgets(conn: &Connection) -> CoreResult<()> {
    // 5 widgets matching current Reports.tsx default report.
    // Order matches brief §4 Layout Strategy.
    let now = Utc::now().to_rfc3339();
    let defaults: Vec<(&str, &str, &str, &str, &str)> = vec![
        // title, chart_type, split_by, period, filters_json
        (
            "Monthly overview",
            "bar",
            "month",
            "All",
            r#"{"includeTransfers":false,"includeArchived":false}"#,
        ),
        (
            "Spending by category",
            "bar",
            "category",
            "All",
            r#"{"includeTransfers":false,"includeArchived":false}"#,
        ),
        (
            "Top categories",
            "table",
            "category",
            "All",
            r#"{"includeTransfers":false,"includeArchived":false}"#,
        ),
        (
            "Top merchants",
            "table",
            "payee",
            "All",
            r#"{"includeTransfers":false,"includeArchived":false}"#,
        ),
        (
            "Net worth",
            "area",
            "month",
            "All",
            r#"{"includeTransfers":false,"includeArchived":false}"#,
        ),
    ];
    for (idx, (title, chart_type, split_by, period, filters_json)) in defaults.into_iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO report_widgets(id, position, title, chart_type, split_by, period, filters_json, created_at, updated_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![id, idx as i64, title, chart_type, split_by, period, filters_json, now],
        );
    }
    Ok(())
}

/// Get one widget by id.
pub fn get_widget(conn: &Connection, id: &str) -> CoreResult<Option<ReportWidget>> {
    let v = conn
        .query_row(
            "SELECT id, position, title, chart_type, split_by, period, filters_json, created_at, updated_at \
             FROM report_widgets WHERE id = ?1",
            params![id],
            row_to_widget,
        )
        .optional()?;
    Ok(v)
}

/// Create a widget. Position defaults to append at end.
pub fn create_widget(
    conn: &mut Connection,
    title: &str,
    chart_type: &str,
    split_by: &str,
    period: &str,
    filters_json: Option<&str>,
    position: Option<i64>,
) -> CoreResult<ReportWidget> {
    validate_title(title)?;
    if !validate_chart_type(chart_type) {
        return Err(CoreError::Validation(format!(
            "invalid chart_type `{chart_type}`"
        )));
    }
    if !validate_split_by(split_by) {
        return Err(CoreError::Validation(format!("invalid split_by `{split_by}`")));
    }
    if !validate_period(period) {
        return Err(CoreError::Validation(format!("invalid period `{period}`")));
    }
    let filters = normalize_filters(filters_json)?;
    let title = title.trim().to_string();
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();

    let pos = if let Some(p) = position {
        if p < 0 {
            return Err(CoreError::Validation("position must be >= 0".to_string()));
        }
        // Shift down existing widgets at >= p to make room.
        conn.execute(
            "UPDATE report_widgets SET position = position + 1, updated_at = ?1 WHERE position >= ?2",
            params![now, p],
        )?;
        p
    } else {
        let max_pos: Option<i64> =
            conn.query_row("SELECT MAX(position) FROM report_widgets", [], |r| r.get(0))?;
        max_pos.map(|m| m + 1).unwrap_or(0)
    };

    conn.execute(
        "INSERT INTO report_widgets(id, position, title, chart_type, split_by, period, filters_json, created_at, updated_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![id, pos, title, chart_type, split_by, period, filters, now],
    )?;
    Ok(ReportWidget {
        id,
        position: pos,
        title,
        chart_type: chart_type.to_string(),
        split_by: split_by.to_string(),
        period: period.to_string(),
        filters_json: filters,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Patch a widget. Returns None if not found.
pub fn update_widget(
    conn: &mut Connection,
    id: &str,
    title: Option<&str>,
    chart_type: Option<&str>,
    split_by: Option<&str>,
    period: Option<&str>,
    filters_json: Option<&str>,
) -> CoreResult<Option<ReportWidget>> {
    let Some(cur) = get_widget(conn, id)? else {
        return Ok(None);
    };
    let new_title = title.unwrap_or(&cur.title).trim().to_string();
    validate_title(&new_title)?;
    let new_chart = chart_type.unwrap_or(&cur.chart_type);
    if !validate_chart_type(new_chart) {
        return Err(CoreError::Validation(format!(
            "invalid chart_type `{new_chart}`"
        )));
    }
    let new_split = split_by.unwrap_or(&cur.split_by);
    if !validate_split_by(new_split) {
        return Err(CoreError::Validation(format!("invalid split_by `{new_split}`")));
    }
    let new_period = period.unwrap_or(&cur.period);
    if !validate_period(new_period) {
        return Err(CoreError::Validation(format!("invalid period `{new_period}`")));
    }
    let new_filters = if let Some(fj) = filters_json {
        normalize_filters(Some(fj))?
    } else {
        cur.filters_json.clone()
    };
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE report_widgets SET title=?1, chart_type=?2, split_by=?3, period=?4, filters_json=?5, updated_at=?6 WHERE id=?7",
        params![new_title, new_chart, new_split, new_period, new_filters, now, id],
    )?;
    Ok(Some(ReportWidget {
        id: id.to_string(),
        position: cur.position,
        title: new_title,
        chart_type: new_chart.to_string(),
        split_by: new_split.to_string(),
        period: new_period.to_string(),
        filters_json: new_filters,
        created_at: cur.created_at,
        updated_at: now,
    }))
}

/// Delete by id. Returns true if a row was deleted. Compacts positions to keep 0..N-1 dense
/// so future inserts don't leave gaps (Actual keeps dense ordering).
pub fn delete_widget(conn: &mut Connection, id: &str) -> CoreResult<bool> {
    let Some(cur) = get_widget(conn, id)? else {
        return Ok(false);
    };
    crate::repos::atomic(conn, |conn| {
        let n = conn.execute("DELETE FROM report_widgets WHERE id = ?1", params![id])?;
        if n == 0 {
            return Ok(false);
        }
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE report_widgets SET position = position - 1, updated_at = ?1 WHERE position > ?2",
            params![now, cur.position],
        )?;
        Ok(true)
    })
}

/// Reorder widgets to exactly the order of `ordered_ids`. The caller must
/// supply *all* existing ids exactly once, in desired order.
/// Idempotent and transactional.
pub fn reorder_widgets(conn: &mut Connection, ordered_ids: &[String]) -> CoreResult<Vec<ReportWidget>> {
    if ordered_ids.is_empty() {
        return Err(CoreError::Validation(
            "ordered_ids must be non-empty".to_string(),
        ));
    }
    let existing = list_widgets(conn)?;
    // For reorder we want the true existing set before lazy-seed? Already seeded.
    // Validate that the supplied set is exactly the existing set.
    let existing_ids: std::collections::HashSet<String> =
        existing.iter().map(|w| w.id.clone()).collect();
    let supplied_ids: std::collections::HashSet<String> = ordered_ids.iter().cloned().collect();
    if existing_ids != supplied_ids {
        return Err(CoreError::Validation(format!(
            "ordered_ids must contain exactly all {} widget ids once",
            existing.len()
        )));
    }
    if ordered_ids.len() != existing.len() {
        return Err(CoreError::Validation(
            "ordered_ids length mismatch".to_string(),
        ));
    }
    // Check duplicate ids in supplied vec.
    if supplied_ids.len() != ordered_ids.len() {
        return Err(CoreError::Validation(
            "ordered_ids contains duplicate ids".to_string(),
        ));
    }

    crate::repos::atomic(conn, |conn| {
        let now = Utc::now().to_rfc3339();
        for (pos, id) in ordered_ids.iter().enumerate() {
            conn.execute(
                "UPDATE report_widgets SET position = ?1, updated_at = ?2 WHERE id = ?3",
                params![pos as i64, now, id],
            )?;
        }
        Ok(())
    })?;
    list_widgets(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        crate::testing::migrated_db()
    }

    #[test]
    fn list_seeds_default_widgets_when_empty() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let widgets = list_widgets(&conn).unwrap();
        assert_eq!(widgets.len(), 5);
        assert_eq!(widgets[0].title, "Monthly overview");
        assert_eq!(widgets[0].position, 0);
        assert_eq!(widgets[4].title, "Net worth");
        // Second list does not re-seed.
        let widgets2 = list_widgets(&conn).unwrap();
        assert_eq!(widgets2.len(), 5);
    }

    #[test]
    fn create_append_and_position_insert() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let _ = list_widgets(&conn).unwrap(); // seed
        let w = create_widget(
            &mut conn,
            "My widget",
            "table",
            "category",
            "All",
            Some(r#"{"includeTransfers":true}"#),
            None,
        )
        .unwrap();
        assert_eq!(w.position, 5);
        assert_eq!(w.title, "My widget");
        // Insert at 0 shifts others down.
        let w2 = create_widget(&mut conn, "Top", "bar", "payee", "YTD", None, Some(0)).unwrap();
        assert_eq!(w2.position, 0);
        let widgets = list_widgets(&conn).unwrap();
        assert_eq!(widgets[0].id, w2.id);
        assert_eq!(widgets.len(), 7);
        // Positions dense 0..6
        for (i, w) in widgets.iter().enumerate() {
            assert_eq!(w.position, i as i64);
        }
    }

    #[test]
    fn update_patch_and_validation() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let widgets = list_widgets(&conn).unwrap();
        let id = widgets[0].id.clone();
        let updated = update_widget(&mut conn, &id, Some("Renamed"), Some("donut"), None, None, None)
            .unwrap()
            .unwrap();
        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.chart_type, "donut");
        // invalid
        let err = update_widget(&mut conn, &id, None, Some("bad"), None, None, None).unwrap_err();
        assert!(format!("{err}").contains("invalid chart_type"));
    }

    #[test]
    fn delete_compacts_positions() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let widgets = list_widgets(&conn).unwrap();
        let id_mid = widgets[2].id.clone();
        assert!(delete_widget(&mut conn, &id_mid).unwrap());
        let widgets = list_widgets(&conn).unwrap();
        assert_eq!(widgets.len(), 4);
        for (i, w) in widgets.iter().enumerate() {
            assert_eq!(w.position, i as i64);
        }
        assert!(!delete_widget(&mut conn, "nope").unwrap());
    }

    #[test]
    fn reorder_exact_set() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let widgets = list_widgets(&conn).unwrap();
        let mut ids: Vec<String> = widgets.iter().map(|w| w.id.clone()).collect();
        ids.reverse();
        let reordered = reorder_widgets(&mut conn, &ids).unwrap();
        assert_eq!(reordered[0].id, ids[0]);
        assert_eq!(reordered[4].id, ids[4]);
        // Missing id should fail.
        let mut bad = ids.clone();
        bad.pop();
        let err = reorder_widgets(&mut conn, &bad).unwrap_err();
        assert!(format!("{err}").contains("exactly all"));
    }

    #[test]
    fn validation_rejects_bad_inputs() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let _ = list_widgets(&conn).unwrap();
        let err = create_widget(&mut conn, "", "table", "category", "All", None, None).unwrap_err();
        assert!(format!("{err}").contains("title"));
        let err = create_widget(&mut conn, "x", "bad", "category", "All", None, None).unwrap_err();
        assert!(format!("{err}").contains("chart_type"));
        let err = create_widget(&mut conn, "x", "table", "bad", "All", None, None).unwrap_err();
        assert!(format!("{err}").contains("split_by"));
        let err = create_widget(&mut conn, "x", "table", "category", "Bad", None, None).unwrap_err();
        assert!(format!("{err}").contains("period"));
        let err = create_widget(&mut conn, "x", "table", "category", "All", Some("not json"), None).unwrap_err();
        assert!(format!("{err}").contains("valid JSON"));
    }
}

use crate::error::CoreResult;
use crate::models::{AgentActionBundle, AgentActionItem, AgentExecutionEntry};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

fn map_bundle_row(r: &rusqlite::Row) -> rusqlite::Result<AgentActionBundle> {
    Ok(AgentActionBundle {
        id: r.get(0)?,
        session_id: r.get(1)?,
        title: r.get(2)?,
        summary: r.get(3)?,
        rationale: r.get(4)?,
        confidence: r.get(5)?,
        status: r.get(6)?,
        provider_id: r.get(7)?,
        model_id: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        items: Vec::new(),
    })
}

fn map_item_row(r: &rusqlite::Row) -> rusqlite::Result<AgentActionItem> {
    Ok(AgentActionItem {
        id: r.get(0)?,
        bundle_id: r.get(1)?,
        action_kind: r.get(2)?,
        payload_json: r.get(3)?,
        preview_json: r.get(4)?,
        rationale: r.get(5)?,
        confidence: r.get(6)?,
        status: r.get(7)?,
        validation_errors: r.get(8)?,
        sort_order: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

fn map_execution_row(r: &rusqlite::Row) -> rusqlite::Result<AgentExecutionEntry> {
    Ok(AgentExecutionEntry {
        id: r.get(0)?,
        item_id: r.get(1)?,
        bundle_id: r.get(2)?,
        action_kind: r.get(3)?,
        status: r.get(4)?,
        result_json: r.get(5)?,
        error: r.get(6)?,
        executed_at: r.get(7)?,
    })
}

fn load_items(conn: &mut Connection, bundle_id: &str) -> CoreResult<Vec<AgentActionItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, bundle_id, action_kind, payload_json, preview_json, rationale,
                confidence, status, validation_errors, sort_order, created_at, updated_at
         FROM agent_action_items
         WHERE bundle_id = ?1
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = stmt.query_map(params![bundle_id], map_item_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert_bundle(
    conn: &mut Connection,
    session_id: Option<&str>,
    title: &str,
    summary: &str,
    rationale: &str,
    confidence: f64,
    provider_id: Option<&str>,
    model_id: Option<&str>,
) -> CoreResult<AgentActionBundle> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_action_bundles(
            id, session_id, title, summary, rationale, confidence, status,
            provider_id, model_id, created_at, updated_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8, ?9, ?9)",
        params![
            id,
            session_id,
            title,
            summary,
            rationale,
            confidence,
            provider_id,
            model_id,
            now
        ],
    )?;
    Ok(AgentActionBundle {
        id,
        session_id: session_id.map(str::to_string),
        title: title.to_string(),
        summary: summary.to_string(),
        rationale: rationale.to_string(),
        confidence,
        status: "pending".to_string(),
        provider_id: provider_id.map(str::to_string),
        model_id: model_id.map(str::to_string),
        created_at: now.clone(),
        updated_at: now,
        items: Vec::new(),
    })
}

pub fn insert_item(
    conn: &mut Connection,
    bundle_id: &str,
    action_kind: &str,
    payload_json: &str,
    rationale: &str,
    confidence: f64,
    sort_order: i64,
) -> CoreResult<AgentActionItem> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_action_items(
            id, bundle_id, action_kind, payload_json, rationale, confidence,
            status, sort_order, created_at, updated_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8, ?8)",
        params![
            id,
            bundle_id,
            action_kind,
            payload_json,
            rationale,
            confidence,
            sort_order,
            now
        ],
    )?;
    Ok(AgentActionItem {
        id,
        bundle_id: bundle_id.to_string(),
        action_kind: action_kind.to_string(),
        payload_json: payload_json.to_string(),
        preview_json: None,
        rationale: rationale.to_string(),
        confidence,
        status: "pending".to_string(),
        validation_errors: None,
        sort_order,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn get_bundle(conn: &mut Connection, id: &str) -> CoreResult<Option<AgentActionBundle>> {
    let bundle = match conn.query_row(
        "SELECT id, session_id, title, summary, rationale, confidence, status,
                provider_id, model_id, created_at, updated_at
         FROM agent_action_bundles
         WHERE id = ?1",
        params![id],
        map_bundle_row,
    ) {
        Ok(bundle) => Some(bundle),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(err) => return Err(err.into()),
    };

    bundle
        .map(|mut bundle| {
            bundle.items = load_items(conn, &bundle.id)?;
            Ok(bundle)
        })
        .transpose()
}

pub fn list_bundles(
    conn: &mut Connection,
    status_filter: Option<&str>,
    session_id: Option<&str>,
    limit: u32,
) -> CoreResult<Vec<AgentActionBundle>> {
    let mut bundles = Vec::new();
    let mut sql = String::from(
        "SELECT id, session_id, title, summary, rationale, confidence, status,
                provider_id, model_id, created_at, updated_at
         FROM agent_action_bundles",
    );
    let mut conditions = Vec::new();
    let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(status) = status_filter {
        conditions.push("status = ?".to_string());
        query_params.push(Box::new(status.to_string()));
    }
    if let Some(session_id) = session_id {
        conditions.push("session_id = ?".to_string());
        query_params.push(Box::new(session_id.to_string()));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ?");
    query_params.push(Box::new(limit as i64));

    {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(query_params.iter().map(|p| p.as_ref())),
            map_bundle_row,
        )?;
        for row in rows {
            bundles.push(row?);
        }
    }

    for bundle in &mut bundles {
        bundle.items = load_items(conn, &bundle.id)?;
    }

    Ok(bundles)
}

pub fn set_bundle_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agent_action_bundles
         SET status = ?1, updated_at = ?2
         WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

pub fn set_item_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agent_action_items
         SET status = ?1, updated_at = ?2
         WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

pub fn set_item_preview(conn: &mut Connection, id: &str, preview_json: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agent_action_items
         SET preview_json = ?1, updated_at = ?2
         WHERE id = ?3",
        params![preview_json, now, id],
    )?;
    Ok(())
}

pub fn set_item_validation_errors(
    conn: &mut Connection,
    id: &str,
    errors_json: &str,
) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agent_action_items
         SET validation_errors = ?1, updated_at = ?2
         WHERE id = ?3",
        params![errors_json, now, id],
    )?;
    Ok(())
}

pub fn insert_execution_log_entry(
    conn: &mut Connection,
    item_id: &str,
    bundle_id: &str,
    action_kind: &str,
    status: &str,
    result_json: Option<&str>,
    error: Option<&str>,
) -> CoreResult<AgentExecutionEntry> {
    let id = Uuid::new_v4().to_string();
    let executed_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_execution_log(
            id, item_id, bundle_id, action_kind, status, result_json, error, executed_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            item_id,
            bundle_id,
            action_kind,
            status,
            result_json,
            error,
            executed_at
        ],
    )?;
    Ok(AgentExecutionEntry {
        id,
        item_id: item_id.to_string(),
        bundle_id: bundle_id.to_string(),
        action_kind: action_kind.to_string(),
        status: status.to_string(),
        result_json: result_json.map(str::to_string),
        error: error.map(str::to_string),
        executed_at,
    })
}

pub fn list_execution_log(
    conn: &mut Connection,
    bundle_id: &str,
) -> CoreResult<Vec<AgentExecutionEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, item_id, bundle_id, action_kind, status, result_json, error, executed_at
         FROM agent_execution_log
         WHERE bundle_id = ?1
         ORDER BY executed_at DESC",
    )?;
    let rows = stmt.query_map(params![bundle_id], map_execution_row)?;
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
        let db = Db::open(&dir.path().join("copilot-actions.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_bundle_and_items_loads_nested_items() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let bundle = insert_bundle(
            &mut conn,
            None,
            "Budget fixes",
            "Rebalance envelopes",
            "Detected overspend",
            0.82,
            Some("anthropic"),
            Some("claude"),
        )
        .unwrap();
        let item_b = insert_item(
            &mut conn,
            &bundle.id,
            "set_budget",
            r#"{"categoryId":"groceries"}"#,
            "Increase groceries",
            0.9,
            2,
        )
        .unwrap();
        let item_a = insert_item(
            &mut conn,
            &bundle.id,
            "set_budget",
            r#"{"categoryId":"transport"}"#,
            "Decrease transport",
            0.7,
            1,
        )
        .unwrap();

        let fetched = get_bundle(&mut conn, &bundle.id).unwrap().unwrap();
        assert_eq!(fetched.items.len(), 2);
        assert_eq!(fetched.items[0].id, item_a.id);
        assert_eq!(fetched.items[1].id, item_b.id);

        let listed = list_bundles(&mut conn, Some("pending"), None, 10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].items.len(), 2);
    }

    #[test]
    fn status_and_execution_updates_persist() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let bundle = insert_bundle(
            &mut conn,
            None,
            "Rules",
            "Create rule changes",
            "Repeated user actions",
            0.66,
            None,
            None,
        )
        .unwrap();
        let item = insert_item(
            &mut conn,
            &bundle.id,
            "create_rule",
            r#"{"pattern":"NETFLIX"}"#,
            "Common merchant",
            0.8,
            0,
        )
        .unwrap();

        set_bundle_status(&mut conn, &bundle.id, "reviewed").unwrap();
        set_item_status(&mut conn, &item.id, "approved").unwrap();
        set_item_preview(&mut conn, &item.id, r#"{"label":"Netflix → Streaming"}"#).unwrap();
        set_item_validation_errors(&mut conn, &item.id, r#"["none"]"#).unwrap();
        let entry = insert_execution_log_entry(
            &mut conn,
            &item.id,
            &bundle.id,
            "create_rule",
            "applied",
            Some(r#"{"ruleId":"r1"}"#),
            None,
        )
        .unwrap();

        let fetched = get_bundle(&mut conn, &bundle.id).unwrap().unwrap();
        assert_eq!(fetched.status, "reviewed");
        assert_eq!(fetched.items[0].status, "approved");
        assert_eq!(
            fetched.items[0].preview_json.as_deref(),
            Some(r#"{"label":"Netflix → Streaming"}"#)
        );
        assert_eq!(
            fetched.items[0].validation_errors.as_deref(),
            Some(r#"["none"]"#)
        );

        let log = list_execution_log(&mut conn, &bundle.id).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].id, entry.id);
        assert_eq!(log[0].status, "applied");
    }
}

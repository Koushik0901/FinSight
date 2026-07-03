use crate::error::CoreResult;
use crate::models::{ConversationMessage, ConversationSummary};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

// ── conversations ────────────────────────────────────────────────────────────

pub fn create_conversation(conn: &mut Connection, id: &str) -> CoreResult<ConversationSummary> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO conversations(id, title, created_at, updated_at)
         VALUES(?1, 'New conversation', ?2, ?2)",
        params![id, now],
    )?;
    Ok(ConversationSummary {
        id: id.to_string(),
        title: "New conversation".to_string(),
        message_count: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn update_conversation_title(conn: &mut Connection, id: &str, title: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, id],
    )?;
    Ok(())
}

pub fn touch_conversation(conn: &mut Connection, id: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn list_conversations(conn: &mut Connection) -> CoreResult<Vec<ConversationSummary>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title, c.created_at, c.updated_at,
                COUNT(m.id) AS message_count
         FROM conversations c
         LEFT JOIN conversation_messages m ON m.conversation_id = c.id
         GROUP BY c.id
         ORDER BY c.updated_at DESC
         LIMIT 100",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ConversationSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            created_at: r.get(2)?,
            updated_at: r.get(3)?,
            message_count: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete_conversation(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
    Ok(())
}

// ── conversation messages ────────────────────────────────────────────────────

pub fn insert_message(
    conn: &mut Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    tool_trace: Option<&str>,
    action_bundle_id: Option<&str>,
    branch_parent_id: Option<&str>,
    parts_json: Option<&str>,
) -> CoreResult<ConversationMessage> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO conversation_messages
             (id, conversation_id, role, content, tool_trace, action_bundle_id, branch_parent_id, parts_json, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            conversation_id,
            role,
            content,
            tool_trace,
            action_bundle_id,
            branch_parent_id,
            parts_json,
            now,
        ],
    )?;
    // bump conversation.updated_at
    touch_conversation(conn, conversation_id)?;
    Ok(ConversationMessage {
        id,
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tool_trace: tool_trace.map(|s| s.to_string()),
        action_bundle_id: action_bundle_id.map(|s| s.to_string()),
        branch_parent_id: branch_parent_id.map(|s| s.to_string()),
        parts_json: parts_json.map(|s| s.to_string()),
        run_status: "completed".to_string(),
        ag_ui_metadata_json: None,
        created_at: now,
    })
}

pub fn list_messages(
    conn: &mut Connection,
    conversation_id: &str,
) -> CoreResult<Vec<ConversationMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, tool_trace,
                action_bundle_id, branch_parent_id, parts_json, run_status, ag_ui_metadata_json, created_at
         FROM conversation_messages
         WHERE conversation_id = ?1
         ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![conversation_id], |r| {
        Ok(ConversationMessage {
            id: r.get(0)?,
            conversation_id: r.get(1)?,
            role: r.get(2)?,
            content: r.get(3)?,
            tool_trace: r.get(4)?,
            action_bundle_id: r.get(5)?,
            branch_parent_id: r.get(6)?,
            parts_json: r.get(7)?,
            run_status: r.get(8)?,
            ag_ui_metadata_json: r.get(9)?,
            created_at: r.get(10)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update_message_run_status(
    conn: &mut Connection,
    message_id: &str,
    run_status: &str,
    ag_ui_metadata_json: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE conversation_messages
         SET run_status = ?1, ag_ui_metadata_json = ?2
         WHERE id = ?3",
        params![run_status, ag_ui_metadata_json, message_id],
    )?;
    Ok(())
}

pub fn update_user_message(
    conn: &mut Connection,
    message_id: &str,
    content: &str,
    parts_json: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE conversation_messages
         SET content = ?1, parts_json = ?2
         WHERE id = ?3 AND role = 'user'",
        params![content, parts_json, message_id],
    )?;
    Ok(())
}

pub fn delete_messages_after(
    conn: &mut Connection,
    conversation_id: &str,
    message_id: &str,
) -> CoreResult<usize> {
    let created_at: String = conn.query_row(
        "SELECT created_at FROM conversation_messages
         WHERE id = ?1 AND conversation_id = ?2",
        params![message_id, conversation_id],
        |r| r.get(0),
    )?;
    let deleted = conn.execute(
        "DELETE FROM conversation_messages
         WHERE conversation_id = ?1 AND created_at > ?2",
        params![conversation_id, created_at],
    )?;
    touch_conversation(conn, conversation_id)?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("conversations.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn create_and_list_conversations() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let id = Uuid::new_v4().to_string();
        let conv = create_conversation(&mut conn, &id).unwrap();
        assert_eq!(conv.title, "New conversation");
        assert_eq!(conv.message_count, 0);

        let list = list_conversations(&mut conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }

    #[test]
    fn update_title_and_touch() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let id = Uuid::new_v4().to_string();
        create_conversation(&mut conn, &id).unwrap();
        update_conversation_title(&mut conn, &id, "Budget review").unwrap();

        let list = list_conversations(&mut conn).unwrap();
        assert_eq!(list[0].title, "Budget review");
    }

    #[test]
    fn insert_messages_and_list() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let conv_id = Uuid::new_v4().to_string();
        create_conversation(&mut conn, &conv_id).unwrap();

        insert_message(
            &mut conn, &conv_id, "user", "Hello?", None, None, None, None,
        )
        .unwrap();
        insert_message(
            &mut conn,
            &conv_id,
            "assistant",
            "Hi! Here's your budget.",
            Some("[\"budget_envelopes\"]"),
            None,
            None,
            Some("[{\"type\":\"text\",\"text\":\"Hi! Here's your budget.\"}]"),
        )
        .unwrap();

        let msgs = list_messages(&mut conn, &conv_id).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(
            msgs[1].tool_trace.as_deref(),
            Some("[\"budget_envelopes\"]")
        );
        assert!(msgs[1]
            .parts_json
            .as_deref()
            .is_some_and(|v| v.contains("\"text\"")));

        // message_count via list_conversations
        let list = list_conversations(&mut conn).unwrap();
        assert_eq!(list[0].message_count, 2);
    }

    #[test]
    fn delete_conversation_cascades() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let id = Uuid::new_v4().to_string();
        create_conversation(&mut conn, &id).unwrap();
        insert_message(&mut conn, &id, "user", "test", None, None, None, None).unwrap();

        delete_conversation(&mut conn, &id).unwrap();
        let list = list_conversations(&mut conn).unwrap();
        assert!(list.is_empty());
        let msgs = list_messages(&mut conn, &id).unwrap();
        assert!(msgs.is_empty());
    }
}

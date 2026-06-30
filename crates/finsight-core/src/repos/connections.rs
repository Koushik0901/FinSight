//! CRUD for SimpleFin connections.

use crate::error::CoreResult;
use crate::models::{NewSimpleFinConnection, SimpleFinConnection, SimpleFinConnectionPatch};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn insert(
    conn: &mut Connection,
    input: NewSimpleFinConnection,
) -> CoreResult<SimpleFinConnection> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO simplefin_connections \
         (id, access_url_ref, conn_id, org_id, org_name, org_url, sfin_url, label, status, last_error, last_synced_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            &id,
            &input.access_url_ref,
            &input.conn_id,
            &input.org_id,
            &input.org_name,
            &input.org_url,
            &input.sfin_url,
            &input.label,
            "active",
            None::<&str>,
            None::<&str>,
            now.to_rfc3339(),
        ],
    )?;
    Ok(SimpleFinConnection {
        id,
        access_url_ref: input.access_url_ref,
        conn_id: input.conn_id,
        org_id: input.org_id,
        org_name: input.org_name,
        org_url: input.org_url,
        sfin_url: input.sfin_url,
        label: input.label,
        status: "active".to_string(),
        last_error: None,
        last_synced_at: None,
        created_at: now,
    })
}

pub fn upsert_by_conn_id(
    conn: &mut Connection,
    input: NewSimpleFinConnection,
) -> CoreResult<SimpleFinConnection> {
    if let Some(conn_id) = input.conn_id.as_deref() {
        if let Some(existing) = find_by_conn_id(conn, conn_id)? {
            conn.execute(
                "UPDATE simplefin_connections SET \
                    access_url_ref = ?1, \
                    org_id = ?2, \
                    org_name = ?3, \
                    org_url = ?4, \
                    sfin_url = ?5, \
                    label = ?6, \
                    status = 'active', \
                    last_error = NULL \
                 WHERE id = ?7",
                params![
                    &input.access_url_ref,
                    &input.org_id,
                    &input.org_name,
                    &input.org_url,
                    &input.sfin_url,
                    &input.label,
                    &existing.id,
                ],
            )?;
            return get(conn, &existing.id);
        }
    }

    insert(conn, input)
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<SimpleFinConnection>> {
    let mut stmt = conn.prepare(
        "SELECT id, access_url_ref, conn_id, org_id, org_name, org_url, sfin_url, label, status, last_error, last_synced_at, created_at \
         FROM simplefin_connections ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let last_synced_s: Option<String> = r.get(10)?;
        let created_s: String = r.get(11)?;
        Ok(SimpleFinConnection {
            id: r.get(0)?,
            access_url_ref: r.get(1)?,
            conn_id: r.get(2)?,
            org_id: r.get(3)?,
            org_name: r.get(4)?,
            org_url: r.get(5)?,
            sfin_url: r.get(6)?,
            label: r.get(7)?,
            status: r.get(8)?,
            last_error: r.get(9)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .unwrap()
                .with_timezone(&Utc),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<SimpleFinConnection> {
    conn.query_row(
        "SELECT id, access_url_ref, conn_id, org_id, org_name, org_url, sfin_url, label, status, last_error, last_synced_at, created_at \
         FROM simplefin_connections WHERE id = ?1",
        params![id],
        |r| {
            let last_synced_s: Option<String> = r.get(10)?;
            let created_s: String = r.get(11)?;
            Ok(SimpleFinConnection {
                id: r.get(0)?,
                access_url_ref: r.get(1)?,
                conn_id: r.get(2)?,
                org_id: r.get(3)?,
                org_name: r.get(4)?,
                org_url: r.get(5)?,
                sfin_url: r.get(6)?,
                label: r.get(7)?,
                status: r.get(8)?,
                last_error: r.get(9)?,
                last_synced_at: last_synced_s.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                created_at: DateTime::parse_from_rfc3339(&created_s)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    )
    .map_err(Into::into)
}

pub fn update(
    conn: &mut Connection,
    id: &str,
    patch: SimpleFinConnectionPatch,
) -> CoreResult<SimpleFinConnection> {
    if let Some(status) = &patch.status {
        conn.execute(
            "UPDATE simplefin_connections SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
    }
    if let Some(last_error) = &patch.last_error {
        conn.execute(
            "UPDATE simplefin_connections SET last_error = ?1 WHERE id = ?2",
            params![last_error, id],
        )?;
    }
    if let Some(last_synced_at) = &patch.last_synced_at {
        conn.execute(
            "UPDATE simplefin_connections SET last_synced_at = ?1 WHERE id = ?2",
            params![last_synced_at.map(|d| d.to_rfc3339()), id],
        )?;
    }
    if let Some(label) = &patch.label {
        conn.execute(
            "UPDATE simplefin_connections SET label = ?1 WHERE id = ?2",
            params![label, id],
        )?;
    }
    if let Some(org_name) = &patch.org_name {
        conn.execute(
            "UPDATE simplefin_connections SET org_name = ?1 WHERE id = ?2",
            params![org_name, id],
        )?;
    }
    get(conn, id)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM simplefin_connections WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn find_by_conn_id(
    conn: &mut Connection,
    conn_id: &str,
) -> CoreResult<Option<SimpleFinConnection>> {
    let mut stmt = conn.prepare(
        "SELECT id, access_url_ref, conn_id, org_id, org_name, org_url, sfin_url, label, status, last_error, last_synced_at, created_at \
         FROM simplefin_connections WHERE conn_id = ?1",
    )?;
    let mut rows = stmt.query_map(params![conn_id], |r| {
        let last_synced_s: Option<String> = r.get(10)?;
        let created_s: String = r.get(11)?;
        Ok(SimpleFinConnection {
            id: r.get(0)?,
            access_url_ref: r.get(1)?,
            conn_id: r.get(2)?,
            org_id: r.get(3)?,
            org_name: r.get(4)?,
            org_url: r.get(5)?,
            sfin_url: r.get(6)?,
            label: r.get(7)?,
            status: r.get(8)?,
            last_error: r.get(9)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .unwrap()
                .with_timezone(&Utc),
        })
    })?;
    Ok(rows.next().transpose()?)
}

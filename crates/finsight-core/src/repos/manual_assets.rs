use crate::error::CoreResult;
use crate::models::{ManualAsset, ManualAssetPatch, NewManualAsset};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list(conn: &mut Connection) -> CoreResult<Vec<ManualAsset>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, asset_type, value_cents, currency, notes, created_at, updated_at \
         FROM manual_assets ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], map_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn create(conn: &mut Connection, a: NewManualAsset) -> CoreResult<ManualAsset> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO manual_assets(id, name, asset_type, value_cents, currency, notes, created_at, updated_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, a.name, a.asset_type, a.value_cents, a.currency, a.notes, now],
    )?;
    Ok(ManualAsset {
        id,
        name: a.name,
        asset_type: a.asset_type,
        value_cents: a.value_cents,
        currency: a.currency,
        notes: a.notes,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn update(conn: &mut Connection, id: &str, patch: ManualAssetPatch) -> CoreResult<ManualAsset> {
    if let Some(v) = &patch.name {
        conn.execute(
            "UPDATE manual_assets SET name = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.asset_type {
        conn.execute(
            "UPDATE manual_assets SET asset_type = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = patch.value_cents {
        conn.execute(
            "UPDATE manual_assets SET value_cents = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.currency {
        conn.execute(
            "UPDATE manual_assets SET currency = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.notes {
        conn.execute(
            "UPDATE manual_assets SET notes = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    conn.execute(
        "UPDATE manual_assets SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    get_by_id(conn, id)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM manual_assets WHERE id = ?1", params![id])?;
    Ok(())
}

fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<ManualAsset> {
    conn.query_row(
        "SELECT id, name, asset_type, value_cents, currency, notes, created_at, updated_at \
         FROM manual_assets WHERE id = ?1",
        params![id],
        map_row,
    )
    .map_err(Into::into)
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<ManualAsset> {
    Ok(ManualAsset {
        id: r.get(0)?,
        name: r.get(1)?,
        asset_type: r.get(2)?,
        value_cents: r.get(3)?,
        currency: r.get(4)?,
        notes: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("ma.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn create_update_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let a = create(
            &mut conn,
            NewManualAsset {
                name: "House".into(),
                asset_type: "property".into(),
                value_cents: 50_000_000,
                currency: "USD".into(),
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 1);
        let updated = update(
            &mut conn,
            &a.id,
            ManualAssetPatch {
                value_cents: Some(52_000_000),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.value_cents, 52_000_000);
        delete(&mut conn, &a.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}

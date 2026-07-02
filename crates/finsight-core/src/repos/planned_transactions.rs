use crate::error::CoreResult;
use crate::models::planned_transaction::{
    NewPlannedTransaction, PlannedTransaction, PlannedTransactionPatch, PlannedTxnFilter,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list(
    conn: &mut Connection,
    filter: PlannedTxnFilter,
) -> CoreResult<Vec<PlannedTransaction>> {
    let mut sql = String::from(
        "SELECT id, description, amount_cents, account_id, category_id, due_date, status, source, created_at \
         FROM planned_transactions"
    );
    let mut conditions: Vec<String> = Vec::new();

    if filter.status.is_some() {
        conditions.push("status = ?1".to_string());
    }
    if filter.due_before.is_some() {
        conditions.push("due_date <= ?2".to_string());
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY due_date ASC");

    let mut stmt = conn.prepare(&sql)?;

    let rows = if let Some(ref status) = filter.status {
        if let Some(ref due_before) = filter.due_before {
            stmt.query_map(params![status, due_before], map_row)?
        } else {
            stmt.query_map(params![status], map_row)?
        }
    } else if let Some(ref due_before) = filter.due_before {
        stmt.query_map(params![due_before], map_row)?
    } else {
        stmt.query_map([], map_row)?
    };

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<PlannedTransaction> {
    Ok(PlannedTransaction {
        id: r.get(0)?,
        description: r.get(1)?,
        amount_cents: r.get(2)?,
        account_id: r.get(3)?,
        category_id: r.get(4)?,
        due_date: r.get(5)?,
        status: r.get(6)?,
        source: r.get(7)?,
        created_at: r.get(8)?,
    })
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<PlannedTransaction>> {
    let mut stmt = conn.prepare(
        "SELECT id, description, amount_cents, account_id, category_id, due_date, status, source, created_at \
         FROM planned_transactions WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], map_row)?;
    Ok(rows.next().transpose()?)
}

pub fn insert(conn: &mut Connection, new: NewPlannedTransaction) -> CoreResult<PlannedTransaction> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO planned_transactions (id, description, amount_cents, account_id, category_id, due_date, status, source, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'planned', ?7, ?8)",
        params![id, new.description, new.amount_cents, new.account_id, new.category_id, new.due_date, new.source, now],
    )?;
    Ok(PlannedTransaction {
        id,
        description: new.description,
        amount_cents: new.amount_cents,
        account_id: new.account_id,
        category_id: new.category_id,
        due_date: new.due_date,
        status: "planned".to_string(),
        source: new.source,
        created_at: now,
    })
}

pub fn update_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE planned_transactions SET status = ?1 WHERE id = ?2",
        params![status, id],
    )?;
    Ok(())
}

pub fn update(
    conn: &mut Connection,
    id: &str,
    patch: PlannedTransactionPatch,
) -> CoreResult<PlannedTransaction> {
    let current = get(conn, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let description = patch.description.unwrap_or(current.description);
    let amount_cents = patch.amount_cents.unwrap_or(current.amount_cents);
    let account_id = patch.account_id.unwrap_or(current.account_id);
    let category_id = patch.category_id.unwrap_or(current.category_id);
    let due_date = patch.due_date.unwrap_or(current.due_date);
    let status = patch.status.unwrap_or(current.status);
    let source = patch.source.unwrap_or(current.source);

    conn.execute(
        "UPDATE planned_transactions \
         SET description = ?1, amount_cents = ?2, account_id = ?3, category_id = ?4, due_date = ?5, status = ?6, source = ?7 \
         WHERE id = ?8",
        params![
            description,
            amount_cents,
            account_id,
            category_id,
            due_date,
            status,
            source,
            id
        ],
    )?;

    Ok(PlannedTransaction {
        id: current.id,
        description,
        amount_cents,
        account_id,
        category_id,
        due_date,
        status,
        source,
        created_at: current.created_at,
    })
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM planned_transactions WHERE id = ?1",
        params![id],
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
        let db = Db::open(&dir.path().join("planned.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_and_list_planned_transactions() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let planned = insert(
            &mut conn,
            NewPlannedTransaction {
                description: "Pay credit card".to_string(),
                amount_cents: 80000,
                account_id: None,
                category_id: None,
                due_date: "2026-06-25".to_string(),
                source: "agent".to_string(),
            },
        )
        .unwrap();

        assert_eq!(planned.status, "planned");
        assert_eq!(planned.amount_cents, 80000);

        let list = list(&mut conn, PlannedTxnFilter::default()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, planned.id);
    }

    #[test]
    fn update_status_and_delete() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let planned = insert(
            &mut conn,
            NewPlannedTransaction {
                description: "Invest".to_string(),
                amount_cents: 50000,
                account_id: None,
                category_id: None,
                due_date: "2026-06-20".to_string(),
                source: "agent".to_string(),
            },
        )
        .unwrap();

        update_status(&mut conn, &planned.id, "completed").unwrap();
        let fetched = get(&mut conn, &planned.id).unwrap().unwrap();
        assert_eq!(fetched.status, "completed");

        delete(&mut conn, &planned.id).unwrap();
        assert!(get(&mut conn, &planned.id).unwrap().is_none());
    }
}

use crate::error::CoreResult;
use crate::models::{Liability, LiabilityPatch, NewLiability};
use crate::repos::goals;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list(conn: &mut Connection) -> CoreResult<Vec<Liability>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, liability_type, balance_cents, limit_cents, apr_pct, min_payment_cents, payoff_date, original_balance_cents, started_at, currency, created_at, updated_at \
         FROM liabilities ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], map_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn create(conn: &mut Connection, l: NewLiability) -> CoreResult<Liability> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO liabilities(id, name, liability_type, balance_cents, limit_cents, apr_pct, min_payment_cents, payoff_date, original_balance_cents, started_at, currency, created_at, updated_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![id, l.name, l.liability_type, l.balance_cents, l.limit_cents, l.apr_pct, l.min_payment_cents, l.payoff_date, l.original_balance_cents, l.started_at, l.currency, now],
    )?;
    get_by_id(conn, &id)
}

pub fn update(conn: &mut Connection, id: &str, patch: LiabilityPatch) -> CoreResult<Liability> {
    if let Some(v) = &patch.name {
        conn.execute(
            "UPDATE liabilities SET name = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.liability_type {
        conn.execute(
            "UPDATE liabilities SET liability_type = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = patch.balance_cents {
        conn.execute(
            "UPDATE liabilities SET balance_cents = ?1 WHERE id = ?2",
            params![v, id],
        )?;
        goals::sync_linked_liabilities(conn, id)?;
    }
    if let Some(v) = &patch.limit_cents {
        conn.execute(
            "UPDATE liabilities SET limit_cents = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.apr_pct {
        conn.execute(
            "UPDATE liabilities SET apr_pct = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.min_payment_cents {
        conn.execute(
            "UPDATE liabilities SET min_payment_cents = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.payoff_date {
        conn.execute(
            "UPDATE liabilities SET payoff_date = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.original_balance_cents {
        conn.execute(
            "UPDATE liabilities SET original_balance_cents = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.started_at {
        conn.execute(
            "UPDATE liabilities SET started_at = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    if let Some(v) = &patch.currency {
        conn.execute(
            "UPDATE liabilities SET currency = ?1 WHERE id = ?2",
            params![v, id],
        )?;
    }
    conn.execute(
        "UPDATE liabilities SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    get_by_id(conn, id)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM liabilities WHERE id = ?1", params![id])?;
    Ok(())
}

fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Liability> {
    conn.query_row(
        "SELECT id, name, liability_type, balance_cents, limit_cents, apr_pct, min_payment_cents, payoff_date, original_balance_cents, started_at, currency, created_at, updated_at \
         FROM liabilities WHERE id = ?1",
        params![id], map_row,
    ).map_err(Into::into)
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<Liability> {
    Ok(Liability {
        id: r.get(0)?,
        name: r.get(1)?,
        liability_type: r.get(2)?,
        balance_cents: r.get(3)?,
        limit_cents: r.get(4)?,
        apr_pct: r.get(5)?,
        min_payment_cents: r.get(6)?,
        payoff_date: r.get(7)?,
        original_balance_cents: r.get(8)?,
        started_at: r.get(9)?,
        currency: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
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
        let db = Db::open(&dir.path().join("li.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn create_update_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let l = create(
            &mut conn,
            NewLiability {
                name: "Mortgage".into(),
                liability_type: "mortgage".into(),
                balance_cents: 30_000_000,
                limit_cents: Some(35_000_000),
                apr_pct: Some(5.5),
                min_payment_cents: Some(180_000),
                payoff_date: Some("2045-01-01".into()),
                original_balance_cents: Some(40_000_000),
                started_at: Some("2021-06".into()),
                currency: "USD".into(),
            },
        )
        .unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 1);
        let updated = update(
            &mut conn,
            &l.id,
            LiabilityPatch {
                balance_cents: Some(29_500_000),
                min_payment_cents: Some(Some(175_000)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.balance_cents, 29_500_000);
        assert_eq!(updated.apr_pct, Some(5.5));
        assert_eq!(updated.min_payment_cents, Some(175_000));
        assert_eq!(updated.original_balance_cents, Some(40_000_000));
        assert_eq!(updated.started_at, Some("2021-06".into()));
        delete(&mut conn, &l.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }

    #[test]
    fn updating_liability_balance_syncs_linked_goal() {
        use crate::repos::goals;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let l = create(
            &mut conn,
            NewLiability {
                name: "Car loan".into(),
                liability_type: "loan".into(),
                balance_cents: 15_000_000,
                limit_cents: None,
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                original_balance_cents: Some(20_000_000),
                started_at: None,
                currency: "USD".into(),
            },
        )
        .unwrap();
        let goal = goals::insert(
            &mut conn,
            goals::NewGoal {
                name: "Pay off car".into(),
                goal_type: "debt-payoff".into(),
                target_cents: 20_000_000,
                monthly_cents: 500_00,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                liability_id: Some(l.id.clone()),
                account_id: None,
            },
        )
        .unwrap();
        assert_eq!(goal.current_cents, 0);

        update(
            &mut conn,
            &l.id,
            LiabilityPatch {
                balance_cents: Some(12_000_000),
                ..Default::default()
            },
        )
        .unwrap();
        let synced = goals::get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(synced.current_cents, 12_000_000);
    }
}

use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Goal {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub current_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub liability_id: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewGoal {
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub liability_id: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GoalPatch {
    pub name: Option<String>,
    pub target_cents: Option<i64>,
    pub current_cents: Option<i64>,
    pub monthly_cents: Option<i64>,
    pub target_date: Option<Option<String>>,
    pub color: Option<String>,
    pub notes: Option<String>,
    pub purpose: Option<Option<String>>,
    pub liability_id: Option<Option<String>>,
    pub account_id: Option<Option<String>>,
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, target_cents, current_cents, monthly_cents, \
                target_date, color, notes, purpose, sort_order, created_at, \
                liability_id, account_id \
         FROM goals WHERE archived_at IS NULL ORDER BY sort_order, created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Goal {
            id: r.get(0)?,
            name: r.get(1)?,
            goal_type: r.get(2)?,
            target_cents: r.get(3)?,
            current_cents: r.get(4)?,
            monthly_cents: r.get(5)?,
            target_date: r.get(6)?,
            color: r.get(7)?,
            notes: r.get(8)?,
            purpose: r.get(9)?,
            sort_order: r.get(10)?,
            created_at: r.get(11)?,
            liability_id: r.get(12)?,
            account_id: r.get(13)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Goal> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, target_cents, current_cents, monthly_cents, \
                target_date, color, notes, purpose, sort_order, created_at, \
                liability_id, account_id \
         FROM goals WHERE id = ?1 AND archived_at IS NULL",
    )?;
    let mut rows = stmt.query_map(params![id], |r| {
        Ok(Goal {
            id: r.get(0)?,
            name: r.get(1)?,
            goal_type: r.get(2)?,
            target_cents: r.get(3)?,
            current_cents: r.get(4)?,
            monthly_cents: r.get(5)?,
            target_date: r.get(6)?,
            color: r.get(7)?,
            notes: r.get(8)?,
            purpose: r.get(9)?,
            sort_order: r.get(10)?,
            created_at: r.get(11)?,
            liability_id: r.get(12)?,
            account_id: r.get(13)?,
        })
    })?;
    rows.next()
        .transpose()?
        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
}

pub fn insert(conn: &mut Connection, g: NewGoal) -> CoreResult<Goal> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO goals(id, name, type, target_cents, current_cents, monthly_cents, \
                           target_date, color, notes, purpose, sort_order, created_at, \
                           liability_id, account_id)
         VALUES(?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12)",
        params![
            id,
            g.name,
            g.goal_type,
            g.target_cents,
            g.monthly_cents,
            g.target_date,
            g.color,
            g.notes,
            g.purpose,
            now,
            g.liability_id,
            g.account_id
        ],
    )?;
    Ok(Goal {
        id,
        name: g.name,
        goal_type: g.goal_type,
        target_cents: g.target_cents,
        current_cents: 0,
        monthly_cents: g.monthly_cents,
        target_date: g.target_date,
        color: g.color,
        notes: g.notes,
        purpose: g.purpose,
        sort_order: 0,
        created_at: now,
        liability_id: g.liability_id,
        account_id: g.account_id,
    })
}

/// Sync `current_cents` of every goal linked to the given liability with the
/// liability's current `balance_cents`.
pub fn sync_linked_liabilities(conn: &mut Connection, liability_id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals
         SET current_cents = COALESCE((SELECT balance_cents FROM liabilities WHERE id = ?1), 0)
         WHERE liability_id = ?1",
        params![liability_id],
    )?;
    Ok(())
}

pub fn set_current_cents(conn: &mut Connection, id: &str, current_cents: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET current_cents = ?1 WHERE id = ?2",
        params![current_cents, id],
    )?;
    Ok(())
}

pub fn archive(conn: &mut Connection, id: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE goals SET archived_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn set_monthly_cents(conn: &mut Connection, id: &str, monthly_cents: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET monthly_cents = ?1 WHERE id = ?2",
        params![monthly_cents, id],
    )?;
    Ok(())
}

pub fn set_purpose(conn: &mut Connection, id: &str, purpose: Option<&str>) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET purpose = ?1 WHERE id = ?2",
        params![purpose, id],
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
        let db = Db::open(&dir.path().join("g.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn set_monthly_cents_updates_correctly() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let goal = insert(
            &mut conn,
            NewGoal {
                name: "Italy trip".into(),
                goal_type: "save-by-date".into(),
                target_cents: 500_000,
                monthly_cents: 10_000,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                liability_id: None,
                account_id: None,
            },
        )
        .unwrap();
        set_monthly_cents(&mut conn, &goal.id, 25_000).unwrap();
        let updated = list(&mut conn)
            .unwrap()
            .into_iter()
            .find(|g| g.id == goal.id)
            .unwrap();
        assert_eq!(updated.monthly_cents, 25_000);
    }

    #[test]
    fn insert_goal_with_links_round_trip() {
        use crate::models::NewLiability;
        use crate::repos::liabilities;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let l = liabilities::create(
            &mut conn,
            NewLiability {
                name: "Loan".into(),
                liability_type: "loan".into(),
                balance_cents: 5_000_00,
                limit_cents: None,
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                original_balance_cents: None,
                started_at: None,
                currency: "USD".into(),
            },
        )
        .unwrap();
        let goal = insert(
            &mut conn,
            NewGoal {
                name: "Payoff".into(),
                goal_type: "debt-payoff".into(),
                target_cents: 5_000_00,
                monthly_cents: 100_00,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                liability_id: Some(l.id.clone()),
                account_id: None,
            },
        )
        .unwrap();
        assert_eq!(goal.liability_id, Some(l.id.clone()));
        let fetched = get_by_id(&mut conn, &goal.id).unwrap();
        assert_eq!(fetched.liability_id, Some(l.id));
    }

    #[test]
    fn deleting_liability_clears_goal_link() {
        use crate::models::NewLiability;
        use crate::repos::liabilities;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let l = liabilities::create(
            &mut conn,
            NewLiability {
                name: "Loan".into(),
                liability_type: "loan".into(),
                balance_cents: 5_000_00,
                limit_cents: None,
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                original_balance_cents: None,
                started_at: None,
                currency: "USD".into(),
            },
        )
        .unwrap();
        let lid = l.id.clone();
        let goal = insert(
            &mut conn,
            NewGoal {
                name: "Payoff".into(),
                goal_type: "debt-payoff".into(),
                target_cents: 5_000_00,
                monthly_cents: 100_00,
                target_date: None,
                color: "#C9F950".into(),
                notes: None,
                purpose: None,
                liability_id: Some(lid),
                account_id: None,
            },
        )
        .unwrap();
        liabilities::delete(&mut conn, &l.id).unwrap();
        let fetched = get_by_id(&mut conn, &goal.id).unwrap();
        assert!(fetched.liability_id.is_none());
    }
}

use crate::error::CoreResult;
use crate::models::RuleProposal;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<RuleProposal> {
    Ok(RuleProposal {
        id: r.get(0)?, when_label: r.get(1)?, description: r.get(2)?, pattern: r.get(3)?,
        category_id: r.get(4)?, status: r.get(5)?, created_at: r.get(6)?,
    })
}

pub fn list(conn: &mut Connection, status: Option<&str>) -> CoreResult<Vec<RuleProposal>> {
    let mut out = Vec::new();
    match status {
        Some(s) => {
            let mut stmt = conn.prepare(
                "SELECT id, when_label, description, pattern, category_id, status, created_at \
                 FROM rule_proposals WHERE status = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![s], map_row)?;
            for row in rows { out.push(row?); }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, when_label, description, pattern, category_id, status, created_at \
                 FROM rule_proposals ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], map_row)?;
            for row in rows { out.push(row?); }
        }
    }
    Ok(out)
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<RuleProposal>> {
    match conn.query_row(
        "SELECT id, when_label, description, pattern, category_id, status, created_at \
         FROM rule_proposals WHERE id = ?1",
        params![id], map_row,
    ) {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn insert(conn: &mut Connection, when_label: &str, description: &str, pattern: &str, category_id: &str) -> CoreResult<RuleProposal> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO rule_proposals(id, when_label, description, pattern, category_id, status, created_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
        params![id, when_label, description, pattern, category_id, now],
    )?;
    Ok(RuleProposal {
        id, when_label: when_label.to_string(), description: description.to_string(),
        pattern: pattern.to_string(), category_id: category_id.to_string(),
        status: "pending".to_string(), created_at: now,
    })
}

pub fn set_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    conn.execute("UPDATE rule_proposals SET status = ?1 WHERE id = ?2", params![status, id])?;
    Ok(())
}

pub fn exists_pending(conn: &mut Connection, pattern: &str, category_id: &str) -> CoreResult<bool> {
    let found: bool = conn.query_row(
        "SELECT 1 FROM rule_proposals \
         WHERE lower(pattern) = lower(?1) AND category_id = ?2 AND status = 'pending' LIMIT 1",
        params![pattern, category_id],
        |_| Ok(true),
    ).or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(false),
        other => Err(other),
    })?;
    Ok(found)
}

/// Find merchants the user has manually set to the same category at least
/// `threshold` distinct times, and emit a pending proposal for each — unless an
/// enabled rule or a pending proposal already covers it. Returns count inserted.
pub fn emit_from_corrections(conn: &mut Connection, threshold: i64) -> CoreResult<usize> {
    let mut stmt = conn.prepare(
        "SELECT t.merchant_raw, ca.category_id, c.label, COUNT(DISTINCT ca.txn_id) AS n \
         FROM categorizations ca \
         JOIN transactions t ON t.id = ca.txn_id \
         JOIN categories c ON c.id = ca.category_id \
         WHERE ca.source = 'user' \
         GROUP BY lower(t.merchant_raw), ca.category_id \
         HAVING COUNT(DISTINCT ca.txn_id) >= ?1",
    )?;
    let candidates: Vec<(String, String, String, i64)> = stmt
        .query_map(params![threshold], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let mut inserted = 0usize;
    for (merchant_raw, category_id, category_label, n) in candidates {
        let rule_exists: bool = conn.query_row(
            "SELECT 1 FROM rules WHERE lower(pattern) = lower(?1) AND enabled = 1 LIMIT 1",
            params![merchant_raw],
            |_| Ok(true),
        ).or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })?;
        if rule_exists || exists_pending(conn, &merchant_raw, &category_id)? {
            continue;
        }
        let description = format!(
            "You've set \"{}\" to {} {} times — make it a rule?",
            merchant_raw, category_label, n
        );
        insert(conn, "Recurring", &description, &merchant_raw, &category_id)?;
        inserted += 1;
    }
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("rp.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_three_user_corrections(conn: &mut Connection) {
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Streaming','#0f0',0)", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
        for i in 0..3 {
            let tid = format!("t{i}");
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES(?1,'a1','2024-01-01T00:00:00Z',-1500,'NETFLIX','cleared',0,'2024-01-01T00:00:00Z')",
                params![tid],
            ).unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES(?1,?2,'cat1','user',1.0,'2024-01-02T00:00:00Z')",
                params![format!("c{i}"), tid],
            ).unwrap();
        }
    }

    #[test]
    fn emit_creates_one_pending_then_dedupes() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_three_user_corrections(&mut conn);
        let n = emit_from_corrections(&mut conn, 3).unwrap();
        assert_eq!(n, 1);
        let pending = list(&mut conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].pattern, "NETFLIX");
        assert_eq!(pending[0].category_id, "cat1");
        // Re-running must not create a duplicate.
        assert_eq!(emit_from_corrections(&mut conn, 3).unwrap(), 0);
    }

    #[test]
    fn set_status_excludes_from_pending() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let p = insert(&mut conn, "Recurring", "desc", "SPOTIFY", "cat1").unwrap();
        assert_eq!(list(&mut conn, Some("pending")).unwrap().len(), 1);
        set_status(&mut conn, &p.id, "declined").unwrap();
        assert_eq!(list(&mut conn, Some("pending")).unwrap().len(), 0);
        assert!(get(&mut conn, &p.id).unwrap().is_some());
    }
}

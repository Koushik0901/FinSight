use crate::error::CoreResult;
use crate::models::{NewRule, Rule};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list_active(conn: &mut Connection) -> CoreResult<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, category_id, enabled, source, created_at \
         FROM rules WHERE enabled = 1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let created_s: String = r.get(5)?;
        Ok(Rule {
            id: r.get(0)?,
            pattern: r.get(1)?,
            category_id: r.get(2)?,
            enabled: r.get::<_, i64>(3)? != 0,
            source: r.get(4)?,
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&Utc),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert(conn: &mut Connection, rule: NewRule) -> CoreResult<Rule> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO rules(id, pattern, category_id, enabled, source, created_at) \
         VALUES(?1, ?2, ?3, 1, ?4, ?5)",
        params![
            id,
            rule.pattern,
            rule.category_id,
            rule.source,
            now.to_rfc3339()
        ],
    )?;
    Ok(Rule {
        id,
        pattern: rule.pattern,
        category_id: rule.category_id,
        enabled: true,
        source: rule.source,
        created_at: now,
    })
}

/// Retroactively apply a rule pattern to existing UNCATEGORIZED, non-transfer
/// expense transactions, so a rule created from a recurring payment (e.g. an
/// e-transfer to a landlord → Housing) categorizes the history immediately
/// instead of only future imports. Returns the number of rows categorized.
/// Uses the same `%…%`=contains / bare=exact LIKE semantics as the categorizer.
pub fn apply_to_uncategorized(
    conn: &mut Connection,
    pattern: &str,
    category_id: &str,
) -> CoreResult<usize> {
    // Only categorize real uncategorized spending — never transfers (invariant),
    // never income, never already-categorized rows.
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM transactions \
             WHERE category_id IS NULL AND is_transfer = 0 AND amount_cents < 0 \
               AND lower(merchant_raw) LIKE lower(?1)",
        )?;
        let rows = stmt.query_map(params![pattern], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if ids.is_empty() {
        return Ok(0);
    }
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    {
        let mut set_cat = tx.prepare_cached(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
        )?;
        let mut record = tx.prepare_cached(
            "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) \
             VALUES(?1, ?2, ?3, 'rule', 1.0, NULL, ?4)",
        )?;
        for id in &ids {
            set_cat.execute(params![category_id, id])?;
            record.execute(params![Uuid::new_v4().to_string(), id, category_id, now])?;
        }
    }
    tx.commit()?;
    Ok(ids.len())
}

pub fn set_enabled(conn: &mut Connection, id: &str, enabled: bool) -> CoreResult<()> {
    conn.execute(
        "UPDATE rules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, id],
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
        let db = Db::open(&dir.path().join("r.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn apply_to_uncategorized_backfills_history_but_not_transfers_or_income() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('housing','g1','Housing','#f00',0)", []).unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('chk','You','B','Checking','C','CAD','#111','manual',datetime('now'))",
            [],
        ).unwrap();
        // Two rent e-transfers (uncategorized expense), a same-recipient transfer
        // leg, and an income row — only the two expenses should be categorized.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) VALUES\
             ('r1','chk','2026-05-02T12:00:00Z',-120000,'INTERAC e-Transfer To: LANDLORD PROPERTIES',0,'cleared',datetime('now')),\
             ('r2','chk','2026-06-02T12:00:00Z',-120000,'INTERAC e-Transfer To: LANDLORD PROPERTIES',0,'cleared',datetime('now')),\
             ('tf','chk','2026-06-02T12:00:00Z', 120000,'INTERAC e-Transfer From LANDLORD refund',1,'cleared',datetime('now')),\
             ('in','chk','2026-06-03T12:00:00Z', 500000,'LANDLORD PROPERTIES DEPOSIT',0,'cleared',datetime('now'))",
            [],
        ).unwrap();

        let n = apply_to_uncategorized(&mut conn, "%landlord properties%", "housing").unwrap();
        assert_eq!(n, 2, "both rent expenses categorized");
        let housing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id='housing'", [], |r| r.get(0)).unwrap();
        assert_eq!(housing, 2);
        // Transfer leg and income row untouched.
        let tf_cat: Option<String> = conn.query_row("SELECT category_id FROM transactions WHERE id='tf'", [], |r| r.get(0)).unwrap();
        let in_cat: Option<String> = conn.query_row("SELECT category_id FROM transactions WHERE id='in'", [], |r| r.get(0)).unwrap();
        assert_eq!(tf_cat, None, "transfer leg never categorized");
        assert_eq!(in_cat, None, "income row (positive amount) not categorized");
    }

    #[test]
    fn insert_and_list_active_rules() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();

        let rule = NewRule {
            pattern: "%amazon%".to_string(),
            category_id: "cat1".to_string(),
            source: "user".to_string(),
        };
        let r = insert(&mut conn, rule).unwrap();
        assert_eq!(r.pattern, "%amazon%");

        let active = list_active(&mut conn).unwrap();
        assert_eq!(active.len(), 1);

        set_enabled(&mut conn, &r.id, false).unwrap();
        let active2 = list_active(&mut conn).unwrap();
        assert_eq!(active2.len(), 0);
    }
}

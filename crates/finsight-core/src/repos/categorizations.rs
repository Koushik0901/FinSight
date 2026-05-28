use crate::error::CoreResult;
use crate::models::{Categorization, NewCategorization};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn insert(conn: &mut Connection, row: NewCategorization) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Uuid::new_v4().to_string(),
            row.txn_id,
            row.category_id,
            row.source,
            row.confidence,
            row.model,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn list_for_txn(conn: &mut Connection, txn_id: &str) -> CoreResult<Vec<Categorization>> {
    let mut stmt = conn.prepare(
        "SELECT id, txn_id, category_id, source, confidence, model, at \
         FROM categorizations WHERE txn_id = ?1 ORDER BY at DESC",
    )?;
    let rows = stmt.query_map(params![txn_id], |r| {
        let at_s: String = r.get(6)?;
        Ok(Categorization {
            id: r.get(0)?,
            txn_id: r.get(1)?,
            category_id: r.get(2)?,
            source: r.get(3)?,
            confidence: r.get(4)?,
            model: r.get(5)?,
            at: DateTime::parse_from_rfc3339(&at_s)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&Utc),
        })
    })?;
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
        let db = Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_and_list_categorization() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // Insert a category + account + transaction first
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Checking','Ch','USD','#000','manual','2024-01-01T00:00:00Z')", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t1','a1','2024-01-01T00:00:00Z',1000,'AMAZON','cleared',0,'2024-01-01T00:00:00Z')", [],
        ).unwrap();

        let row = NewCategorization {
            txn_id: "t1".to_string(),
            category_id: Some("cat1".to_string()),
            source: "user".to_string(),
            confidence: 1.0,
            model: None,
        };
        insert(&mut conn, row).unwrap();
        let rows = list_for_txn(&mut conn, "t1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, "user");
        assert_eq!(rows[0].category_id.as_deref(), Some("cat1"));
    }
}

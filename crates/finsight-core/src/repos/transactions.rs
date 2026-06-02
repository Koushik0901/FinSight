use crate::error::CoreResult;
use crate::models::{NewTransaction, ProposedRule, Transaction, TransactionStatus, TxnPatch};
use crate::repos::categorizations;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn insert(conn: &mut Connection, input: NewTransaction) -> CoreResult<Transaction> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO transactions \
         (id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, notes, is_anomaly, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        params![
            &id,
            &input.account_id,
            input.posted_at.to_rfc3339(),
            input.amount_cents,
            &input.merchant_raw,
            &input.category_id,
            input.status.as_db(),
            &input.notes,
            now.to_rfc3339(),
        ],
    )?;
    Ok(Transaction {
        id,
        account_id: input.account_id,
        posted_at: input.posted_at,
        amount_cents: input.amount_cents,
        merchant_raw: input.merchant_raw,
        merchant_id: None,
        merchant_label: None,
        merchant_color: None,
        merchant_initials: None,
        category_id: input.category_id,
        category_label: None,
        category_color: None,
        status: input.status,
        notes: input.notes,
        ai_confidence: None,
        ai_explanation: None,
        is_anomaly: false,
        created_at: now,
    })
}

pub struct TxnFilter {
    pub account_id: Option<String>,
    pub limit: i64,
    pub offset: i64,
    pub search: Option<String>,
    pub filter_preset: Option<String>,
}

impl Default for TxnFilter {
    fn default() -> Self {
        Self {
            account_id: None,
            limit: 100,
            offset: 0,
            search: None,
            filter_preset: None,
        }
    }
}

pub fn list(conn: &mut Connection, filter: TxnFilter) -> CoreResult<Vec<Transaction>> {
    let mut sql = String::from(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id ",
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();

    if let Some(aid) = filter.account_id.as_ref() {
        conditions.push("t.account_id = ?".to_string());
        params.push(Box::new(aid.clone()));
    }
    if let Some(search) = filter.search.as_ref() {
        conditions.push(
            "(lower(t.merchant_raw) LIKE lower(?) OR lower(COALESCE(t.notes,'')) LIKE lower(?))".to_string(),
        );
        let pattern = format!("%{}%", search);
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }
    match filter.filter_preset.as_deref() {
        Some("needs_review") => {
            conditions.push("t.ai_confidence IS NOT NULL AND t.ai_confidence < 0.6".to_string());
        }
        Some("anomalies") => {
            conditions.push("t.is_anomaly = 1".to_string());
        }
        Some("no_category") => {
            conditions.push("t.category_id IS NULL".to_string());
        }
        _ => {}
    }
    if !conditions.is_empty() {
        sql.push_str("WHERE ");
        sql.push_str(&conditions.join(" AND "));
        sql.push(' ');
    }
    sql.push_str("ORDER BY t.posted_at DESC LIMIT ? OFFSET ?");
    params.push(Box::new(filter.limit));
    params.push(Box::new(filter.offset));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())),
        |r| {
            let posted_at_s: String = r.get(2)?;
            let created_at_s: String = r.get(17)?;
            Ok(Transaction {
                id: r.get(0)?,
                account_id: r.get(1)?,
                posted_at: DateTime::parse_from_rfc3339(&posted_at_s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
                amount_cents: r.get(3)?,
                merchant_raw: r.get(4)?,
                merchant_id: r.get(5)?,
                merchant_label: r.get(6)?,
                merchant_color: r.get(7)?,
                merchant_initials: r.get(8)?,
                category_id: r.get(9)?,
                category_label: r.get(10)?,
                category_color: r.get(11)?,
                status: TransactionStatus::from_db(&r.get::<_, String>(12)?),
                notes: r.get(13)?,
                ai_confidence: r.get(14)?,
                ai_explanation: r.get(15)?,
                is_anomaly: r.get::<_, i64>(16)? != 0,
                created_at: DateTime::parse_from_rfc3339(&created_at_s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            17,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update(
    conn: &mut Connection,
    id: &str,
    patch: TxnPatch,
) -> CoreResult<(Transaction, Option<ProposedRule>)> {
    if let Some(notes) = &patch.notes {
        conn.execute("UPDATE transactions SET notes = ?1 WHERE id = ?2", params![notes, id])?;
    }
    if let Some(amount) = patch.amount_cents {
        conn.execute("UPDATE transactions SET amount_cents = ?1 WHERE id = ?2", params![amount, id])?;
    }
    if let Some(merchant) = &patch.merchant_raw {
        conn.execute("UPDATE transactions SET merchant_raw = ?1 WHERE id = ?2", params![merchant, id])?;
    }

    let mut proposed_rule: Option<ProposedRule> = None;

    if let Some(cat) = &patch.category_id {
        // Append categorization audit row
        categorizations::insert(conn, crate::models::NewCategorization {
            txn_id: id.to_string(),
            category_id: cat.clone(),
            source: "user".to_string(),
            confidence: 1.0,
            model: None,
        })?;
        // Update live columns
        conn.execute(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
            params![cat, id],
        )?;
        // Check for rule proposal (only when setting a category, not clearing)
        if let Some(category_id) = cat {
            let merchant_raw: String = conn.query_row(
                "SELECT merchant_raw FROM transactions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            let rule_exists: bool = conn.query_row(
                "SELECT 1 FROM rules WHERE lower(pattern) = lower(?1) AND enabled = 1 LIMIT 1",
                params![merchant_raw],
                |_| Ok(true),
            ).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                other => Err(other),
            })?;
            if !rule_exists {
                let category_label: String = conn.query_row(
                    "SELECT label FROM categories WHERE id = ?1",
                    params![category_id],
                    |r| r.get(0),
                ).unwrap_or_default();
                proposed_rule = Some(ProposedRule {
                    pattern: merchant_raw,
                    category_id: category_id.clone(),
                    category_label,
                });
            }
        }
    }

    // Fetch and return updated transaction
    let txn = get_by_id(conn, id)?;
    Ok((txn, proposed_rule))
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
    Ok(())
}

/// Fetch a single transaction by id (used internally).
fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Transaction> {
    conn.query_row(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.id = ?1",
        params![id],
        |r| {
            let posted_s: String = r.get(2)?;
            let created_s: String = r.get(17)?;
            Ok(Transaction {
                id: r.get(0)?,
                account_id: r.get(1)?,
                posted_at: DateTime::parse_from_rfc3339(&posted_s)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?
                    .with_timezone(&Utc),
                amount_cents: r.get(3)?,
                merchant_raw: r.get(4)?,
                merchant_id: r.get(5)?,
                merchant_label: r.get(6)?,
                merchant_color: r.get(7)?,
                merchant_initials: r.get(8)?,
                category_id: r.get(9)?,
                category_label: r.get(10)?,
                category_color: r.get(11)?,
                status: TransactionStatus::from_db(&r.get::<_, String>(12)?),
                notes: r.get(13)?,
                ai_confidence: r.get(14)?,
                ai_explanation: r.get(15)?,
                is_anomaly: r.get::<_, i64>(16)? != 0,
                created_at: DateTime::parse_from_rfc3339(&created_s)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(17, rusqlite::types::Type::Text, Box::new(e)))?
                    .with_timezone(&Utc),
            })
        },
    ).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::run_migrations, keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::accounts,
        Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut rusqlite::Connection) -> (String, String) {
        // category
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        // account
        let acc = accounts::insert(conn, NewAccount {
            owner: "Me".into(), bank: "Bank".into(),
            r#type: AccountType::Checking, name: "Ch".into(),
            last4: None, currency: "USD".into(), color: "#fff".into(),
            opening_balance_cents: 0, source: "manual".into(),
        }).unwrap();
        // transaction
        let txn = insert(conn, NewTransaction {
            account_id: acc.id.clone(),
            posted_at: chrono::Utc::now(),
            amount_cents: 1000,
            merchant_raw: "AMAZON".to_string(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
        }).unwrap();
        (acc.id, txn.id)
    }

    #[test]
    fn update_transaction_notes() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch { notes: Some(Some("edited".into())), ..Default::default() };
        let (updated, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert_eq!(updated.notes.as_deref(), Some("edited"));
        assert!(rule.is_none()); // no category change → no rule proposal
    }

    #[test]
    fn update_category_appends_categorization_and_proposes_rule() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch {
            category_id: Some(Some("cat1".into())),
            ..Default::default()
        };
        let (updated, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert_eq!(updated.category_id.as_deref(), Some("cat1"));
        // Rule proposed because no existing rule for "AMAZON"
        assert!(rule.is_some());
        let r = rule.unwrap();
        assert_eq!(r.pattern, "AMAZON");
        assert_eq!(r.category_id, "cat1");
    }

    #[test]
    fn update_category_no_rule_when_rule_exists() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        // Pre-create a matching rule
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at) \
             VALUES('r1','AMAZON','cat1',1,'user','2024-01-01T00:00:00Z')", [],
        ).unwrap();
        let patch = TxnPatch { category_id: Some(Some("cat1".into())), ..Default::default() };
        let (_, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert!(rule.is_none()); // rule already exists → no proposal
    }

    #[test]
    fn delete_transaction_removes_row() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        delete(&mut conn, &txn_id).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE id = ?1",
            rusqlite::params![txn_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }
}

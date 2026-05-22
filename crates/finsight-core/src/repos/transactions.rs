use crate::error::CoreResult;
use crate::models::{NewTransaction, Transaction, TransactionStatus};
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
}

impl Default for TxnFilter {
    fn default() -> Self {
        Self {
            account_id: None,
            limit: 100,
            offset: 0,
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
    if let Some(aid) = filter.account_id.as_ref() {
        sql.push_str("WHERE t.account_id = ? ");
        params.push(Box::new(aid.clone()));
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

//! CRUD for detected transaction transfers.

use crate::error::CoreResult;
use crate::models::TransactionTransfer;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

pub fn insert(
    conn: &mut Connection,
    input: TransactionTransfer,
) -> CoreResult<TransactionTransfer> {
    conn.execute(
        "INSERT INTO transaction_transfers (id, from_transaction_id, to_transaction_id, confidence, detected_at, user_confirmed) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &input.id,
            &input.from_transaction_id,
            &input.to_transaction_id,
            &input.confidence,
            input.detected_at.to_rfc3339(),
            input.user_confirmed,
        ],
    )?;
    Ok(input)
}

pub fn find_candidates(
    conn: &mut Connection,
    account_id: &str,
    since: DateTime<Utc>,
) -> CoreResult<Vec<(String, i64, DateTime<Utc>)>> {
    let mut stmt = conn.prepare(
        "SELECT id, amount_cents, posted_at FROM transactions \
         WHERE account_id = ?1 AND posted_at >= ?2 AND pending = 0 \
         ORDER BY posted_at DESC",
    )?;
    let rows = stmt.query_map(params![account_id, since.to_rfc3339()], |r| {
        let posted_s: String = r.get(2)?;
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            DateTime::parse_from_rfc3339(&posted_s)
                .unwrap()
                .with_timezone(&Utc),
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// A pending transfer suggestion with the two transactions and accounts shown.
#[derive(Debug, Clone)]
pub struct TransferSuggestion {
    pub id: String,
    pub confidence: String,
    pub detected_at: DateTime<Utc>,
    pub from_transaction_id: String,
    pub from_account_name: String,
    pub from_merchant: String,
    pub from_amount_cents: i64,
    pub from_posted_at: DateTime<Utc>,
    pub to_transaction_id: String,
    pub to_account_name: String,
    pub to_merchant: String,
    pub to_amount_cents: i64,
    pub to_posted_at: DateTime<Utc>,
}

pub fn list_suggestions(conn: &mut Connection) -> CoreResult<Vec<TransferSuggestion>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.confidence, t.detected_at, \
         t.from_transaction_id, fa.name, fx.merchant_raw, fx.amount_cents, fx.posted_at, \
         t.to_transaction_id, ta.name, tx.merchant_raw, tx.amount_cents, tx.posted_at \
         FROM transaction_transfers t \
         JOIN transactions fx ON fx.id = t.from_transaction_id \
         JOIN transactions tx ON tx.id = t.to_transaction_id \
         JOIN accounts fa ON fa.id = fx.account_id \
         JOIN accounts ta ON ta.id = tx.account_id \
         WHERE t.user_confirmed = 0 \
         ORDER BY t.detected_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let from_posted_s: String = r.get(7)?;
        let to_posted_s: String = r.get(12)?;
        let detected_s: String = r.get(2)?;
        Ok(TransferSuggestion {
            id: r.get(0)?,
            confidence: r.get(1)?,
            detected_at: DateTime::parse_from_rfc3339(&detected_s)
                .unwrap()
                .with_timezone(&Utc),
            from_transaction_id: r.get(3)?,
            from_account_name: r.get(4)?,
            from_merchant: r.get(5)?,
            from_amount_cents: r.get(6)?,
            from_posted_at: DateTime::parse_from_rfc3339(&from_posted_s)
                .unwrap()
                .with_timezone(&Utc),
            to_transaction_id: r.get(8)?,
            to_account_name: r.get(9)?,
            to_merchant: r.get(10)?,
            to_amount_cents: r.get(11)?,
            to_posted_at: DateTime::parse_from_rfc3339(&to_posted_s)
                .unwrap()
                .with_timezone(&Utc),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

pub fn confirm(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE transaction_transfers SET user_confirmed = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn reject(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM transaction_transfers WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::{accounts, transactions},
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

    fn seed_account(conn: &mut rusqlite::Connection) -> String {
        accounts::insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Ch".into(),
                last4: None,
                currency: "USD".into(),
                color: "#fff".into(),
                opening_balance_cents: 0,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
            },
        )
        .unwrap()
        .id
    }

    fn seed_txn(conn: &mut rusqlite::Connection, account_id: &str, amount_cents: i64) -> String {
        transactions::insert(
            conn,
            NewTransaction {
                account_id: account_id.into(),
                posted_at: Utc::now(),
                amount_cents,
                merchant_raw: "TEST".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn insert_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        let from_id = seed_txn(&mut conn, &acc, -5000);
        let to_id = seed_txn(&mut conn, &acc, 5000);

        let transfer = TransactionTransfer {
            id: "t1".into(),
            from_transaction_id: from_id,
            to_transaction_id: to_id,
            confidence: "high".into(),
            detected_at: Utc::now(),
            user_confirmed: false,
        };
        let inserted = insert(&mut conn, transfer.clone()).unwrap();
        assert_eq!(inserted.id, transfer.id);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transaction_transfers WHERE id = ?1",
                params![transfer.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn find_candidates_excludes_pending_and_old() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        let now = Utc::now();
        let _old = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acc.clone(),
                posted_at: now - chrono::Duration::hours(2),
                amount_cents: -1000,
                merchant_raw: "OLD".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
            },
        )
        .unwrap();
        let recent = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acc.clone(),
                posted_at: now,
                amount_cents: -2500,
                merchant_raw: "RECENT".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: true,
                external_tx_id: None,
                external_account_id: None,
            },
        )
        .unwrap();
        let cleared = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acc.clone(),
                posted_at: now,
                amount_cents: 2500,
                merchant_raw: "CLEARED".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
            },
        )
        .unwrap();

        let candidates =
            find_candidates(&mut conn, &acc, now - chrono::Duration::hours(1)).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, cleared.id);
        assert_eq!(candidates[0].1, 2500);

        let all = find_candidates(&mut conn, &acc, now - chrono::Duration::days(365)).unwrap();
        assert!(all.iter().any(|(id, _, _)| id == &_old.id));
        assert!(!all.iter().any(|(id, _, _)| id == &recent.id));
    }
}

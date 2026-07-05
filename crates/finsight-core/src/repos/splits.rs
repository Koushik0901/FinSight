use crate::error::{CoreError, CoreResult};
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct TransactionSplit {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Clone)]
pub struct SplitInput {
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

pub fn list(conn: &mut Connection, txn_id: &str) -> CoreResult<Vec<TransactionSplit>> {
    let mut stmt = conn.prepare(
        "SELECT id, txn_id, category_id, amount_cents FROM transaction_splits WHERE txn_id = ?1 ORDER BY rowid"
    )?;
    let rows = stmt.query_map(params![txn_id], |r| {
        Ok(TransactionSplit {
            id: r.get(0)?,
            txn_id: r.get(1)?,
            category_id: r.get(2)?,
            amount_cents: r.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Database)
}

/// Replace all splits for a transaction atomically.
pub fn set(conn: &mut Connection, txn_id: &str, splits: &[SplitInput]) -> CoreResult<()> {
    let tx = conn.transaction()?;
    if !splits.is_empty() {
        if splits.len() < 2 {
            return Err(CoreError::InvalidState("at least 2 splits required".into()));
        }
        let parent_abs: i64 = tx
            .query_row(
                "SELECT ABS(amount_cents) FROM transactions WHERE id = ?1",
                params![txn_id],
                |r| r.get(0),
            )
            .map_err(CoreError::Database)?;
        let total: i64 = splits.iter().map(|s| s.amount_cents).sum();
        if total != parent_abs {
            return Err(CoreError::InvalidState(format!(
                "splits sum {total} != transaction abs amount {parent_abs}"
            )));
        }
    }
    tx.execute(
        "DELETE FROM transaction_splits WHERE txn_id = ?1",
        params![txn_id],
    )?;
    for s in splits {
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO transaction_splits (id, txn_id, category_id, amount_cents) VALUES (?1, ?2, ?3, ?4)",
            params![id, txn_id, s.category_id, s.amount_cents],
        )?;
    }
    if splits.is_empty() {
        tx.execute(
            "UPDATE transactions SET is_split = 0 WHERE id = ?1",
            params![txn_id],
        )?;
    } else {
        tx.execute(
            "UPDATE transactions SET is_split = 1, category_id = NULL WHERE id = ?1",
            params![txn_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::accounts,
        repos::transactions,
        Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("splits.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn insert_test_txn(conn: &mut Connection, amount_cents: i64) -> String {
        let acc = accounts::insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Chk".into(),
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
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                limit_cents: None,
                original_balance_cents: None,
                started_at: None,
            },
        )
        .unwrap();
        let txn = transactions::insert(
            conn,
            NewTransaction {
                account_id: acc.id.clone(),
                posted_at: chrono::Utc::now(),
                amount_cents,
                merchant_raw: "Costco".into(),
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
        txn.id
    }

    #[test]
    fn set_and_list_splits() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let txn_id = insert_test_txn(&mut conn, -10000);

        set(
            &mut conn,
            &txn_id,
            &[
                SplitInput {
                    category_id: None,
                    amount_cents: 6000,
                },
                SplitInput {
                    category_id: None,
                    amount_cents: 4000,
                },
            ],
        )
        .unwrap();

        let splits = list(&mut conn, &txn_id).unwrap();
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].amount_cents, 6000);
        assert_eq!(splits[1].amount_cents, 4000);

        let (is_split, cat_id): (i64, Option<String>) = conn
            .query_row(
                "SELECT is_split, category_id FROM transactions WHERE id = ?1",
                params![txn_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_split, 1);
        assert!(cat_id.is_none());
    }

    #[test]
    fn clear_splits_resets_flag() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let txn_id = insert_test_txn(&mut conn, -5000);
        set(
            &mut conn,
            &txn_id,
            &[
                SplitInput {
                    category_id: None,
                    amount_cents: 3000,
                },
                SplitInput {
                    category_id: None,
                    amount_cents: 2000,
                },
            ],
        )
        .unwrap();
        set(&mut conn, &txn_id, &[]).unwrap();

        let splits = list(&mut conn, &txn_id).unwrap();
        assert!(splits.is_empty());
        let is_split: i64 = conn
            .query_row(
                "SELECT is_split FROM transactions WHERE id = ?1",
                params![txn_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_split, 0);
    }

    #[test]
    fn rejects_sum_mismatch() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let txn_id = insert_test_txn(&mut conn, -10000);
        let err = set(
            &mut conn,
            &txn_id,
            &[
                SplitInput {
                    category_id: None,
                    amount_cents: 3000,
                },
                SplitInput {
                    category_id: None,
                    amount_cents: 3000,
                },
            ],
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::InvalidState(_)));
    }
}

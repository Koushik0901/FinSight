//! Detect likely transfer pairs between SimpleFin-linked accounts.
//!
//! After syncing, scan the last 30 days for transaction pairs that look like
//! internal transfers: same absolute amount, opposite sign, close post dates,
//! and matching keywords or unique amount. Suggestions are stored in
//! `transaction_transfers` with a confidence score.

use chrono::{DateTime, Duration, Utc};
use finsight_core::error::CoreResult;
use finsight_core::models::TransactionTransfer;
use finsight_core::repos::transfers;
use rusqlite::Connection;
use uuid::Uuid;

const TRANSFER_KEYWORDS: &[&str] = &[
    "transfer",
    "zelle",
    "venmo",
    "wire",
    "ach",
    "move money",
    "account transfer",
];
const LOOKBACK_DAYS: i64 = 30;
const HIGH_CONFIDENCE_DAYS: i64 = 1;
const MEDIUM_CONFIDENCE_DAYS: i64 = 3;
const LOW_CONFIDENCE_DAYS: i64 = 7;

pub fn detect_transfers(
    conn: &mut Connection,
    account_ids: &[String],
) -> CoreResult<Vec<TransactionTransfer>> {
    let since = Utc::now() - Duration::days(LOOKBACK_DAYS);
    // Gather all candidate transactions from each linked account.
    let mut all_candidates: Vec<(String, String, i64, DateTime<Utc>, String)> = Vec::new();
    for aid in account_ids {
        let candidates = transfers::find_candidates(conn, aid, since)?;
        for (txn_id, amt, posted) in candidates {
            let merchant = conn
                .query_row(
                    "SELECT merchant_raw FROM transactions WHERE id = ?1",
                    [&txn_id],
                    |r| r.get::<_, String>(0),
                )
                .unwrap_or_default();
            all_candidates.push((aid.clone(), txn_id, amt, posted, merchant));
        }
    }

    let mut detected = Vec::new();
    let now = Utc::now();
    for i in 0..all_candidates.len() {
        let (acc_a, id_a, amt_a, posted_a, merchant_a) = &all_candidates[i];
        for j in (i + 1)..all_candidates.len() {
            let (acc_b, id_b, amt_b, posted_b, merchant_b) = &all_candidates[j];
            // Must be different accounts, same |amount|, opposite sign.
            if acc_a == acc_b {
                continue;
            }
            if amt_a.abs() != amt_b.abs() {
                continue;
            }
            if amt_a.signum() == amt_b.signum() {
                continue;
            }

            let (
                outflow_id,
                inflow_id,
                outflow_date,
                inflow_date,
                outflow_merchant,
                _inflow_merchant,
            ) = if *amt_a < 0 {
                (id_a, id_b, *posted_a, *posted_b, merchant_a, merchant_b)
            } else {
                (id_b, id_a, *posted_b, *posted_a, merchant_b, merchant_a)
            };

            let days_apart = (outflow_date - inflow_date).num_days().abs();

            let desc_lower = outflow_merchant.to_lowercase();
            let keyword_match = TRANSFER_KEYWORDS.iter().any(|kw| desc_lower.contains(kw));

            let confidence = if days_apart <= HIGH_CONFIDENCE_DAYS && keyword_match {
                "high"
            } else if days_apart <= MEDIUM_CONFIDENCE_DAYS {
                "medium"
            } else if days_apart <= LOW_CONFIDENCE_DAYS {
                "low"
            } else {
                continue;
            };

            let transfer = TransactionTransfer {
                id: Uuid::new_v4().to_string(),
                from_transaction_id: outflow_id.clone(),
                to_transaction_id: inflow_id.clone(),
                confidence: confidence.to_string(),
                detected_at: now,
                user_confirmed: false,
            };
            // Only insert if this pair doesn't already exist.
            if transfer_exists(conn, outflow_id, inflow_id)? {
                continue;
            }
            transfers::insert(conn, transfer.clone())?;
            detected.push(transfer);
        }
    }
    Ok(detected)
}

fn transfer_exists(conn: &Connection, from_id: &str, to_id: &str) -> CoreResult<bool> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM transaction_transfers WHERE from_transaction_id = ?1 AND to_transaction_id = ?2)",
        rusqlite::params![from_id, to_id],
        |r| r.get(0),
    )?;
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{
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

    fn seed_account(conn: &mut Connection, name: &str) -> String {
        accounts::insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: name.into(),
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

    fn seed_txn(
        conn: &mut Connection,
        account_id: &str,
        amount_cents: i64,
        merchant: &str,
        days_ago: i64,
    ) -> String {
        transactions::insert(
            conn,
            NewTransaction {
                account_id: account_id.into(),
                posted_at: Utc::now() - Duration::days(days_ago),
                amount_cents,
                merchant_raw: merchant.into(),
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
    fn detects_high_confidence_transfer() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc_a = seed_account(&mut conn, "Checking");
        let acc_b = seed_account(&mut conn, "Savings");

        let _ = seed_txn(&mut conn, &acc_a, -5000, "Online Transfer", 0);
        let _ = seed_txn(&mut conn, &acc_b, 5000, "Transfer from Checking", 0);

        let result = detect_transfers(&mut conn, &[acc_a.clone(), acc_b.clone()]).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].confidence, "high");
    }

    #[test]
    fn skips_different_amounts() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc_a = seed_account(&mut conn, "Checking");
        let acc_b = seed_account(&mut conn, "Savings");

        let _ = seed_txn(&mut conn, &acc_a, -1000, "Transfer", 0);
        let _ = seed_txn(&mut conn, &acc_b, 2000, "Transfer", 0);

        let result = detect_transfers(&mut conn, &[acc_a, acc_b]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn skips_same_sign() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc_a = seed_account(&mut conn, "Checking");
        let acc_b = seed_account(&mut conn, "Savings");

        let _ = seed_txn(&mut conn, &acc_a, -5000, "Transfer", 0);
        let _ = seed_txn(&mut conn, &acc_b, -5000, "Transfer", 0);

        let result = detect_transfers(&mut conn, &[acc_a, acc_b]).unwrap();
        assert!(result.is_empty());
    }
}

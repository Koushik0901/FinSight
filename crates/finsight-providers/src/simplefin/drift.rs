//! Balance-drift monitor for SimpleFin-linked accounts.
//!
//! After each sync, compare the bank-reported balance (from
//! `account_balances`) against the transaction-ledger balance. When drift
//! exceeds a threshold, create an alert in `simplefin_alerts`.

use chrono::Utc;
use finsight_core::models::SimpleFinAlert;
use finsight_core::repos::alerts;
use rusqlite::Connection;
use uuid::Uuid;

const DRIFT_ERROR_THRESHOLD_CENTS: i64 = 500; // $5.00
const DRIFT_WARNING_THRESHOLD_CENTS: i64 = 1; // $0.01

pub fn check_drift(
    conn: &mut Connection,
    account_id: &str,
) -> Result<Option<SimpleFinAlert>, finsight_core::CoreError> {
    let ledger: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions WHERE account_id = ?1",
        [account_id],
        |r| r.get(0),
    )?;

    let reported: Option<i64> = conn
        .query_row(
            "SELECT balance_cents FROM account_balances \
             WHERE account_id = ?1 AND source = 'simplefin' ORDER BY as_of_date DESC LIMIT 1",
            [account_id],
            |r| r.get(0),
        )
        .ok();

    let Some(reported) = reported else {
        return Ok(None);
    };

    let drift = ledger - reported;

    if drift == 0 {
        return Ok(None);
    }

    let abs_drift = drift.abs();

    let (severity, message) = if abs_drift > DRIFT_ERROR_THRESHOLD_CENTS {
        (
            "error".to_string(),
            format!(
                "Balance drift of ${:.2} detected — bank reports ${:.2} but ledger sums to ${:.2}",
                drift as f64 / 100.0,
                reported as f64 / 100.0,
                ledger as f64 / 100.0,
            ),
        )
    } else if abs_drift >= DRIFT_WARNING_THRESHOLD_CENTS {
        (
            "warning".to_string(),
            format!(
                "Small balance drift of ${:.2} — bank reports ${:.2} but ledger sums to ${:.2}",
                drift as f64 / 100.0,
                reported as f64 / 100.0,
                ledger as f64 / 100.0,
            ),
        )
    } else {
        // Should not reach here since drift != 0 was checked above, but keep for safety.
        return Ok(None);
    };

    if alerts::has_recent_unacknowledged(conn, account_id, "drift")? {
        return Ok(None);
    }

    let alert = SimpleFinAlert {
        id: Uuid::new_v4().to_string(),
        account_id: account_id.to_string(),
        alert_type: "drift".to_string(),
        severity,
        message,
        details_json: Some(format!(
            r#"{{"drift_cents":{},"ledger_cents":{},"reported_cents":{}}}"#,
            drift, ledger, reported
        )),
        acknowledged_at: None,
        created_at: Utc::now(),
    };
    alerts::create(conn, alert.clone())?;
    Ok(Some(alert))
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

    fn seed_account(conn: &mut Connection) -> String {
        accounts::insert(
            conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
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
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                limit_cents: None,
                original_balance_cents: None,
                started_at: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn no_drift_when_reported_matches_ledger() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        // Update the seed balance (which is today, 0) to 12345
        conn.execute(
            "UPDATE account_balances SET balance_cents = 12345, source = 'simplefin' WHERE account_id = ?1",
            [&acc],
        ).unwrap();
        let _ = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acc.clone(),
                posted_at: Utc::now(),
                amount_cents: 12345,
                merchant_raw: "Deposit".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
                activity: None,
            },
        )
        .unwrap();

        let alert = check_drift(&mut conn, &acc).unwrap();
        assert!(alert.is_none());
    }

    #[test]
    fn error_drift_when_large_mismatch() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        // Update the seed balance to 100000
        conn.execute(
            "UPDATE account_balances SET balance_cents = 100000, source = 'simplefin' WHERE account_id = ?1",
            [&acc],
        ).unwrap();
        let _ = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acc.clone(),
                posted_at: Utc::now(),
                amount_cents: 50000,
                merchant_raw: "Deposit".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
                activity: None,
            },
        )
        .unwrap();

        let alert = check_drift(&mut conn, &acc).unwrap();
        assert!(alert.is_some());
        let a = alert.unwrap();
        assert_eq!(a.severity, "error");
        assert!(a.message.contains("drift"));
    }

    #[test]
    fn dedupes_recent_alerts() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        conn.execute(
            "UPDATE account_balances SET balance_cents = 200000, source = 'simplefin' WHERE account_id = ?1",
            [&acc],
        ).unwrap();
        let _ = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acc.clone(),
                posted_at: Utc::now(),
                amount_cents: 100000,
                merchant_raw: "Deposit".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
                activity: None,
            },
        )
        .unwrap();

        let first = check_drift(&mut conn, &acc).unwrap();
        assert!(first.is_some());
        let second = check_drift(&mut conn, &acc).unwrap();
        assert!(second.is_none());
    }
}

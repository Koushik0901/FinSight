use rusqlite::Connection;

use finsight_core::models::{NewTransaction, TransactionStatus};
use finsight_core::repos::{accounts, transactions};

use crate::error::{ProviderError, ProviderResult};
use super::client::SimpleFinClient;

pub struct SimpleFinImportSummary {
    pub added: usize,
    pub skipped: usize,
}

pub async fn import_simplefin_account(
    access_url: &str,
    simplefin_id: &str,
    local_account_id: &str,
    conn: &mut Connection,
) -> ProviderResult<SimpleFinImportSummary> {
    let client = SimpleFinClient::new(access_url)?;

    let accounts_list = client.list_accounts().await?;
    let sfin_account = accounts_list
        .into_iter()
        .find(|a| a.id == simplefin_id)
        .ok_or(ProviderError::AccountNotFound)?;

    let existing_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
            [local_account_id],
            |r| r.get(0),
        )
        .map_err(|e| ProviderError::Core(e.into()))?;
    let is_initial = existing_count == 0;

    let sfin_transactions = client
        .fetch_transactions(simplefin_id, 0)
        .await?;

    let mut new_transactions = Vec::with_capacity(sfin_transactions.len());
    for tx in &sfin_transactions {
        let posted_at = chrono::DateTime::from_timestamp(tx.posted, 0)
            .unwrap_or_else(chrono::Utc::now);
        let amount_cents = parse_amount_cents(&tx.amount)?;
        new_transactions.push(NewTransaction {
            account_id: local_account_id.to_string(),
            amount_cents,
            merchant_raw: tx.payee.clone(),
            notes: Some(tx.description.clone()),
            posted_at,
            status: TransactionStatus::Cleared,
            imported_id: Some(tx.id.clone()),
            source: Some("simplefin".to_string()),
            category_id: None,
        });
    }

    let mut added = 0usize;
    let mut skipped = 0usize;

    if is_initial {
        let reported_balance_cents = parse_amount_cents(&sfin_account.balance)?;
        let imported_total: i64 = new_transactions.iter().map(|t| t.amount_cents).sum();
        let starting_balance_cents = reported_balance_cents - imported_total;

        let oldest_date = new_transactions
            .iter()
            .map(|t| t.posted_at)
            .min()
            .unwrap_or_else(chrono::Utc::now);

        transactions::insert(
            conn,
            NewTransaction {
                account_id: local_account_id.to_string(),
                amount_cents: starting_balance_cents,
                merchant_raw: "Starting balance".to_string(),
                notes: Some("Imported from SimpleFin".to_string()),
                posted_at: oldest_date,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: Some("simplefin".to_string()),
                category_id: None,
            },
        )
        .map_err(|e| ProviderError::Core(e.into()))?;
        added += 1;
    }

    for tx in new_transactions.into_iter() {
        if let Some(ref imported_id) = tx.imported_id {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM transactions WHERE account_id = ?1 AND imported_id = ?2 LIMIT 1",
                    [local_account_id, imported_id],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if exists {
                skipped += 1;
                continue;
            }
        }
        transactions::insert(conn, tx).map_err(|e| ProviderError::Core(e.into()))?;
        added += 1;
    }

    let now = chrono::Utc::now();
    accounts::update_sync_metadata(
        conn,
        local_account_id,
        Some(simplefin_id),
        Some(now),
    )
    .map_err(|e| ProviderError::Core(e.into()))?;

    Ok(SimpleFinImportSummary { added, skipped })
}

fn parse_amount_cents(amount: &str) -> ProviderResult<i64> {
    let parts: Vec<&str> = amount.split('.').collect();
    let dollars: i64 = parts[0]
        .parse()
        .map_err(|_| ProviderError::Internal("invalid amount".into()))?;
    let cents_str = parts.get(1).unwrap_or(&"0");
    let cent_digits: String = cents_str.chars().take(2).collect();
    let cents: i64 = if cent_digits.is_empty() {
        0
    } else {
        cent_digits
            .parse()
            .map_err(|_| ProviderError::Internal("invalid cent amount".into()))?
    };
    if dollars >= 0 {
        Ok(dollars * 100 + cents)
    } else {
        Ok(dollars * 100 - cents)
    }
}

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json;

use finsight_core::models::{NewImportCandidate, NewImportCandidateMatch};
use finsight_core::models::{NewTransaction, TransactionStatus};
use finsight_core::repos::imports::ImportSource;
use finsight_core::repos::{accounts, import_candidates, imports, transactions};

use super::client::SimpleFinClient;
use super::matcher::{reconcile_excluding, ReconciliationDecision};
use super::models::{SimpleFinAccount, SimpleFinTransaction};
use crate::error::{ProviderError, ProviderResult};

/// Number of days before the last sync to re-fetch, to catch retroactively
/// posted transactions (Simledge/Actual Budget pattern).
const SUBSEQUENT_LOOKBACK_DAYS: i64 = 14;
/// SimpleFIN bridges can reject uncapped initial syncs. Keep the initial
/// request inside the common 45-day recommended provider window, then synthesize the
/// opening balance from the bank-reported balance and imported transactions.
const INITIAL_LOOKBACK_DAYS: i64 = 44;

#[derive(Debug, Clone)]
pub struct PendingImport {
    pub simplefin_id: String,
    pub local_account_id: String,
    pub sfin_account: SimpleFinAccount,
    pub transactions: Vec<SimpleFinTransaction>,
}

pub struct SimpleFinImportSummary {
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,
    pub queued_for_review: usize,
}

pub async fn fetch_simplefin_data(
    access_url: &str,
    simplefin_id: &str,
    local_account_id: &str,
    last_synced_at: Option<DateTime<Utc>>,
    import_pending: bool,
) -> ProviderResult<PendingImport> {
    let client = SimpleFinClient::new(access_url)?;
    let accounts_list = client.list_accounts().await?;
    let sfin_account = accounts_list
        .into_iter()
        .find(|a| a.id == simplefin_id)
        .ok_or(ProviderError::AccountNotFound)?;

    let start_epoch = sync_start_epoch(last_synced_at, Utc::now());

    tracing::info!(
        simplefin_id,
        start_epoch,
        last_synced_at = ?last_synced_at,
        import_pending,
        "Fetching SimpleFin transactions"
    );

    let transactions = client
        .fetch_transactions(simplefin_id, start_epoch, import_pending)
        .await?;

    Ok(PendingImport {
        simplefin_id: simplefin_id.to_string(),
        local_account_id: local_account_id.to_string(),
        sfin_account,
        transactions,
    })
}

fn sync_start_epoch(last_synced_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> i64 {
    match last_synced_at {
        Some(t) => (t - Duration::days(SUBSEQUENT_LOOKBACK_DAYS)).timestamp(),
        None => (now - Duration::days(INITIAL_LOOKBACK_DAYS)).timestamp(),
    }
}

pub fn commit_simplefin_import(
    pending: PendingImport,
    conn: &mut Connection,
) -> ProviderResult<SimpleFinImportSummary> {
    commit_simplefin_import_for_run(pending, conn, None)
}

pub fn commit_simplefin_import_for_run(
    pending: PendingImport,
    conn: &mut Connection,
    sync_run_id: Option<&str>,
) -> ProviderResult<SimpleFinImportSummary> {
    let import_id = imports::start(
        conn,
        ImportSource::SimpleFin,
        None,
        Some(&pending.local_account_id),
    )?;

    let result = commit_simplefin_import_inner(pending, conn, &import_id, sync_run_id);

    match &result {
        Ok(summary) => {
            imports::finish(
                conn,
                &import_id,
                summary.added as u32,
                summary.skipped as u32,
                None,
            )?;
        }
        Err(e) => {
            imports::finish(conn, &import_id, 0, 0, Some(&e.to_string()))?;
        }
    }

    result
}

fn commit_simplefin_import_inner(
    pending: PendingImport,
    conn: &mut Connection,
    import_id: &str,
    sync_run_id: Option<&str>,
) -> ProviderResult<SimpleFinImportSummary> {
    let existing_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
            [&pending.local_account_id],
            |r| r.get(0),
        )
        .map_err(|e| ProviderError::Core(e.into()))?;
    let is_initial = existing_count == 0;

    // Parse and sort transactions by posted date (oldest first). The spec says
    // /accounts returns transactions ordered by posted, but bridges vary.
    let mut parsed: Vec<(NewTransaction, SimpleFinTransaction)> = pending
        .transactions
        .iter()
        .map(|tx| {
            let mut mapped = map_transaction(&pending.local_account_id, tx)?;
            mapped.external_account_id = Some(pending.simplefin_id.clone());
            Ok((mapped, tx.clone()))
        })
        .collect::<ProviderResult<Vec<_>>>()?;
    parsed.sort_by_key(|(new_tx, _)| new_tx.posted_at);

    let mut new_transactions = Vec::with_capacity(parsed.len());
    let mut raw_by_imported_id: std::collections::HashMap<String, SimpleFinTransaction> =
        std::collections::HashMap::new();
    for (new_tx, raw) in parsed {
        if let Some(id) = &new_tx.imported_id {
            raw_by_imported_id.insert(id.clone(), raw);
        }
        new_transactions.push(new_tx);
    }

    let mut added = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut queued_for_review = 0usize;

    conn.execute(
        "UPDATE accounts SET \
            available_balance_cents = ?1, \
            balance_date = ?2, \
            extra_json = ?3, \
            raw_json = ?4 \
         WHERE id = ?5",
        params![
            pending
                .sfin_account
                .available_balance
                .as_ref()
                .map(|b| parse_amount_cents(b))
                .transpose()?,
            DateTime::from_timestamp(pending.sfin_account.balance_date, 0).map(|d| d.to_rfc3339()),
            pending.sfin_account.extra.as_ref().map(|v| v.to_string()),
            serde_json::to_string(&pending.sfin_account).ok(),
            &pending.local_account_id,
        ],
    )
    .map_err(|e| ProviderError::Core(e.into()))?;

    if is_initial {
        let reported_balance_cents = parse_amount_cents(&pending.sfin_account.balance)?;
        let imported_total: i64 = new_transactions.iter().map(|t| t.amount_cents).sum();
        let starting_balance_cents = reported_balance_cents - imported_total;

        let oldest_date = new_transactions
            .iter()
            .map(|t| t.posted_at)
            .min()
            .unwrap_or_else(Utc::now);

        transactions::insert(
            conn,
            NewTransaction {
                account_id: pending.local_account_id.clone(),
                amount_cents: starting_balance_cents,
                merchant_raw: "Starting balance".to_string(),
                notes: Some("Imported from SimpleFin".to_string()),
                posted_at: oldest_date,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: Some("simplefin".to_string()),
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: Some(pending.simplefin_id.clone()),
                category_id: None,
                activity: None,
            },
        )
        .map_err(|e| ProviderError::Core(e.into()))?;
        added += 1;
    }

    let mut matched_existing_ids = std::collections::HashSet::new();
    for tx in new_transactions {
        let imported_id = tx.imported_id.clone();
        let raw_json = imported_id
            .as_ref()
            .and_then(|id| raw_by_imported_id.get(id))
            .map(|raw| serde_json::to_string(raw).unwrap_or_default());

        match reconcile_excluding(
            conn,
            &tx.account_id,
            &tx,
            imported_id.as_deref(),
            7,
            &matched_existing_ids,
        )? {
            ReconciliationDecision::AutoMatch(existing) => {
                let should_update = tx.pending != existing.pending
                    || tx.amount_cents != existing.amount_cents
                    || tx.posted_at != existing.posted_at
                    || tx.merchant_raw != existing.merchant_raw
                    || tx.status != existing.status
                    || tx.imported_id != existing.imported_id
                    || tx.source != existing.source
                    || tx.external_tx_id != existing.external_tx_id
                    || tx.external_account_id != existing.external_account_id
                    || raw_json != existing.raw_synced_data;

                if should_update {
                    update_matched_transaction(conn, &existing.id, &tx, raw_json)?;
                    updated += 1;
                } else {
                    skipped += 1;
                }
                matched_existing_ids.insert(existing.id);
            }
            ReconciliationDecision::NeedsReview {
                matches,
                confidence,
                reason,
            } => {
                import_candidates::create(
                    conn,
                    NewImportCandidate {
                        source: "simplefin".to_string(),
                        import_id: Some(import_id.to_string()),
                        sync_run_id: sync_run_id.map(str::to_string),
                        account_id: tx.account_id.clone(),
                        candidate_json: serde_json::to_string(&tx).map_err(|e| {
                            ProviderError::Internal(format!("serialize candidate: {e}"))
                        })?,
                        raw_payload_json: raw_json,
                        imported_id: tx.imported_id.clone(),
                        external_tx_id: tx.external_tx_id.clone(),
                        external_account_id: tx.external_account_id.clone(),
                        posted_at: tx.posted_at,
                        amount_cents: tx.amount_cents,
                        merchant_raw: tx.merchant_raw.clone(),
                        confidence,
                        reason,
                    },
                    matches
                        .into_iter()
                        .map(|m| NewImportCandidateMatch {
                            transaction_id: m.transaction.id,
                            match_kind: m.match_kind,
                            score: m.score,
                            is_recommended: m.is_recommended,
                            explanation_json: m.explanation_json,
                        })
                        .collect(),
                )
                .map_err(|e| ProviderError::Core(e.into()))?;
                queued_for_review += 1;
            }
            ReconciliationDecision::None => {
                let mut new_tx = tx;
                new_tx.raw_synced_data = raw_json;
                transactions::insert(conn, new_tx).map_err(|e| ProviderError::Core(e.into()))?;
                added += 1;
            }
        }
    }

    // Update the bank-reported balance snapshot.
    let balance_cents = parse_amount_cents(&pending.sfin_account.balance)?;
    let available_balance_cents = pending
        .sfin_account
        .available_balance
        .as_ref()
        .map(|b| parse_amount_cents(b))
        .transpose()?;
    let balance_date = DateTime::from_timestamp(pending.sfin_account.balance_date, 0)
        .map(|d| d.date_naive().to_string())
        .unwrap_or_else(|| Utc::now().date_naive().to_string());

    accounts::upsert_balance_snapshot(
        conn,
        &pending.local_account_id,
        &balance_date,
        balance_cents,
        available_balance_cents,
        Some("simplefin"),
    )
    .map_err(|e| ProviderError::Core(e.into()))?;

    // Update last_synced_at.
    accounts::update_sync_metadata(
        conn,
        &pending.local_account_id,
        Some(&pending.simplefin_id),
        Some(Utc::now()),
    )
    .map_err(|e| ProviderError::Core(e.into()))?;

    Ok(SimpleFinImportSummary {
        added,
        updated,
        skipped,
        queued_for_review,
    })
}

fn map_transaction(
    local_account_id: &str,
    tx: &SimpleFinTransaction,
) -> ProviderResult<NewTransaction> {
    let posted_at = DateTime::from_timestamp(tx.posted, 0).unwrap_or_else(Utc::now);
    let amount_cents = parse_amount_cents(&tx.amount)?;
    let status = if tx.pending {
        TransactionStatus::Pending
    } else {
        TransactionStatus::Cleared
    };

    Ok(NewTransaction {
        account_id: local_account_id.to_string(),
        amount_cents,
        merchant_raw: tx.payee.clone(),
        notes: Some(tx.description.clone()),
        posted_at,
        status,
        imported_id: Some(tx.id.clone()),
        source: Some("simplefin".to_string()),
        raw_synced_data: None,
        pending: tx.pending,
        external_tx_id: Some(tx.id.clone()),
        external_account_id: None,
        category_id: None,
        activity: None,
    })
}

fn update_matched_transaction(
    conn: &mut Connection,
    id: &str,
    incoming: &NewTransaction,
    raw_json: Option<String>,
) -> ProviderResult<()> {
    // Preserve user-edited fields: category_id and notes are not overwritten.
    // Update amount, posted_at, merchant_raw, status, imported_id, source, raw_synced_data.
    conn.execute(
        "UPDATE transactions SET \
            amount_cents = ?1, \
            posted_at = ?2, \
            merchant_raw = ?3, \
            status = ?4, \
            imported_id = ?5, \
            source = ?6, \
            raw_synced_data = ?7, \
            pending = ?8, \
            external_tx_id = ?9, \
            external_account_id = ?10 \
         WHERE id = ?11",
        params![
            incoming.amount_cents,
            incoming.posted_at.to_rfc3339(),
            &incoming.merchant_raw,
            incoming.status.as_db(),
            &incoming.imported_id,
            &incoming.source,
            raw_json,
            incoming.pending,
            &incoming.external_tx_id,
            &incoming.external_account_id,
            id,
        ],
    )
    .map_err(|e| ProviderError::Core(e.into()))?;
    Ok(())
}

/// Parse a SimpleFin numeric string (e.g. "-33293.43" or "100.5") into integer cents.
fn parse_amount_cents(amount: &str) -> ProviderResult<i64> {
    let decimal = amount
        .trim()
        .parse::<Decimal>()
        .map_err(|_| ProviderError::Internal(format!("invalid amount: {}", amount)))?;
    // Round to 2 decimal places using standard half-up, then convert to cents.
    let rounded = decimal.round_dp(2);
    let cents = (rounded * Decimal::from(100))
        .round_dp(0)
        .to_i64()
        .ok_or_else(|| ProviderError::Internal(format!("amount out of range: {}", amount)))?;
    Ok(cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_amount_variants() {
        assert_eq!(parse_amount_cents("100.50").unwrap(), 10050);
        assert_eq!(parse_amount_cents("100.5").unwrap(), 10050);
        assert_eq!(parse_amount_cents(".50").unwrap(), 50);
        assert_eq!(parse_amount_cents("100").unwrap(), 10000);
        assert_eq!(parse_amount_cents("-100.5").unwrap(), -10050);
        assert_eq!(parse_amount_cents("100.999").unwrap(), 10100);
    }

    #[test]
    fn initial_sync_start_date_stays_inside_bridge_range() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        assert_eq!(
            sync_start_epoch(None, now),
            (now - Duration::days(INITIAL_LOOKBACK_DAYS)).timestamp()
        );
    }

    #[test]
    fn subsequent_sync_uses_short_lookback_from_last_sync() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let last_sync = now - Duration::days(30);
        assert_eq!(
            sync_start_epoch(Some(last_sync), now),
            (last_sync - Duration::days(SUBSEQUENT_LOOKBACK_DAYS)).timestamp()
        );
    }

    #[test]
    fn simplefin_enriches_existing_csv_match_without_duplicate() {
        let dir = tempfile::TempDir::new().unwrap();
        let key = finsight_core::keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("sync.sqlcipher"), &key).unwrap();
        finsight_core::db::run_migrations(&db).unwrap();
        let mut conn = db.get().unwrap();

        let account = finsight_core::repos::accounts::insert(
            &mut conn,
            finsight_core::models::NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: finsight_core::models::AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#fff".into(),
                opening_balance_cents: 0,
                source: "simplefin".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: Some("sf-acct".into()),
                nickname: None,
                connection_id: Some("conn".into()),
                institution_id: None,
                external_account_id: Some("sf-acct".into()),
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

        finsight_core::repos::transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: account.id.clone(),
                posted_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                amount_cents: -1234,
                merchant_raw: "Coffee Shop".into(),
                category_id: None,
                notes: Some("user note".into()),
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: Some("csv".into()),
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
                activity: None,
            },
        )
        .unwrap();

        let summary = commit_simplefin_import(
            PendingImport {
                simplefin_id: "sf-acct".into(),
                local_account_id: account.id.clone(),
                sfin_account: SimpleFinAccount {
                    id: "sf-acct".into(),
                    name: "Checking".into(),
                    connection_name: Some("Bank".into()),
                    connection_id: Some("conn".into()),
                    currency: "USD".into(),
                    balance: "-12.34".into(),
                    available_balance: None,
                    balance_date: 1_700_000_000,
                    transactions: Some(vec![]),
                    extra: None,
                },
                transactions: vec![SimpleFinTransaction {
                    id: "sf-tx-1".into(),
                    posted: 1_700_000_000,
                    transacted_at: None,
                    amount: "-12.34".into(),
                    description: "synced memo".into(),
                    payee: "Coffee Shop".into(),
                    pending: false,
                    extra: None,
                }],
            },
            &mut conn,
        )
        .unwrap();

        assert_eq!(summary.added, 0);
        assert_eq!(summary.updated, 1);
        let row: (i64, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), MAX(source), MAX(imported_id), MAX(notes) FROM transactions WHERE account_id = ?1 AND merchant_raw = 'Coffee Shop'",
                [&account.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(row.0, 1);
        assert_eq!(row.1.as_deref(), Some("simplefin"));
        assert_eq!(row.2.as_deref(), Some("sf-tx-1"));
        assert_eq!(row.3.as_deref(), Some("user note"));
    }
}

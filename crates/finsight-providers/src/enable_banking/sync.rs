use chrono::{DateTime, Utc};

use super::client::EnableBankingClient;
use super::models::{EnableBankingAccount, EnableBankingTransaction};
use crate::error::{ProviderError, ProviderResult};
use finsight_core::models::{NewTransaction, TransactionStatus};

/// Result of an Enable Banking fetch — mirrors SimpleFIN's `PendingImport`
/// but keeps the EB shapes so the caller can decide how to commit.
#[derive(Debug, Clone)]
pub struct EnableBankingSyncData {
    pub accounts: Vec<EnableBankingAccount>,
    pub transactions: Vec<EnableBankingTransaction>,
}

#[derive(Debug, Clone)]
pub struct EnablePendingImport {
    pub account_uid: String,
    pub local_account_id: String,
    pub eb_account: EnableBankingAccount,
    pub transactions: Vec<EnableBankingTransaction>,
}

pub async fn fetch_enable_data(token: &str) -> ProviderResult<EnableBankingSyncData> {
    let client = EnableBankingClient::new(token)?;
    let accounts = client.list_accounts().await?;
    Ok(EnableBankingSyncData {
        accounts,
        transactions: vec![],
    })
}

/// Test-only helper that lets `enable_banking` tests point at a wiremock server.
/// Production code calls `fetch_enable_data`; tests that need HTTP isolation use this.
pub async fn fetch_enable_data_with_base_url(
    token: &str,
    base_url: &str,
) -> ProviderResult<EnableBankingSyncData> {
    let client = EnableBankingClient::with_base_url(token, base_url)?;
    let accounts = client.list_accounts().await?;
    Ok(EnableBankingSyncData {
        accounts,
        transactions: vec![],
    })
}

/// Fetch transactions for a single EB account and map them to `NewTransaction`
/// rows for a given local FinSight account id. Mirrors SimpleFIN's
/// `fetch_simplefin_data` signature but for Enable Banking.
pub async fn fetch_enable_account_data(
    token: &str,
    eb_account_uid: &str,
    local_account_id: &str,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> ProviderResult<EnablePendingImport> {
    let client = EnableBankingClient::new(token)?;
    let accounts = client.list_accounts().await?;
    let eb_account = accounts
        .into_iter()
        .find(|a| a.id == eb_account_uid)
        .ok_or(ProviderError::AccountNotFound)?;
    let txs = client
        .list_transactions(eb_account_uid, date_from, date_to)
        .await?;
    Ok(EnablePendingImport {
        account_uid: eb_account_uid.to_string(),
        local_account_id: local_account_id.to_string(),
        eb_account,
        transactions: txs,
    })
}

impl EnablePendingImport {
    /// Map a single EB transaction into a `NewTransaction` for the local account.
    /// Keeps the same invariants as `simplefin::sync::map_transaction`:
    /// - `posted_at` from `booking_date` or `value_date` or now
    /// - `amount_cents` from signed amount string
    /// - `merchant_raw` from description / remittance
    /// - `status` always Cleared (EB only returns BOOK, never pending)
    pub fn map_transaction(&self, tx: &EnableBankingTransaction) -> ProviderResult<NewTransaction> {
        let posted_at = parse_eb_date(tx.booking_date.as_deref().or(tx.value_date.as_deref()));
        let amount_cents = parse_amount_cents(&tx.amount)?;
        Ok(NewTransaction {
            account_id: self.local_account_id.clone(),
            amount_cents,
            merchant_raw: if tx.description.is_empty() {
                tx.creditor_name
                    .clone()
                    .or_else(|| tx.debtor_name.clone())
                    .unwrap_or_else(|| tx.id.clone())
            } else {
                tx.description.clone()
            },
            notes: Some(format!(
                "Enable Banking {}{}",
                tx.status.as_deref().unwrap_or("BOOK"),
                if let Some(raw) = &tx.raw {
                    format!(" {}", raw)
                } else {
                    String::new()
                }
            )),
            posted_at,
            status: TransactionStatus::Cleared,
            imported_id: Some(tx.id.clone()),
            source: Some("enable_banking".to_string()),
            raw_synced_data: tx.raw.as_ref().map(|v| v.to_string()),
            pending: false,
            external_tx_id: Some(tx.id.clone()),
            external_account_id: Some(self.account_uid.clone()),
            category_id: None,
            activity: None,
        })
    }
}

fn parse_eb_date(s: Option<&str>) -> DateTime<Utc> {
    if let Some(raw) = s {
        // EB dates are YYYY-MM-DD (no time). Try that, then RFC3339, then fallback to now.
        if let Ok(d) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            return d.and_hms_opt(12, 0, 0).unwrap().and_utc();
        }
        if let Ok(dt) = raw.parse::<DateTime<Utc>>() {
            return dt;
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.fZ") {
            return dt.and_utc();
        }
    }
    Utc::now()
}

fn parse_amount_cents(amount: &str) -> ProviderResult<i64> {
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use rust_decimal::RoundingStrategy;
    let decimal: Decimal = amount
        .trim()
        .parse()
        .map_err(|_| ProviderError::Internal(format!("invalid amount: {}", amount)))?;
    let rounded = decimal.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let cents = (rounded * Decimal::from(100))
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .ok_or_else(|| ProviderError::Internal(format!("amount out of range: {}", amount)))?;
    if cents > crate::amount::MAX_SAFE_CENTS || cents < -crate::amount::MAX_SAFE_CENTS {
        return Err(ProviderError::Internal(format!(
            "amount out of range: {}",
            amount
        )));
    }
    Ok(cents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_amount_rejects_beyond_display_safe_cents() {
        assert_eq!(
            parse_amount_cents("22517998136852.47").unwrap(),
            (1 << 51) - 1
        );
        assert!(parse_amount_cents("22517998136852.48").is_err());
    }

    #[tokio::test]
    async fn fetch_enable_data_with_base_url_isolates_via_http() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/accounts"))
            .and(header("Authorization", "Bearer tok-x"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accounts": [{"id": "x-1", "name": "X", "currency": "EUR"}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/accounts"))
            .and(header("Authorization", "Bearer tok-y"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accounts": [{"id": "y-1", "name": "Y", "currency": "EUR"}]
            })))
            .mount(&server)
            .await;
        let base = server.uri();
        let a = fetch_enable_data_with_base_url("tok-x", &base)
            .await
            .unwrap();
        let b = fetch_enable_data_with_base_url("tok-y", &base)
            .await
            .unwrap();
        assert_ne!(a.accounts[0].id, b.accounts[0].id);
    }

    #[tokio::test]
    async fn fetch_enable_data_literal_token_hits_network_not_stub() {
        // Without network, a literal token should now error (no stub)
        // Brief's original used ProviderError::Auth which does not exist; adapted to real variants.
        let err = fetch_enable_data("token-a").await.unwrap_err();
        assert!(
            matches!(
                err,
                ProviderError::Internal(_)
                    | ProviderError::Forbidden
                    | ProviderError::ServerError(_)
                    | ProviderError::Http(_)
            ),
            "literal token-a should not return stubbed acc-a-1, got {err:?}"
        );
    }

    #[test]
    fn parse_amount_cents_midpoint_away() {
        assert_eq!(parse_amount_cents("12.345").unwrap(), 1235);
        assert_eq!(parse_amount_cents("-12.345").unwrap(), -1235);
        assert_eq!(parse_amount_cents("12.34").unwrap(), 1234);
    }

    #[test]
    fn parse_amount_cents_variants() {
        assert_eq!(parse_amount_cents("12.34").unwrap(), 1234);
        assert_eq!(parse_amount_cents("-12.34").unwrap(), -1234);
        // MidpointAwayFromZero: 12.345 -> 12.35
        assert_eq!(parse_amount_cents("12.345").unwrap(), 1235);
        assert_eq!(parse_amount_cents("12.346").unwrap(), 1235);
        assert_eq!(parse_amount_cents("0.50").unwrap(), 50);
    }

    #[test]
    fn parse_eb_date_uses_midday_utc() {
        let dt = parse_eb_date(Some("2026-08-10"));
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-08-10");
        assert_eq!(dt.format("%H:%M").to_string(), "12:00");
    }
}

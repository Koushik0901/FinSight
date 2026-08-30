use std::time::Duration;

use reqwest;
use serde_json::Value;
use url::Url;

use super::models::{EnableBankingAccount, EnableBankingTransaction};
use crate::error::{ProviderError, ProviderResult};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_BASE: &str = "https://api.enablebanking.com/";

/// Enable Banking client — EU open-banking via https://api.enablebanking.com
/// Auth is `Authorization: Bearer <JWT>` (JWT is the `token` here).
/// For tests the base URL can be overridden to a wiremock http server.
pub struct EnableBankingClient {
    token: String,
    base_url: Url,
    http: reqwest::Client,
}

impl EnableBankingClient {
    /// Create client pointing at the production Enable Banking API.
    pub fn new(token: &str) -> ProviderResult<Self> {
        Self::with_base_url(token, DEFAULT_BASE)
    }

    /// Create client with an explicit base URL (for wiremock tests).
    /// Allows `http://` for test servers; production should remain `https://`.
    pub fn with_base_url(token: &str, base_url: &str) -> ProviderResult<Self> {
        if token.is_empty() {
            return Err(ProviderError::Internal(
                "Enable Banking token is empty".into(),
            ));
        }
        let mut url = Url::parse(base_url).map_err(|_| {
            ProviderError::Internal(format!("invalid Enable Banking base URL: {base_url}"))
        })?;
        // Ensure trailing slash so `join` works predictably (mirrors SimpleFinClient).
        if !url.path().ends_with('/') {
            let p = format!("{}/", url.path());
            url.set_path(&p);
        }
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::default())
            .build()
            .map_err(ProviderError::Http)?;
        Ok(Self {
            token: token.to_string(),
            base_url: url,
            http,
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// List accounts visible in the current session.
    /// Real flow: after POST /sessions you own a session_id; Enable Banking's
    /// session model returns accounts, but this simplified bearer flow exposes
    /// GET /accounts (and the per-account sub-resources) directly, which matches
    /// the sandbox mock and the tilisy.com compat host.
    ///
    /// Handles three response shapes:
    /// - `{ "accounts": [...] }`  (real POST /sessions response wrapper)
    /// - `[ ... ]`                 (stub / wiremock direct array)
    /// - `{ "accounts_data": [...] }` (GET /sessions/{id} shape)
    pub async fn list_accounts(&self) -> ProviderResult<Vec<EnableBankingAccount>> {
        let url = self
            .base_url
            .join("accounts")
            .map_err(|_| ProviderError::Internal("join accounts".into()))?;
        let res = self
            .http
            .get(url.as_str())
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(ProviderError::Http)?;

        let status = res.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Forbidden);
        }
        if !status.is_success() {
            return Err(ProviderError::ServerError(format!(
                "GET accounts returned {status}"
            )));
        }
        let value: Value = res.json().await.map_err(ProviderError::Http)?;
        parse_accounts_value(value)
    }

    /// List accounts raw via GET /sessions/{session_id} (when the caller holds a
    /// session id rather than a bare bearer for /accounts). Exposed for future
    /// use; not used in the current bearer-only flow but kept to mirror the
    /// real Enable Banking contract so we can evolve without a second client.
    pub async fn get_session(&self, session_id: &str) -> ProviderResult<Value> {
        let url = self
            .base_url
            .join(&format!("sessions/{}", session_id))
            .map_err(|_| ProviderError::Internal("join sessions".into()))?;
        let res = self
            .http
            .get(url.as_str())
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(ProviderError::Http)?;
        if !res.status().is_success() {
            return Err(ProviderError::ServerError(format!(
                "GET sessions/{session_id} returned {}",
                res.status()
            )));
        }
        Ok(res.json().await.map_err(ProviderError::Http)?)
    }

    /// Fetch transactions for a single account.
    /// Real endpoint: `GET /accounts/{uid}/transactions?date_from=YYYY-MM-DD&date_to=YYYY-MM-DD`
    /// Handles:
    /// - `{ "transactions": [...] }` (HAL)
    /// - `[ ... ]` (direct array stub)
    pub async fn list_transactions(
        &self,
        account_uid: &str,
        date_from: Option<&str>,
        date_to: Option<&str>,
    ) -> ProviderResult<Vec<EnableBankingTransaction>> {
        let path = format!("accounts/{}/transactions", account_uid);
        let url = self
            .base_url
            .join(&path)
            .map_err(|_| ProviderError::Internal("join transactions".into()))?;
        let mut req = self.http.get(url.as_str()).bearer_auth(&self.token);
        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(d) = date_from {
            query.push(("date_from", d));
        }
        if let Some(d) = date_to {
            query.push(("date_to", d));
        }
        if !query.is_empty() {
            req = req.query(&query);
        }
        let res = req.send().await.map_err(ProviderError::Http)?;
        let status = res.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Forbidden);
        }
        if !status.is_success() {
            return Err(ProviderError::ServerError(format!(
                "GET {path} returned {status}"
            )));
        }
        let value: Value = res.json().await.map_err(ProviderError::Http)?;
        parse_transactions_value(value)
    }

    /// Fetch balances for an account (`GET /accounts/{uid}/balances`).
    pub async fn list_balances(&self, account_uid: &str) -> ProviderResult<Value> {
        let path = format!("accounts/{}/balances", account_uid);
        let url = self
            .base_url
            .join(&path)
            .map_err(|_| ProviderError::Internal("join balances".into()))?;
        let res = self
            .http
            .get(url.as_str())
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(ProviderError::Http)?;
        if !res.status().is_success() {
            return Err(ProviderError::ServerError(format!(
                "GET {path} returned {}",
                res.status()
            )));
        }
        Ok(res.json().await.map_err(ProviderError::Http)?)
    }
}

pub(crate) fn parse_accounts_value(value: Value) -> ProviderResult<Vec<EnableBankingAccount>> {
    if value.is_array() {
        let arr: Vec<EnableBankingAccount> =
            serde_json::from_value(value).map_err(|e| ProviderError::Internal(e.to_string()))?;
        return Ok(arr);
    }
    if let Some(arr) = value.get("accounts") {
        let accounts: Vec<EnableBankingAccount> = serde_json::from_value(arr.clone())
            .map_err(|e| ProviderError::Internal(format!("parse accounts: {e}")))?;
        return Ok(accounts);
    }
    if let Some(arr) = value.get("accounts_data") {
        // GET /sessions/{id} returns { accounts_data: [{uid, identification_hash}] }
        // Those entries are minimal (uid only); hydrate to EnableBankingAccount with uid alone.
        let minimal: Vec<Value> = serde_json::from_value(arr.clone())
            .map_err(|e| ProviderError::Internal(format!("parse accounts_data: {e}")))?;
        let mut out = Vec::with_capacity(minimal.len());
        for v in minimal {
            let acc: EnableBankingAccount = serde_json::from_value(v)
                .map_err(|e| ProviderError::Internal(format!("account_data entry: {e}")))?;
            out.push(acc);
        }
        return Ok(out);
    }
    // Some ASPSP wrappers return { account: {...} } singular — treat as one.
    if value.get("uid").is_some() || value.get("account_id").is_some() || value.get("id").is_some()
    {
        let one: EnableBankingAccount = serde_json::from_value(value)
            .map_err(|e| ProviderError::Internal(format!("parse single account: {e}")))?;
        return Ok(vec![one]);
    }
    Err(ProviderError::Internal(format!(
        "unexpected accounts JSON shape: {}",
        serde_json::to_string(&value).unwrap_or_default()
    )))
}

pub(crate) fn parse_transactions_value(
    value: Value,
) -> ProviderResult<Vec<EnableBankingTransaction>> {
    if value.is_array() {
        let arr: Vec<EnableBankingTransaction> =
            serde_json::from_value(value).map_err(|e| ProviderError::Internal(e.to_string()))?;
        return Ok(arr);
    }
    if let Some(arr) = value.get("transactions") {
        let txs: Vec<EnableBankingTransaction> = serde_json::from_value(arr.clone())
            .map_err(|e| ProviderError::Internal(format!("parse transactions: {e}")))?;
        return Ok(txs);
    }
    Err(ProviderError::Internal(format!(
        "unexpected transactions JSON shape: {}",
        serde_json::to_string(&value).unwrap_or_default()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn new_rejects_empty_token() {
        assert!(EnableBankingClient::new("").is_err());
    }

    #[test]
    fn with_base_url_allows_http_for_wiremock() {
        let c = EnableBankingClient::with_base_url("tok", "http://localhost:1234/").unwrap();
        assert_eq!(c.base_url().as_str(), "http://localhost:1234/");
    }

    #[test]
    fn with_base_url_adds_trailing_slash() {
        let c = EnableBankingClient::with_base_url("tok", "http://localhost:1234/api").unwrap();
        assert_eq!(c.base_url().as_str(), "http://localhost:1234/api/");
    }

    #[tokio::test]
    async fn list_accounts_parses_wrapped_and_array_shapes() {
        let server = MockServer::start().await;
        // wrapped
        Mock::given(method("GET"))
            .and(path("/accounts"))
            .and(header("Authorization", "Bearer tok-wrapped"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accounts": [{"uid": "uid-1", "name": "Checking", "currency": "EUR", "account_id": {"iban": "FI123"}}]
            })))
            .mount(&server)
            .await;
        let c = EnableBankingClient::with_base_url("tok-wrapped", &server.uri()).unwrap();
        let accs = c.list_accounts().await.unwrap();
        assert_eq!(accs.len(), 1);
        assert_eq!(accs[0].id, "uid-1");
        assert_eq!(accs[0].iban.as_deref(), Some("FI123"));

        // direct array (stub shape for plan's simple test)
        let server2 = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/accounts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": "stub-1", "name": "Stub", "currency": "EUR"}
            ])))
            .mount(&server2)
            .await;
        let c2 = EnableBankingClient::with_base_url("tok", &server2.uri()).unwrap();
        let accs2 = c2.list_accounts().await.unwrap();
        assert_eq!(accs2[0].id, "stub-1");
    }

    #[tokio::test]
    async fn list_accounts_bearer_isolation_per_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/accounts"))
            .and(header("Authorization", "Bearer token-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accounts": [{"id": "acc-a-1", "name": "A", "currency": "EUR"}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/accounts"))
            .and(header("Authorization", "Bearer token-b"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accounts": [{"id": "acc-b-1", "name": "B", "currency": "EUR"}]
            })))
            .mount(&server)
            .await;
        let base = server.uri();
        let a = EnableBankingClient::with_base_url("token-a", &base)
            .unwrap()
            .list_accounts()
            .await
            .unwrap();
        let b = EnableBankingClient::with_base_url("token-b", &base)
            .unwrap()
            .list_accounts()
            .await
            .unwrap();
        assert_ne!(a[0].id, b[0].id);
        assert_eq!(a[0].id, "acc-a-1");
        assert_eq!(b[0].id, "acc-b-1");
    }

    #[tokio::test]
    async fn list_transactions_parses_hal_shape() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/accounts/uid-1/transactions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "transactions": [{
                    "transaction_id": "tx1",
                    "transaction_amount": {"amount": "12.34", "currency": "EUR"},
                    "credit_debit_indicator": "CRDT",
                    "booking_date": "2026-08-01",
                    "remittance_information": ["Salary"],
                    "status": "BOOK"
                }],
                "continuation_key": null
            })))
            .mount(&server)
            .await;
        let c = EnableBankingClient::with_base_url("tok", &server.uri()).unwrap();
        let txs = c.list_transactions("uid-1", None, None).await.unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].id, "tx1");
        assert_eq!(txs[0].amount, "12.34");
        assert_eq!(txs[0].description, "Salary");
    }
}

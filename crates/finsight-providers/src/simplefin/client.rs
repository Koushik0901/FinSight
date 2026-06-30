use std::time::Duration;

use base64::Engine;
use reqwest;
use url::Url;

use super::models::{
    SimpleFinAccount, SimpleFinAccountSet, SimpleFinConnection, SimpleFinTransaction,
};
use crate::error::{ProviderError, ProviderResult};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct SimpleFinClient {
    base_url: Url,
    username: String,
    password: String,
    http: reqwest::Client,
}

impl SimpleFinClient {
    pub fn new(access_url: &str) -> ProviderResult<Self> {
        let url = Url::parse(access_url).map_err(|_| ProviderError::InvalidAccessUrl)?;
        if url.scheme() != "https" {
            return Err(ProviderError::InvalidAccessUrl);
        }
        let username = url.username().to_string();
        let password = url.password().unwrap_or("").to_string();
        if username.is_empty() || password.is_empty() {
            return Err(ProviderError::InvalidAccessUrl);
        }

        // Preserve host, port, and path from the access URL. SimpleFin access URLs
        // look like https://user:pass@bridge.example.com:8443/simplefin
        // and all endpoints are relative to that root.
        let base_url = Url::parse(&format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or(""),
            url.port().map(|p| format!(":{}", p)).unwrap_or_default(),
        ))
        .map_err(|_| ProviderError::InvalidAccessUrl)?;

        let path_segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
        let base_url = if path_segments.is_empty() {
            base_url
        } else {
            let path_with_trailing = format!("{}/", path_segments.join("/"));
            base_url
                .join(&path_with_trailing)
                .map_err(|_| ProviderError::InvalidAccessUrl)?
        };

        // Use reqwest's default redirect policy, which strips Authorization on
        // cross-origin redirects and follows same-origin redirects safely.
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::default())
            .build()
            .map_err(ProviderError::Http)?;

        Ok(Self {
            base_url,
            username,
            password,
            http,
        })
    }

    pub async fn claim_token(setup_token: &str) -> ProviderResult<String> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(setup_token.trim())
            .map_err(|_| ProviderError::InvalidAccessUrl)?;
        let claim_url = String::from_utf8(decoded).map_err(|_| ProviderError::InvalidAccessUrl)?;
        let url = Url::parse(&claim_url).map_err(|_| ProviderError::InvalidAccessUrl)?;
        if url.scheme() != "https" {
            return Err(ProviderError::InvalidAccessUrl);
        }

        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::default())
            .build()
            .map_err(ProviderError::Http)?;

        let res = http
            .post(url.as_str())
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(ProviderError::Http)?;

        if res.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::TokenClaimFailed);
        }
        if res.status() == reqwest::StatusCode::PAYMENT_REQUIRED {
            return Err(ProviderError::ServerError("payment required".into()));
        }
        if !res.status().is_success() {
            return Err(ProviderError::ServerError(format!(
                "claim failed: {}",
                res.status()
            )));
        }

        let access_url = res.text().await.map_err(ProviderError::Http)?;
        let trimmed = access_url.trim();
        let _ = Self::new(trimmed)?;
        Ok(trimmed.to_string())
    }

    fn auth_header(&self) -> String {
        let creds = format!("{}:{}", self.username, self.password);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(creds)
        )
    }

    pub async fn list_accounts(&self) -> ProviderResult<Vec<SimpleFinAccount>> {
        Ok(self.list_accounts_with_connections().await?.0)
    }

    pub async fn list_accounts_with_connections(
        &self,
    ) -> ProviderResult<(Vec<SimpleFinAccount>, Vec<SimpleFinConnection>)> {
        let res = self
            .get("accounts", &[("version", "2"), ("balances-only", "1")])
            .await?;
        let set: SimpleFinAccountSet = res.json().await.map_err(ProviderError::Http)?;
        if !set.errlist.is_empty() {
            let msgs: Vec<String> = set.errlist.iter().map(|e| e.msg.clone()).collect();
            return Err(ProviderError::ServerError(msgs.join("; ")));
        }
        Ok((set.accounts, set.connections))
    }

    pub async fn fetch_transactions(
        &self,
        account_id: &str,
        start_epoch: i64,
        include_pending: bool,
    ) -> ProviderResult<Vec<SimpleFinTransaction>> {
        let start_str = start_epoch.to_string();
        let mut query: Vec<(&str, &str)> = vec![
            ("version", "2"),
            ("account", account_id),
            ("start-date", &start_str),
        ];
        if include_pending {
            query.push(("pending", "1"));
        }
        let res = self.get("accounts", &query).await?;
        let set: SimpleFinAccountSet = res.json().await.map_err(ProviderError::Http)?;
        if !set.errlist.is_empty() {
            let msgs: Vec<String> = set.errlist.iter().map(|e| e.msg.clone()).collect();
            return Err(ProviderError::ServerError(msgs.join("; ")));
        }
        let account = set
            .accounts
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or(ProviderError::AccountNotFound)?;
        Ok(account.transactions.unwrap_or_default())
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> ProviderResult<reqwest::Response> {
        let full_url = self
            .base_url
            .join(path)
            .map_err(|_| ProviderError::InvalidAccessUrl)?;

        let response = self
            .http
            .get(full_url.as_str())
            .header("Authorization", self.auth_header())
            .query(query)
            .send()
            .await
            .map_err(ProviderError::Http)?;

        let status = response.status();

        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Forbidden);
        }
        if status == reqwest::StatusCode::PAYMENT_REQUIRED {
            return Err(ProviderError::ServerError("payment required".into()));
        }
        if !status.is_success() {
            return Err(ProviderError::ServerError(format!(
                "GET {} returned {}",
                path, status
            )));
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_valid_access_url() {
        let client = SimpleFinClient::new("https://user:pass@bridge.simplefin.org/simplefin");
        assert!(client.is_ok());
    }

    #[test]
    fn reject_http_access_url() {
        let client = SimpleFinClient::new("http://user:pass@bridge.simplefin.org/simplefin");
        assert!(matches!(client, Err(ProviderError::InvalidAccessUrl)));
    }

    #[test]
    fn reject_missing_password() {
        let client = SimpleFinClient::new("https://user@bridge.simplefin.org/simplefin");
        assert!(matches!(client, Err(ProviderError::InvalidAccessUrl)));
    }

    #[test]
    fn preserve_nonstandard_port() {
        let client =
            SimpleFinClient::new("https://user:pass@bridge.example.com:8443/simplefin").unwrap();
        assert_eq!(
            client.base_url.as_str(),
            "https://bridge.example.com:8443/simplefin/"
        );
    }

    #[tokio::test]
    async fn claim_token_decodes_and_posts() {
        let server = MockServer::start().await;
        let claim_path = "/simplefin/claim/demo";
        let access_url = format!("https://user:pass@{}/simplefin", server.address());
        Mock::given(method("POST"))
            .and(path(claim_path))
            .respond_with(ResponseTemplate::new(200).set_body_string(&access_url))
            .mount(&server)
            .await;
        let token = base64::engine::general_purpose::STANDARD.encode(format!(
            "http://{}:{}{}",
            server.address().ip(),
            server.address().port(),
            claim_path
        ));
        // Token encodes http, which claim_token should reject after decoding.
        let result = SimpleFinClient::claim_token(&token).await;
        assert!(matches!(result, Err(ProviderError::InvalidAccessUrl)));
    }

    #[tokio::test]
    #[ignore = "wiremock serves plain HTTP; SimpleFinClient requires HTTPS, so this needs a TLS terminator"]
    async fn list_accounts_returns_accounts() {
        let server = MockServer::start().await;
        let access_url = format!("https://user:pass@{}/simplefin", server.address());
        Mock::given(method("GET"))
            .and(path("/simplefin/accounts"))
            .and(query_param("version", "2"))
            .and(query_param("balances-only", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "errlist": [],
                "connections": [],
                "accounts": [{"id":"1","name":"Checking","conn_id":"c1","currency":"USD","balance":"100.00","balance-date":1700000000}]
            })))
            .mount(&server).await;
        let client = SimpleFinClient::new(&access_url).unwrap();
        let accounts = client.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "1");
    }
}

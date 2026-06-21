use std::time::Duration;

use base64::Engine;
use reqwest;
use url::Url;

use crate::error::{ProviderError, ProviderResult};
use super::models::{SimpleFinAccount, SimpleFinAccountSet, SimpleFinTransaction};

const MAX_REDIRECTS: u8 = 5;

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
        if username.is_empty() {
            return Err(ProviderError::InvalidAccessUrl);
        }
        let base_url = Url::parse(&format!(
            "{}://{}",
            url.scheme(),
            url.host_str().unwrap_or("")
        ))
        .map_err(|_| ProviderError::InvalidAccessUrl)?;
        let path_segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.collect())
            .unwrap_or_default();
        let base_url = if path_segments.is_empty() {
            base_url
        } else {
            let path_with_trailing = format!("{}/", path_segments.join("/"));
            base_url.join(&path_with_trailing).map_err(|_| ProviderError::InvalidAccessUrl)?
        };

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ProviderError::Http(e))?;

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
        let claim_url =
            String::from_utf8(decoded).map_err(|_| ProviderError::InvalidAccessUrl)?;
        let url = Url::parse(&claim_url).map_err(|_| ProviderError::InvalidAccessUrl)?;
        if url.scheme() != "https" {
            return Err(ProviderError::InvalidAccessUrl);
        }

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ProviderError::Http(e))?;

        let res = http
            .post(url.as_str())
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(ProviderError::Http)?;

        if res.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::TokenClaimFailed);
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
        let res = self
            .get("accounts", &[("balances-only", "1")])
            .await?;
        let set: SimpleFinAccountSet = res.json().await.map_err(ProviderError::Http)?;
        Ok(set.accounts)
    }

    pub async fn fetch_transactions(
        &self,
        account_id: &str,
        start_epoch: i64,
    ) -> ProviderResult<Vec<SimpleFinTransaction>> {
        let res = self
            .get(
                "accounts",
                &[
                    ("account", account_id),
                    ("start-date", &start_epoch.to_string()),
                    ("pending", "0"),
                ],
            )
            .await?;
        let set: SimpleFinAccountSet = res.json().await.map_err(ProviderError::Http)?;
        let account = set
            .accounts
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or(ProviderError::AccountNotFound)?;
        Ok(account.transactions.unwrap_or_default())
    }

    async fn get(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> ProviderResult<reqwest::Response> {
        let mut full_url = self.base_url.clone();
        if !path.is_empty() {
            full_url = full_url.join(path).map_err(|_| ProviderError::InvalidAccessUrl)?;
        }

        let mut current_url = full_url.clone();

        for _hop in 0..MAX_REDIRECTS {
            let response = self
                .http
                .get(current_url.as_str())
                .header("Authorization", self.auth_header())
                .query(query)
                .send()
                .await
                .map_err(ProviderError::Http)?;

            let status = response.status();

            if status.is_redirection() {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or(ProviderError::ServerError(
                        "redirect without location".into(),
                    ))?;
                current_url = full_url
                    .join(location)
                    .map_err(|_| ProviderError::ServerError("invalid redirect url".into()))?;
                continue;
            }

            if status == reqwest::StatusCode::FORBIDDEN {
                return Err(ProviderError::Forbidden);
            }
            if !status.is_success() {
                return Err(ProviderError::ServerError(format!(
                    "GET {} returned {}",
                    path, status
                )));
            }
            return Ok(response);
        }

        Err(ProviderError::ServerError("too many redirects".into()))
    }
}

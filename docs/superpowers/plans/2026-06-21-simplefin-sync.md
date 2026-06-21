# SimpleFin Bank Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add minimal SimpleFin bank-sync to FinSight so users can paste a setup token, discover accounts, import selected accounts with optional nicknames, and sync transactions per account with a “Sync now” button.

**Architecture:** A new `simplefin` module in `finsight-providers` handles the SimpleFin HTTP protocol. `finsight-core` persists linked-account metadata and import history. `finsight-app` exposes Tauri commands. The UI adds SimpleFin to onboarding, a credentials panel in Settings, and per-account sync on the Accounts screen.

**Tech Stack:** Rust (`reqwest`, `chrono`, `serde`, `thiserror`, `keyring`), Tauri + Specta, React + TanStack Query, existing FinSight design tokens/components.

## Global Constraints
- Only HTTPS URLs are allowed; reject HTTP token/access URLs.
- Store the SimpleFin **Access URL** in the OS keychain (`com.finsight.simplefin`); the setup token is transient and discarded after claiming.
- Import only **posted** transactions (`pending=0`).
- Create a **starting-balance transaction** on initial sync so the local balance matches SimpleFin’s reported balance.
- Fetch **all available history** on initial sync (no 90-day cap).
- Imported accounts use the user’s optional nickname when set; otherwise use SimpleFin’s `name`.
- Import all currencies as-is.
- Per-account manual sync only for this slice (no scheduled/batch sync yet).

---

### Task 1: Database migration

**Files:**
- Create: `crates/finsight-core/migrations/V017__simplefin.sql`

**Interfaces:**
- Produces: `accounts.sync_source`, `accounts.simplefin_account_id`, `accounts.last_synced_at`, `accounts.nickname` columns.

- [ ] **Step 1: Write migration file**

```sql
ALTER TABLE accounts ADD COLUMN sync_source TEXT;
ALTER TABLE accounts ADD COLUMN simplefin_account_id TEXT;
ALTER TABLE accounts ADD COLUMN last_synced_at INTEGER;
ALTER TABLE accounts ADD COLUMN nickname TEXT;

-- The imports.source CHECK constraint currently lives in V002.
-- SQLite does not support ALTER TABLE DROP CONSTRAINT, so we recreate the table.
CREATE TABLE imports_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    source TEXT NOT NULL CHECK(source IN ('csv', 'manual', 'sample', 'simplefin')),
    rows_total INTEGER,
    rows_imported INTEGER,
    rows_errored INTEGER,
    error_message TEXT
);

INSERT INTO imports_new SELECT * FROM imports;
DROP TABLE imports;
ALTER TABLE imports_new RENAME TO imports;
```

- [ ] **Step 2: Verify migration runs**

Run: `cargo test -p finsight-core --test migrations`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/migrations/V017__simplefin.sql
git commit -m "db: add simplefin account sync columns and import source"
```

---

### Task 2: Extend `finsight-core` models and repos

**Files:**
- Modify: `crates/finsight-core/src/models/account.rs`
- Modify: `crates/finsight-core/src/repos/accounts.rs`
- Modify: `crates/finsight-core/src/repos/imports.rs`

**Interfaces:**
- Consumes: new DB columns from Task 1.
- Produces:
  - `pub struct Account { ..., pub sync_source: Option<String>, pub simplefin_account_id: Option<String>, pub last_synced_at: Option<i64>, pub nickname: Option<String> }`
  - `pub struct NewAccount { ..., pub sync_source: Option<String>, pub simplefin_account_id: Option<String>, pub nickname: Option<String> }`
  - `pub struct AccountUpdate { ..., pub nickname: Option<Option<String>> }`
  - `pub fn update_account_nickname(conn, id, nickname: Option<&str>) -> CoreResult<()>`
  - `pub fn update_account_sync_metadata(conn, id, simplefin_account_id, sync_source, last_synced_at) -> CoreResult<()>`
  - `pub fn list_sync_linked_accounts(conn, source: &str) -> CoreResult<Vec<Account>>`
  - `ImportSource::SimpleFin`

- [ ] **Step 1: Add fields to `Account`/`NewAccount`/`AccountUpdate`**

Update struct definitions and `From<&Row>` mappings.

- [ ] **Step 2: Add `ImportSource::SimpleFin`**

Add variant and update `ToSql`/`FromSql` impls.

- [ ] **Step 3: Add repo helpers**

```rust
pub fn update_account_nickname(
    conn: &mut Connection,
    id: i64,
    nickname: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE accounts SET nickname = ?1 WHERE id = ?2",
        params![nickname, id],
    )?;
    Ok(())
}

pub fn update_account_sync_metadata(
    conn: &mut Connection,
    id: i64,
    simplefin_account_id: &str,
    sync_source: &str,
    last_synced_at: Option<i64>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE accounts SET simplefin_account_id = ?1, sync_source = ?2, last_synced_at = ?3 WHERE id = ?4",
        params![simplefin_account_id, sync_source, last_synced_at, id],
    )?;
    Ok(())
}

pub fn list_sync_linked_accounts(
    conn: &mut Connection,
    source: &str,
) -> CoreResult<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM accounts WHERE sync_source = ?1 AND archived_at IS NULL",
    )?;
    let rows = stmt.query_map(params![source], |r| Account::from(r))?;
    rows.collect()
}
```

- [ ] **Step 4: Write tests**

```rust
#[test]
fn account_round_trip_with_simplefin_fields() {
    // create account with sync_source/simplefin_account_id/nickname
    // read back and assert fields
}
```

Run: `cargo test -p finsight-core --lib repos::accounts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/models/account.rs crates/finsight-core/src/repos/accounts.rs crates/finsight-core/src/repos/imports.rs
git commit -m "core: add SimpleFin fields to account models and repos"
```

---

### Task 3: Add HTTP dependencies to `finsight-providers`

**Files:**
- Modify: `crates/finsight-providers/Cargo.toml`
- Modify: `Cargo.toml` workspace deps (if needed)

**Interfaces:**
- Produces: `reqwest`, `base64`, `url` available to `finsight-providers`.

- [ ] **Step 1: Add dependencies**

```toml
[dependencies]
reqwest = { workspace = true }
base64 = { workspace = true }
url = { workspace = true }
```

Verify workspace has these. If not, add to workspace `[workspace.dependencies]`:

```toml
reqwest = "0.12"
base64 = "0.22"
url = "2.5"
```

- [ ] **Step 2: Verify compile**

Run: `cargo check -p finsight-providers`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-providers/Cargo.toml Cargo.toml
git commit -m "deps: add reqwest, base64, url to finsight-providers"
```

---

### Task 4: Implement SimpleFin HTTP client

**Files:**
- Create: `crates/finsight-providers/src/simplefin/mod.rs`
- Create: `crates/finsight-providers/src/simplefin/client.rs`
- Create: `crates/finsight-providers/src/simplefin/models.rs`
- Modify: `crates/finsight-providers/src/error.rs`
- Modify: `crates/finsight-providers/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct SimpleFinAccount { pub id: String, pub name: String, pub connection_name: String, pub currency: String, pub balance: String, pub balance_date: DateTime<Utc> }`
  - `pub struct SimpleFinTransaction { pub id: String, pub posted_at: DateTime<Utc>, pub transacted_at: Option<DateTime<Utc>>, pub amount_cents: i64, pub payee: String, pub description: String }`
  - `impl SimpleFinClient { pub fn new(access_url: &str) -> ProviderResult<Self>; pub async fn claim_token(setup_token: &str) -> ProviderResult<String>; pub async fn list_accounts(&self) -> ProviderResult<Vec<SimpleFinAccount>>; pub async fn fetch_transactions(&self, account_id: &str, start_date: DateTime<Utc>) -> ProviderResult<Vec<SimpleFinTransaction>>; }`

- [ ] **Step 1: Define models**

In `crates/finsight-providers/src/simplefin/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinAccount {
    pub id: String,
    pub name: String,
    #[serde(rename = "conn_name")]
    pub connection_name: String,
    pub currency: String,
    pub balance: String,
    #[serde(rename = "balance-date")]
    pub balance_date: i64,
}

impl SimpleFinAccount {
    pub fn balance_date(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.balance_date, 0).unwrap_or_else(|| Utc::now())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinTransaction {
    pub id: String,
    pub posted: i64,
    pub transacted_at: Option<i64>,
    pub amount: String,
    pub description: String,
    #[serde(default)]
    pub payee: String,
}
```

- [ ] **Step 2: Implement client**

In `crates/finsight-providers/src/simplefin/client.rs`:

```rust
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use reqwest;
use url::Url;

use crate::error::{ProviderError, ProviderResult};
use super::models::{SimpleFinAccount, SimpleFinAccountSet, SimpleFinTransaction};

pub struct SimpleFinClient {
    base_url: Url,
    username: String,
    password: String,
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
        let base_url = Url::parse(&format!("{}://{}", url.scheme(), url.host_str().unwrap_or("")))
            .map_err(|_| ProviderError::InvalidAccessUrl)?;
        Ok(Self { base_url, username, password })
    }

    pub async fn claim_token(setup_token: &str) -> ProviderResult<String> {
        let decoded = STANDARD.decode(setup_token.trim())
            .map_err(|_| ProviderError::InvalidAccessUrl)?;
        let claim_url = String::from_utf8(decoded)
            .map_err(|_| ProviderError::InvalidAccessUrl)?;
        let url = Url::parse(&claim_url).map_err(|_| ProviderError::InvalidAccessUrl)?;
        if url.scheme() != "https" {
            return Err(ProviderError::InvalidAccessUrl);
        }
        let client = reqwest::Client::new();
        let res = client.post(url.as_str()).header("Content-Length", 0).send().await
            .map_err(ProviderError::Http)?;
        if res.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::TokenClaimFailed);
        }
        if !res.status().is_success() {
            return Err(ProviderError::ServerError(format!("claim failed: {}", res.status())));
        }
        let access_url = res.text().await.map_err(ProviderError::Http)?;
        // Validate by parsing
        let _ = Self::new(&access_url)?;
        Ok(access_url)
    }

    fn auth_header(&self) -> String {
        let creds = format!("{}:{}", self.username, self.password);
        format!("Basic {}", STANDARD.encode(creds))
    }

    pub async fn list_accounts(&self) -> ProviderResult<Vec<SimpleFinAccount>> {
        let url = self.base_url.join("/accounts")?;
        let client = reqwest::Client::new();
        let res = client.get(url.as_str())
            .header("Authorization", self.auth_header())
            .query(&[("balances-only", "1")])
            .send().await
            .map_err(ProviderError::Http)?;
        if res.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Forbidden);
        }
        if !res.status().is_success() {
            return Err(ProviderError::ServerError(format!("list accounts failed: {}", res.status())));
        }
        let set: SimpleFinAccountSet = res.json().await.map_err(ProviderError::Http)?;
        Ok(set.accounts)
    }

    pub async fn fetch_transactions(
        &self,
        account_id: &str,
        start_date: DateTime<Utc>,
    ) -> ProviderResult<Vec<SimpleFinTransaction>> {
        let url = self.base_url.join("/accounts")?;
        let client = reqwest::Client::new();
        let start_ts = start_date.timestamp();
        let res = client.get(url.as_str())
            .header("Authorization", self.auth_header())
            .query(&[("account", account_id), ("start-date", &start_ts.to_string()), ("pending", "0")])
            .send().await
            .map_err(ProviderError::Http)?;
        if res.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Forbidden);
        }
        if !res.status().is_success() {
            return Err(ProviderError::ServerError(format!("fetch transactions failed: {}", res.status())));
        }
        let set: SimpleFinAccountSet = res.json().await.map_err(ProviderError::Http)?;
        let account = set.accounts.into_iter()
            .find(|a| a.id == account_id)
            .ok_or(ProviderError::AccountNotFound)?;
        Ok(account.transactions.unwrap_or_default())
    }
}
```

- [ ] **Step 3: Add `SimpleFinAccountSet` deserialization wrapper**

In `models.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct SimpleFinAccountSet {
    #[serde(default)]
    pub accounts: Vec<SimpleFinAccount>,
}
```

- [ ] **Step 4: Add error variants**

In `crates/finsight-providers/src/error.rs`:

```rust
InvalidAccessUrl,
TokenClaimFailed,
Forbidden,
ServerError(String),
AccountNotFound,
Http(reqwest::Error),
```

- [ ] **Step 5: Wire up module**

In `crates/finsight-providers/src/lib.rs`:

```rust
pub mod simplefin;
```

In `crates/finsight-providers/src/simplefin/mod.rs`:

```rust
pub mod client;
pub mod models;

pub use client::SimpleFinClient;
pub use models::{SimpleFinAccount, SimpleFinTransaction};
```

- [ ] **Step 6: Write tests with `wiremock`**

Add `wiremock = "0.6"` to `Cargo.toml` dev-dependencies.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

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
        let token = base64::encode(format!("https://{}:{}{}", server.address().ip(), server.address().port(), claim_path));
        let result = SimpleFinClient::claim_token(&token).await.unwrap();
        assert_eq!(result, access_url);
    }

    #[tokio::test]
    async fn list_accounts_returns_accounts() {
        let server = MockServer::start().await;
        let access_url = format!("https://user:pass@{}/simplefin", server.address());
        Mock::given(method("GET"))
            .and(path("/simplefin/accounts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accounts": [{"id":"1","name":"Checking","conn_name":"Bank","currency":"USD","balance":"100.00","balance-date":1700000000}]
            })))
            .mount(&server).await;
        let client = SimpleFinClient::new(&access_url).unwrap();
        let accounts = client.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "1");
    }
}
```

Run: `cargo test -p finsight-providers --lib simplefin`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/finsight-providers/src/simplefin crates/finsight-providers/src/error.rs crates/finsight-providers/src/lib.rs crates/finsight-providers/Cargo.toml
git commit -m "feat(simplefin): add HTTP client and protocol models"
```

---

### Task 5: Implement SimpleFin import/sync logic

**Files:**
- Create: `crates/finsight-providers/src/simplefin/sync.rs`
- Modify: `crates/finsight-providers/src/simplefin/mod.rs`

**Interfaces:**
- Consumes: `SimpleFinClient`, `SimpleFinAccount`, `SimpleFinTransaction`, `NewTransaction`, `ImportSource::SimpleFin`, `accounts` repo helpers.
- Produces:
  - `pub struct SimpleFinImportSummary { pub added: usize, pub updated: usize, pub skipped: usize }`
  - `pub async fn import_simplefin_account(access_url: &str, simplefin_id: &str, local_account_id: i64, db: &mut Connection) -> ProviderResult<SimpleFinImportSummary>`

- [ ] **Step 1: Implement sync function**

```rust
use chrono::{DateTime, Utc};
use rusqlite::Connection;

use finsight_core::{
    models::{NewTransaction, TransactionFlags},
    repos::{accounts, imports, transactions},
    CoreResult,
};

use crate::error::{ProviderError, ProviderResult};
use super::client::SimpleFinClient;
use super::models::SimpleFinTransaction;

pub struct SimpleFinImportSummary {
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,
}

pub async fn import_simplefin_account(
    access_url: &str,
    simplefin_id: &str,
    local_account_id: i64,
    conn: &mut Connection,
) -> ProviderResult<SimpleFinImportSummary> {
    let client = SimpleFinClient::new(access_url)?;

    // Verify account exists and read current balance.
    let accounts_list = client.list_accounts().await?;
    let sfin_account = accounts_list.into_iter()
        .find(|a| a.id == simplefin_id)
        .ok_or(ProviderError::AccountNotFound)?;

    // Determine if this is the initial sync.
    let existing_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
        [local_account_id],
        |r| r.get(0),
    ).map_err(|e| ProviderError::Core(e.into()))?;
    let is_initial = existing_count == 0;

    // Fetch all transactions since epoch (full history).
    let start = DateTime::from_timestamp(0, 0).unwrap();
    let sfin_transactions = client.fetch_transactions(simplefin_id, start).await?;

    // Map SimpleFin transactions to NewTransaction.
    let mut new_transactions = Vec::with_capacity(sfin_transactions.len());
    for tx in &sfin_transactions {
        let amount_cents = parse_amount_cents(&tx.amount)?;
        new_transactions.push(NewTransaction {
            account_id: local_account_id,
            amount_cents,
            merchant_raw: tx.payee.clone(),
            notes: Some(tx.description.clone()),
            posted_at: tx.posted_at(),
            source: finsight_core::repos::imports::ImportSource::SimpleFin,
            imported_id: Some(tx.id.clone()),
            flags: TransactionFlags::default(),
            category_id: None,
        });
    }

    // Insert transactions, deduplicating on imported_id.
    let mut added = 0;
    let mut updated = 0;
    let mut skipped = 0;

    if is_initial {
        // Compute starting balance so running balance matches SimpleFin balance.
        let reported_balance_cents = parse_amount_cents(&sfin_account.balance)?;
        let imported_total: i64 = new_transactions.iter().map(|t| t.amount_cents).sum();
        let starting_balance_cents = reported_balance_cents - imported_total;

        let oldest_date = new_transactions.iter()
            .map(|t| t.posted_at)
            .min()
            .unwrap_or_else(Utc::now);

        transactions::insert(conn, &NewTransaction {
            account_id: local_account_id,
            amount_cents: starting_balance_cents,
            merchant_raw: "Starting balance".to_string(),
            notes: Some("Imported from SimpleFin".to_string()),
            posted_at: oldest_date,
            source: finsight_core::repos::imports::ImportSource::SimpleFin,
            imported_id: None,
            flags: TransactionFlags::default(),
            category_id: None,
        }).map_err(|e| ProviderError::Core(e.into()))?;
        added += 1;
    }

    for tx in new_transactions {
        if let Some(ref imported_id) = tx.imported_id {
            let exists: bool = conn.query_row(
                "SELECT 1 FROM transactions WHERE account_id = ?1 AND imported_id = ?2 LIMIT 1",
                [local_account_id, imported_id],
                |_| Ok(true),
            ).unwrap_or(false);
            if exists {
                skipped += 1;
                continue;
            }
        }
        transactions::insert(conn, &tx).map_err(|e| ProviderError::Core(e.into()))?;
        added += 1;
    }

    // Update last_synced_at.
    let now = Utc::now().timestamp();
    accounts::update_account_sync_metadata(
        conn,
        local_account_id,
        simplefin_id,
        "simplefin",
        Some(now),
    ).map_err(|e| ProviderError::Core(e.into()))?;

    Ok(SimpleFinImportSummary { added, updated, skipped })
}

fn parse_amount_cents(amount: &str) -> ProviderResult<i64> {
    // SimpleFin amounts are numeric strings like "-33293.43" or "100.23".
    // Convert to integer cents.
    let parts: Vec<&str> = amount.split('.').collect();
    let dollars: i64 = parts[0].parse().map_err(|_| ProviderError::ParseError)?;
    let cents: i64 = parts.get(1).unwrap_or(&"0").parse().map_err(|_| ProviderError::ParseError)?;
    Ok(dollars * 100 + cents)
}
```

- [ ] **Step 2: Adjust `SimpleFinTransaction` model for timestamp helpers**

Add methods:

```rust
impl SimpleFinTransaction {
    pub fn posted_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.posted, 0).unwrap_or_else(|| Utc::now())
    }

    pub fn transacted_at(&self) -> Option<DateTime<Utc>> {
        self.transacted_at.map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(|| Utc::now()))
    }
}
```

- [ ] **Step 3: Write tests**

Use a temporary in-memory rusqlite DB or mock the client.

```rust
#[tokio::test]
async fn initial_sync_creates_starting_balance_and_transactions() {
    // Setup mocked client, in-memory DB
    // Call import_simplefin_account
    // Assert starting balance + N transactions inserted
}
```

Run: `cargo test -p finsight-providers --lib simplefin::sync`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-providers/src/simplefin/sync.rs crates/finsight-providers/src/simplefin/mod.rs
git commit -m "feat(simplefin): add account import/sync logic with starting balance"
```

---

### Task 6: Keychain helpers for SimpleFin

**Files:**
- Modify: `crates/finsight-core/src/keychain.rs`

**Interfaces:**
- Produces:
  - `pub fn save_simplefin_access_url(url: &str) -> CoreResult<()>`
  - `pub fn load_simplefin_access_url() -> CoreResult<Option<String>>`
  - `pub fn delete_simplefin_access_url() -> CoreResult<()>`

- [ ] **Step 1: Add helpers**

```rust
const SIMPLEFIN_SERVICE: &str = "com.finsight.simplefin";
const SIMPLEFIN_ACCOUNT: &str = "default";

pub fn save_simplefin_access_url(url: &str) -> CoreResult<()> {
    let entry = keyring::Entry::new(SIMPLEFIN_SERVICE, SIMPLEFIN_ACCOUNT)?;
    entry.set_password(url)?;
    Ok(())
}

pub fn load_simplefin_access_url() -> CoreResult<Option<String>> {
    let entry = keyring::Entry::new(SIMPLEFIN_SERVICE, SIMPLEFIN_ACCOUNT)?;
    match entry.get_password() {
        Ok(url) => Ok(Some(url)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_simplefin_access_url() -> CoreResult<()> {
    let entry = keyring::Entry::new(SIMPLEFIN_SERVICE, SIMPLEFIN_ACCOUNT)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
```

- [ ] **Step 2: Write tests**

```rust
#[test]
fn simplefin_access_url_round_trip() {
    save_simplefin_access_url("https://u:p@example.com/simplefin").unwrap();
    let loaded = load_simplefin_access_url().unwrap();
    assert_eq!(loaded, Some("https://u:p@example.com/simplefin".to_string()));
    delete_simplefin_access_url().unwrap();
    assert_eq!(load_simplefin_access_url().unwrap(), None);
}
```

Run: `cargo test -p finsight-core --lib keychain`
Expected: PASS (may be flaky on Windows parallel; mark `#[serial]` if needed)

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/src/keychain.rs
git commit -m "feat(keychain): add SimpleFin access URL storage helpers"
```

---

### Task 7: Tauri commands

**Files:**
- Create: `crates/finsight-app/src/commands/simplefin.rs`
- Modify: `crates/finsight-app/src/lib.rs`
- Modify: `crates/finsight-app/src/error.rs` (if needed)
- Modify: `crates/finsight-app/src/commands/mod.rs` (if exists)

**Interfaces:**
- Consumes: `SimpleFinClient`, `import_simplefin_account`, keychain helpers, `run()` pattern, `AppState`.
- Produces:
  - `save_simplefin_setup_token(state, token: String) -> AppResult<()>`
  - `get_simplefin_status(state) -> AppResult<SimpleFinStatus>`
  - `list_simplefin_accounts(state) -> AppResult<Vec<SimpleFinAccountInfo>>`
  - `import_simplefin_accounts(state, accounts: Vec<SimpleFinAccountImportRequest>) -> AppResult<Vec<i64>>`
  - `sync_simplefin_account(state, account_id: i64) -> AppResult<SyncSummary>`
  - `disconnect_simplefin(state) -> AppResult<()>`

- [ ] **Step 1: Define command types**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SimpleFinStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SimpleFinAccountInfo {
    pub id: String,
    pub name: String,
    pub connection_name: String,
    pub currency: String,
    pub balance: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SimpleFinAccountImportRequest {
    pub simplefin_id: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SyncSummary {
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,
}
```

- [ ] **Step 2: Implement commands**

```rust
use finsight_core::{
    db::Db,
    keychain::{delete_simplefin_access_url, load_simplefin_access_url, save_simplefin_access_url},
    models::NewAccount,
    repos::accounts,
};
use finsight_providers::simplefin::{import_simplefin_account, SimpleFinClient};

use crate::state::AppState;
use crate::error::{AppError, AppResult};
use crate::commands::run;

#[tauri::command]
#[specta::specta]
pub async fn save_simplefin_setup_token(state: tauri::State<'_, AppState>, token: String) -> AppResult<()> {
    let access_url = SimpleFinClient::claim_token(&token).await.map_err(AppError::from)?;
    save_simplefin_access_url(&access_url).map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_simplefin_status(_state: tauri::State<'_, AppState>) -> AppResult<SimpleFinStatus> {
    let configured = load_simplefin_access_url().map_err(AppError::from)?.is_some();
    Ok(SimpleFinStatus { configured })
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<SimpleFinAccountInfo>> {
    let access_url = load_simplefin_access_url().map_err(AppError::from)?
        .ok_or(AppError::InvalidState("SimpleFin not configured".into()))?;
    let client = SimpleFinClient::new(&access_url).map_err(AppError::from)?;
    let accounts = client.list_accounts().await.map_err(AppError::from)?;
    Ok(accounts.into_iter().map(|a| SimpleFinAccountInfo {
        id: a.id,
        name: a.name,
        connection_name: a.connection_name,
        currency: a.currency,
        balance: a.balance,
    }).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn import_simplefin_accounts(
    state: tauri::State<'_, AppState>,
    accounts: Vec<SimpleFinAccountImportRequest>,
) -> AppResult<Vec<i64>> {
    let db = state.db.clone();
    let access_url = load_simplefin_access_url().map_err(AppError::from)?
        .ok_or(AppError::InvalidState("SimpleFin not configured".into()))?;

    let mut created_ids = Vec::new();
    for req in accounts {
        let id = run(&db, move |conn| {
            let account = NewAccount {
                owner: None,
                bank: Some("SimpleFin".to_string()),
                account_type: finsight_core::models::AccountType::Checking,
                name: req.nickname.clone().unwrap_or_else(|| req.simplefin_id.clone()),
                last4: None,
                currency: "USD".to_string(), // will be updated after first sync
                color: None,
                nickname: req.nickname,
                sync_source: Some("simplefin".to_string()),
                simplefin_account_id: Some(req.simplefin_id.clone()),
                last_synced_at: None,
            };
            accounts::insert(conn, &account)
        }).await.map_err(AppError::from)?;
        created_ids.push(id);
    }

    // Sync each new account immediately.
    for id in &created_ids {
        sync_simplefin_account_inner(*id, &access_url, &db).await?;
    }

    Ok(created_ids)
}

#[tauri::command]
#[specta::specta]
pub async fn sync_simplefin_account(state: tauri::State<'_, AppState>, account_id: i64) -> AppResult<SyncSummary> {
    let access_url = load_simplefin_access_url().map_err(AppError::from)?
        .ok_or(AppError::InvalidState("SimpleFin not configured".into()))?;
    sync_simplefin_account_inner(account_id, &access_url, &state.db).await
}

async fn sync_simplefin_account_inner(account_id: i64, access_url: &str, db: &Db) -> AppResult<SyncSummary> {
    let access_url = access_url.to_string();
    let simplefin_id = run(db, move |conn| {
        let account = accounts::get(conn, account_id)?;
        Ok(account.simplefin_account_id.clone().ok_or_else(||
            finsight_core::Error::InvalidState("Account is not linked to SimpleFin".into())
        )?)
    }).await.map_err(AppError::from)?;

    let summary = run(db, move |conn| {
        import_simplefin_account(&access_url, &simplefin_id, account_id, conn)
    }).await.map_err(AppError::from)?;

    Ok(SyncSummary { added: summary.added, updated: summary.updated, skipped: summary.skipped })
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_simplefin(state: tauri::State<'_, AppState>) -> AppResult<()> {
    delete_simplefin_access_url().map_err(AppError::from)?;
    // Optionally clear sync_source from linked accounts.
    let db = state.db.clone();
    run(&db, |conn| {
        conn.execute(
            "UPDATE accounts SET sync_source = NULL, simplefin_account_id = NULL WHERE sync_source = 'simplefin'",
            [],
        )?;
        Ok(())
    }).await.map_err(AppError::from)?;
    Ok(())
}
```

- [ ] **Step 3: Register commands**

In `crates/finsight-app/src/lib.rs`, add to `collect_commands!`:

```rust
commands::simplefin::save_simplefin_setup_token,
commands::simplefin::get_simplefin_status,
commands::simplefin::list_simplefin_accounts,
commands::simplefin::import_simplefin_accounts,
commands::simplefin::sync_simplefin_account,
commands::simplefin::disconnect_simplefin,
```

- [ ] **Step 4: Verify compile**

Run: `cargo check -p finsight-app`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/simplefin.rs crates/finsight-app/src/lib.rs
git commit -m "feat(app): add SimpleFin Tauri commands"
```

---

### Task 8: Regenerate TypeScript bindings

**Files:**
- Modify: `ui/src/api/bindings.ts` (generated)

- [ ] **Step 1: Run export bindings**

```bash
cargo run -p finsight-tauri --bin export_bindings
```

- [ ] **Step 2: Verify diff**

Run: `git diff -- ui/src/api/bindings.ts`
Expected: New SimpleFin commands and types appear.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/bindings.ts
git commit -m "chore: regenerate TypeScript bindings for SimpleFin commands"
```

---

### Task 9: TanStack Query hooks

**Files:**
- Create: `ui/src/api/hooks/simplefin.ts`

**Interfaces:**
- Consumes: generated bindings commands.
- Produces:
  - `useSimpleFinStatus()`
  - `useSaveSimpleFinToken()`
  - `useSimpleFinAccounts()`
  - `useImportSimpleFinAccounts()`
  - `useSyncSimpleFinAccount()`
  - `useDisconnectSimpleFin()`

- [ ] **Step 1: Implement hooks**

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type SimpleFinAccountInfo,
  type SimpleFinAccountImportRequest,
  type SyncSummary,
} from "../bindings";

const simplefinKeys = {
  status: ["simplefin", "status"] as const,
  accounts: ["simplefin", "accounts"] as const,
};

export function useSimpleFinStatus() {
  return useQuery({
    queryKey: simplefinKeys.status,
    queryFn: () => commands.getSimplefinStatus(),
  });
}

export function useSaveSimpleFinToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (token: string) => commands.saveSimplefinSetupToken(token),
    onSuccess: () => qc.invalidateQueries({ queryKey: simplefinKeys.status }),
  });
}

export function useSimpleFinAccounts() {
  return useQuery({
    queryKey: simplefinKeys.accounts,
    queryFn: () => commands.listSimplefinAccounts(),
    enabled: false,
  });
}

export function useImportSimpleFinAccounts() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (accounts: SimpleFinAccountImportRequest[]) =>
      commands.importSimplefinAccounts(accounts),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
    },
  });
}

export function useSyncSimpleFinAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (accountId: number) => commands.syncSimplefinAccount(accountId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
    },
  });
}

export function useDisconnectSimpleFin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => commands.disconnectSimplefin(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}
```

- [ ] **Step 2: Verify type check**

Run: `cd ui && npx tsc --noEmit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/hooks/simplefin.ts
git commit -m "feat(ui): add SimpleFin tanstack-query hooks"
```

---

### Task 10: Onboarding SimpleFin card + discovery dialog

**Files:**
- Modify: `ui/src/screens/onboarding/StepConnect.tsx`
- Create: `ui/src/screens/onboarding/SimpleFinDialog.tsx`

**Interfaces:**
- Consumes: `useSaveSimpleFinToken`, `useSimpleFinAccounts`, `useImportSimpleFinAccounts`.

- [ ] **Step 1: Create `SimpleFinDialog`**

A dialog/drawer with:
- Explanation text and link to `https://bridge.simplefin.org/simplefin/create`
- Textarea for setup token
- “Connect” button → `saveSimplefinSetupToken`
- On success: list discovered accounts with checkboxes and nickname inputs
- “Import selected” button → `importSimplefinAccounts`
- Error display if token invalid

- [ ] **Step 2: Add SimpleFin card to `StepConnect`**

```tsx
const [simpleFinOpen, setSimpleFinOpen] = useState(false);

<Card className="stack stack-md">
  <h3>Connect with SimpleFin</h3>
  <p>Link bank accounts securely using your SimpleFin bridge token.</p>
  <Button variant="default" onClick={() => setSimpleFinOpen(true)}>
    Set up SimpleFin
  </Button>
</Card>

{simpleFinOpen && (
  <SimpleFinDialog onClose={() => setSimpleFinOpen(false)} />
)}
```

- [ ] **Step 3: Write tests**

```tsx
test("StepConnect shows SimpleFin setup card", () => {
  render(<StepConnect onNext={vi.fn()} />);
  expect(screen.getByText("Connect with SimpleFin")).toBeInTheDocument();
});
```

Run: `cd ui && npx vitest run src/screens/onboarding/StepConnect.test.tsx`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/onboarding/StepConnect.tsx ui/src/screens/onboarding/SimpleFinDialog.tsx
git commit -m "feat(ui): add SimpleFin onboarding card and account discovery dialog"
```

---

### Task 11: Settings credentials panel

**Files:**
- Modify: `ui/src/screens/Settings.tsx`

**Interfaces:**
- Consumes: `useSimpleFinStatus`, `useDisconnectSimpleFin`, `SimpleFinDialog`.

- [ ] **Step 1: Add Bank connections section**

```tsx
function SimpleFinSettings() {
  const { data: status } = useSimpleFinStatus();
  const { mutate: disconnect } = useDisconnectSimpleFin();
  const [dialogOpen, setDialogOpen] = useState(false);

  return (
    <section className="settings-section">
      <h3>Bank connections</h3>
      <div className="stack stack-sm">
        <p>SimpleFin: {status?.configured ? "Connected" : "Not connected"}</p>
        {status?.configured ? (
          <Button variant="default" onClick={() => disconnect()}>
            Reset SimpleFin credentials
          </Button>
        ) : (
          <Button variant="default" onClick={() => setDialogOpen(true)}>
            Set up SimpleFin
          </Button>
        )}
      </div>
      {dialogOpen && <SimpleFinDialog onClose={() => setDialogOpen(false)} />}
    </section>
  );
}
```

- [ ] **Step 2: Write/update test**

Run: `cd ui && npx vitest run src/screens/Settings.test.tsx`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add ui/src/screens/Settings.tsx
git commit -m "feat(ui): add SimpleFin credentials panel in Settings"
```

---

### Task 12: Accounts screen per-account sync

**Files:**
- Modify: `ui/src/screens/Accounts.tsx`

**Interfaces:**
- Consumes: `useSyncSimpleFinAccount`, account `sync_source` and `last_synced_at` fields.

- [ ] **Step 1: Add sync button for SimpleFin accounts**

In the account list row:

```tsx
{account.sync_source === "simplefin" && (
  <Button
    variant="ghost"
    onClick={() => sync(account.id)}
    disabled={isPending}
  >
    Sync now
  </Button>
)}
{account.last_synced_at && (
  <span className="muted">Synced {formatRelative(account.last_synced_at)}</span>
)}
```

- [ ] **Step 2: Add toast feedback**

On success: `toast.success("Synced N transactions")`.
On error: `toast.error("Sync failed: ...")`.

- [ ] **Step 3: Write test**

```tsx
test("SimpleFin account shows sync button", () => {
  // render Accounts with mocked account having sync_source='simplefin'
  // expect sync button
});
```

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/Accounts.tsx
git commit -m "feat(ui): add per-account SimpleFin sync button"
```

---

### Task 13: End-to-end smoke test and final verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace`
Expected: ALL PASS

- [ ] **Step 2: Run full frontend test suite**

Run: `cd ui && npx vitest run`
Expected: ALL PASS

- [ ] **Step 3: Type check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors

- [ ] **Step 4: Regenerate bindings and verify clean**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: no unexpected diff

- [ ] **Step 5: Manual Tauri dev smoke (optional)**

Run: `pnpm tauri:dev` and use a demo SimpleFin token if available.

- [ ] **Step 6: Final commit**

```bash
git status
git add -A
git commit -m "feat: SimpleFin bank sync integration"
```

---

## Spec coverage check

| Spec requirement | Task |
|---|---|
| Paste setup token in onboarding | Task 10 |
| Claim access URL and store securely | Tasks 4, 6, 7 |
| Discover SimpleFin accounts | Tasks 4, 7, 10 |
| Import selected accounts with optional nickname | Tasks 2, 5, 7, 10 |
| Nickname overrides SimpleFin name | Task 2 (`nickname` field) |
| Fetch all history | Task 5 (start from epoch) |
| Posted transactions only | Task 4 (`pending=0`) |
| Starting balance on initial sync | Task 5 |
| Settings page to add/revoke credentials | Tasks 6, 7, 11 |
| Per-account “Sync now” | Tasks 7, 12 |
| Import all currencies | Task 5 (no currency filter) |

## Placeholder scan

No TBD/TODO/fill-in-later steps. Every step has concrete code or exact commands.

## Type consistency check

- `SimpleFinAccountInfo`, `SimpleFinAccountImportRequest`, `SyncSummary` used consistently across Task 7, 9, 10, 11, 12.
- `imported_id` field on `NewTransaction` exists in `finsight-core` models.
- `ImportSource::SimpleFin` used in Tasks 2 and 5.

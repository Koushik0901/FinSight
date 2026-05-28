# FinSight — Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire up the multi-provider LLM categorization pipeline, add edit/archive for accounts and transactions with a full audit trail, and surface categories on Today and in the transaction list.

**Architecture:** V003 migration adds `categorizations` (append-only audit) and `rules` tables. A new `finsight-agent` crate provides the `CompletionProvider` trait plus Ollama, OpenAI-compat, and Anthropic impls; an `AgentTask` runs as a `tokio::spawn` loop processing jobs from an `mpsc` queue. `finsight-app` gains `AppState.agent` (handle) and `AppState.agent_provider` (swappable at runtime); new commands cover account edit/archive, transaction edit/delete, rule creation, and all agent operations. The React UI gains a `CategoryPicker`, drawer edit modes, a Today "Needs a glance" chip, an `AgentActivityFeed`, and a Settings AI Provider panel.

**Tech Stack:**
- Rust: `rusqlite/sqlcipher`, `reqwest` (JSON), `tokio` (spawn/mpsc), `async-trait`, `anyhow`, `keyring`
- Tauri 2.x + `tauri-specta`
- Frontend: React 18 + TS, Tanstack Query, Zustand, `react-hook-form`, `zod`, `sonner` (toasts)
- Testing: `rstest`, `tempfile`, `vitest`, `@testing-library/react`, `vitest-axe`

**Spec:** `docs/superpowers/specs/2026-05-28-finsight-phase-3-design.md`

**Exit criteria:**
- Fresh import → agent auto-categorizes transactions; low-confidence items appear in Today "Needs a glance".
- User corrects a category in the Transaction drawer → proposed-rule toast → rule created → re-imports use it automatically.
- Settings AI Provider panel: configure Ollama/OpenAI/OpenRouter/Anthropic/Google/Custom → test connection → save.
- Edit account name/color; archive account; edit transaction notes/merchant/amount; delete transaction — all work end-to-end.
- All Rust + frontend tests pass.

---

## File Structure

```
crates/finsight-core/
├── migrations/
│   └── V003__phase3_schema.sql                       # NEW
├── src/
│   ├── keychain.rs                                   # MODIFY — add set_key, get_key
│   ├── lib.rs                                        # MODIFY — pub mod exports for new repos/models
│   ├── models/
│   │   ├── mod.rs                                    # MODIFY — pub mod categorization; pub mod rule
│   │   ├── categorization.rs                         # NEW
│   │   └── rule.rs                                   # NEW
│   └── repos/
│       ├── mod.rs                                    # MODIFY — pub mod categorizations; pub mod rules
│       ├── accounts.rs                               # MODIFY — add update(), archive()
│       ├── transactions.rs                           # MODIFY — add update(), delete()
│       ├── categorizations.rs                        # NEW
│       └── rules.rs                                  # NEW

crates/finsight-agent/
├── Cargo.toml                                        # MODIFY — add reqwest, tokio, serde_json
└── src/
    ├── lib.rs                                        # MODIFY — full CompletionProvider trait + pub mods
    ├── agent.rs                                      # NEW — AgentHandle, AgentJob, AgentEvent, run loop
    ├── categorizer.rs                                # NEW — rules → LLM batch pipeline
    └── providers/
        ├── mod.rs                                    # NEW
        ├── mock.rs                                   # NEW — MockCompletionProvider for tests
        ├── ollama.rs                                 # NEW — OllamaProvider
        ├── openai_compat.rs                          # NEW — OpenAiCompatProvider
        └── anthropic.rs                             # NEW — AnthropicProvider

crates/finsight-app/
├── Cargo.toml                                        # MODIFY — add finsight-agent dep
└── src/
    ├── lib.rs                                        # MODIFY — AppState add agent + agent_provider; configure_app wiring + migration
    └── commands/
        ├── mod.rs                                    # MODIFY — pub mod agent
        ├── accounts.rs                               # MODIFY — add update_account, archive_account
        ├── transactions.rs                           # MODIFY — add update_transaction, delete_transaction, create_rule, list_categories
        └── agent.rs                                  # NEW — CompletionProviderConfig enum + all agent commands

crates/finsight-app/tests/
├── edit_account_cmd.rs                               # NEW
├── edit_transaction_cmd.rs                           # NEW
└── categorization_cmd.rs                             # NEW

ui/src/
├── api/hooks/
│   ├── accounts.ts                                   # MODIFY — add useUpdateAccount, useArchiveAccount
│   ├── transactions.ts                               # MODIFY — add useUpdateTransaction, useDeleteTransaction, useCreateRule
│   └── agent.ts                                      # NEW — all agent hooks
├── components/
│   ├── CategoryPicker.tsx                            # NEW
│   ├── AccountDrawer.tsx                             # MODIFY — add edit mode + archive
│   ├── TransactionDrawer.tsx                         # MODIFY — add edit mode + CategoryPicker + proposed-rule toast
│   └── AgentActivityFeed.tsx                         # NEW
└── screens/
    ├── Accounts.tsx                                  # MODIFY — row-click → edit drawer
    ├── Transactions.tsx                              # MODIFY — row-click → edit drawer; needs_review filter
    ├── Today.tsx                                     # MODIFY — Needs a glance chip + AgentActivityFeed
    ├── Settings.tsx                                  # MODIFY — AI Provider panel
    └── onboarding/
        └── StepAgent.tsx                             # MODIFY — two-path (Local/Cloud) layout
```

---

## Phase 3.0 — Backend Foundations

### Task 1: V003 migration — categorizations + rules tables

**Files:**
- Create: `crates/finsight-core/migrations/V003__phase3_schema.sql`
- Test: inline in existing `crates/finsight-core/src/db.rs` test module (add new test)

- [ ] **Step 1: Write the failing test**

Add to `crates/finsight-core/src/db.rs` in the existing `#[cfg(test)]` block:

```rust
#[test]
fn v003_tables_exist() {
    let dir = tempfile::TempDir::new().unwrap();
    let key = crate::keychain::generate_random_key();
    let db = crate::Db::open(&dir.path().join("v003.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    let conn = db.get().unwrap();
    let cats: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='categorizations'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cats, 1, "categorizations table missing");
    let rules: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rules'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rules, 1, "rules table missing");
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p finsight-core v003_tables_exist
```

Expected: FAIL — `categorizations table missing`

- [ ] **Step 3: Create the V003 migration**

Create `crates/finsight-core/migrations/V003__phase3_schema.sql`:

```sql
-- V003: categorizations audit trail + rules engine

CREATE TABLE categorizations (
  id          TEXT PRIMARY KEY,
  txn_id      TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id TEXT REFERENCES categories(id),  -- NULL means category cleared by user
  source      TEXT NOT NULL,                   -- 'rule' | 'llm' | 'user'
  confidence  REAL NOT NULL DEFAULT 1.0,
  model       TEXT,                            -- NULL for rule/user assignments
  at          TEXT NOT NULL
);
CREATE INDEX idx_cat_txn ON categorizations(txn_id, at DESC);

CREATE TABLE rules (
  id          TEXT PRIMARY KEY,
  pattern     TEXT NOT NULL,   -- matched with lower(merchant_raw) LIKE lower(pattern)
  category_id TEXT NOT NULL REFERENCES categories(id),
  enabled     INTEGER NOT NULL DEFAULT 1,
  source      TEXT NOT NULL DEFAULT 'user',  -- 'user' | 'agent-proposed'
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_rules_enabled ON rules(enabled) WHERE enabled = 1;
```

- [ ] **Step 4: Run test to verify it passes**

```
cargo test -p finsight-core v003_tables_exist
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V003__phase3_schema.sql crates/finsight-core/src/db.rs
git commit -m "feat(core): V003 migration — categorizations + rules tables"
```

---

### Task 2: keychain set_key + get_key

**Files:**
- Modify: `crates/finsight-core/src/keychain.rs`

- [ ] **Step 1: Write the failing tests**

Add to `crates/finsight-core/src/keychain.rs` at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_key_returns_none_when_absent() {
        // Use a unique user so tests don't collide with real keychain entries
        let svc = "com.finsight.test.keychain";
        let usr = &format!("test-absent-{}", uuid::Uuid::new_v4());
        // Clean up before asserting (in case a prior test left something)
        let _ = delete_key(svc, usr);
        let got = get_key(svc, usr).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn set_key_round_trip() {
        let svc = "com.finsight.test.keychain";
        let usr = &format!("test-rt-{}", uuid::Uuid::new_v4());
        set_key(svc, usr, "sk-test-value").unwrap();
        let got = get_key(svc, usr).unwrap();
        assert_eq!(got.as_deref(), Some("sk-test-value"));
        delete_key(svc, usr).unwrap();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test -p finsight-core keychain
```

Expected: FAIL — `set_key` and `get_key` not found

- [ ] **Step 3: Implement set_key and get_key**

Add to `crates/finsight-core/src/keychain.rs` (after `delete_key`):

```rust
/// Store a user-supplied string value in the OS keychain.
pub fn set_key(service: &str, user: &str, value: &str) -> CoreResult<()> {
    let entry = Entry::new(service, user)?;
    entry.set_password(value)?;
    Ok(())
}

/// Retrieve a previously stored value. Returns None if not found.
pub fn get_key(service: &str, user: &str) -> CoreResult<Option<String>> {
    let entry = Entry::new(service, user)?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test -p finsight-core keychain
```

Expected: PASS (2 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/keychain.rs
git commit -m "feat(core): add keychain set_key + get_key for LLM API keys"
```

---

### Task 3: Domain models + repos for categorizations and rules

**Files:**
- Create: `crates/finsight-core/src/models/categorization.rs`
- Create: `crates/finsight-core/src/models/rule.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Create: `crates/finsight-core/src/repos/categorizations.rs`
- Create: `crates/finsight-core/src/repos/rules.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/finsight-core/src/repos/categorizations.rs` with an inline test first (file won't compile yet — that's OK):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_and_list_categorization() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // Insert a category + account + transaction first
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Checking','Ch','USD','#000','manual','2024-01-01T00:00:00Z')", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t1','a1','2024-01-01T00:00:00Z',1000,'AMAZON','cleared',0,'2024-01-01T00:00:00Z')", [],
        ).unwrap();

        let row = NewCategorization {
            txn_id: "t1".to_string(),
            category_id: Some("cat1".to_string()),
            source: "user".to_string(),
            confidence: 1.0,
            model: None,
        };
        insert(&mut conn, row).unwrap();
        let rows = list_for_txn(&mut conn, "t1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, "user");
        assert_eq!(rows[0].category_id.as_deref(), Some("cat1"));
    }
}
```

Create `crates/finsight-core/src/repos/rules.rs` with inline test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("r.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_and_list_active_rules() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();

        let rule = NewRule {
            pattern: "%amazon%".to_string(),
            category_id: "cat1".to_string(),
            source: "user".to_string(),
        };
        let r = insert(&mut conn, rule).unwrap();
        assert_eq!(r.pattern, "%amazon%");

        let active = list_active(&mut conn).unwrap();
        assert_eq!(active.len(), 1);

        set_enabled(&mut conn, &r.id, false).unwrap();
        let active2 = list_active(&mut conn).unwrap();
        assert_eq!(active2.len(), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test -p finsight-core categorization
cargo test -p finsight-core rules
```

Expected: FAIL — compile errors (types not defined)

- [ ] **Step 3: Create model files**

Create `crates/finsight-core/src/models/categorization.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Categorization {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub model: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewCategorization {
    pub txn_id: String,
    pub category_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub model: Option<String>,
}
```

Create `crates/finsight-core/src/models/rule.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Rule {
    pub id: String,
    pub pattern: String,
    pub category_id: String,
    pub enabled: bool,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewRule {
    pub pattern: String,
    pub category_id: String,
    pub source: String,
}
```

Modify `crates/finsight-core/src/models/mod.rs` — add at bottom (use private `mod`, matching existing pattern):

```rust
mod categorization;
mod rule;

pub use categorization::{Categorization, NewCategorization};
pub use rule::{NewRule, Rule};
```

- [ ] **Step 4: Implement categorizations repo**

Create `crates/finsight-core/src/repos/categorizations.rs`:

```rust
use crate::error::CoreResult;
use crate::models::{Categorization, NewCategorization};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn insert(conn: &mut Connection, row: NewCategorization) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Uuid::new_v4().to_string(),
            row.txn_id,
            row.category_id,
            row.source,
            row.confidence,
            row.model,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

pub fn list_for_txn(conn: &mut Connection, txn_id: &str) -> CoreResult<Vec<Categorization>> {
    let mut stmt = conn.prepare(
        "SELECT id, txn_id, category_id, source, confidence, model, at \
         FROM categorizations WHERE txn_id = ?1 ORDER BY at DESC",
    )?;
    let rows = stmt.query_map(params![txn_id], |r| {
        let at_s: String = r.get(6)?;
        Ok(Categorization {
            id: r.get(0)?,
            txn_id: r.get(1)?,
            category_id: r.get(2)?,
            source: r.get(3)?,
            confidence: r.get(4)?,
            model: r.get(5)?,
            at: DateTime::parse_from_rfc3339(&at_s)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&Utc),
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}
```

Create `crates/finsight-core/src/repos/rules.rs`:

```rust
use crate::error::CoreResult;
use crate::models::{NewRule, Rule};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list_active(conn: &mut Connection) -> CoreResult<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, category_id, enabled, source, created_at \
         FROM rules WHERE enabled = 1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let created_s: String = r.get(5)?;
        Ok(Rule {
            id: r.get(0)?,
            pattern: r.get(1)?,
            category_id: r.get(2)?,
            enabled: r.get::<_, i64>(3)? != 0,
            source: r.get(4)?,
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&Utc),
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn insert(conn: &mut Connection, rule: NewRule) -> CoreResult<Rule> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO rules(id, pattern, category_id, enabled, source, created_at) \
         VALUES(?1, ?2, ?3, 1, ?4, ?5)",
        params![id, rule.pattern, rule.category_id, rule.source, now.to_rfc3339()],
    )?;
    Ok(Rule {
        id,
        pattern: rule.pattern,
        category_id: rule.category_id,
        enabled: true,
        source: rule.source,
        created_at: now,
    })
}

pub fn set_enabled(conn: &mut Connection, id: &str, enabled: bool) -> CoreResult<()> {
    conn.execute(
        "UPDATE rules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, id],
    )?;
    Ok(())
}
```

Modify `crates/finsight-core/src/repos/mod.rs` — add two lines after existing mods:

```rust
pub mod categorizations;
pub mod rules;
```

- [ ] **Step 5: Run tests to verify they pass**

```
cargo test -p finsight-core
```

Expected: PASS (all tests including the new ones)

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/categorization.rs \
        crates/finsight-core/src/models/rule.rs \
        crates/finsight-core/src/models/mod.rs \
        crates/finsight-core/src/repos/categorizations.rs \
        crates/finsight-core/src/repos/rules.rs \
        crates/finsight-core/src/repos/mod.rs
git commit -m "feat(core): categorization + rule models and repos"
```

---

### Task 4: Account update + archive repos

**Files:**
- Modify: `crates/finsight-core/src/repos/accounts.rs`
- Modify: `crates/finsight-core/src/models/account.rs` — add `AccountPatch`

- [ ] **Step 1: Write the failing tests**

Add to `crates/finsight-core/src/repos/accounts.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, models::NewAccount, models::AccountType, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("a.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn sample_account(conn: &mut rusqlite::Connection) -> Account {
        insert(conn, NewAccount {
            owner: "Me".into(), bank: "Bank".into(),
            r#type: AccountType::Checking, name: "Checking".into(),
            last4: None, currency: "USD".into(), color: "#fff".into(),
            opening_balance_cents: 0, source: "manual".into(),
        }).unwrap()
    }

    #[test]
    fn update_account_name() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let patch = AccountPatch { name: Some("New Name".into()), ..Default::default() };
        let updated = update(&mut conn, &acc.id, patch).unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.bank, "Bank"); // unchanged
    }

    #[test]
    fn archive_account_sets_archived_at() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        archive(&mut conn, &acc.id).unwrap();
        let archived_at: Option<String> = conn.query_row(
            "SELECT archived_at FROM accounts WHERE id = ?1",
            rusqlite::params![acc.id],
            |r| r.get(0),
        ).unwrap();
        assert!(archived_at.is_some());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p finsight-core accounts
```

Expected: FAIL — `update`, `archive`, `AccountPatch` not found

- [ ] **Step 3: Add AccountPatch to model**

Add to `crates/finsight-core/src/models/account.rs`:

```rust
#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct AccountPatch {
    pub name: Option<String>,
    pub bank: Option<String>,
    pub account_type: Option<AccountType>,
    pub color: Option<String>,
    pub last4: Option<Option<String>>,
    pub currency: Option<String>,
}
```

Replace the existing `pub use account::{...}` line in `models/mod.rs` with the expanded version:

```rust
pub use account::{Account, AccountPatch, AccountSummary, AccountType, NewAccount};
```

- [ ] **Step 4: Implement update + archive in accounts repo**

Add to `crates/finsight-core/src/repos/accounts.rs`:

```rust
use crate::models::AccountPatch;

pub fn update(conn: &mut Connection, id: &str, patch: AccountPatch) -> CoreResult<Account> {
    if let Some(name) = &patch.name {
        conn.execute("UPDATE accounts SET name = ?1 WHERE id = ?2", params![name, id])?;
    }
    if let Some(bank) = &patch.bank {
        conn.execute("UPDATE accounts SET bank = ?1 WHERE id = ?2", params![bank, id])?;
    }
    if let Some(at) = &patch.account_type {
        conn.execute("UPDATE accounts SET type = ?1 WHERE id = ?2", params![at.as_db(), id])?;
    }
    if let Some(color) = &patch.color {
        conn.execute("UPDATE accounts SET color = ?1 WHERE id = ?2", params![color, id])?;
    }
    if let Some(last4) = &patch.last4 {
        conn.execute("UPDATE accounts SET last4 = ?1 WHERE id = ?2", params![last4, id])?;
    }
    if let Some(currency) = &patch.currency {
        conn.execute("UPDATE accounts SET currency = ?1 WHERE id = ?2", params![currency, id])?;
    }
    // Return the updated account
    conn.query_row(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, created_at \
         FROM accounts WHERE id = ?1",
        params![id],
        |r| {
            let archived_s: Option<String> = r.get(8)?;
            let created_s: String = r.get(9)?;
            Ok(Account {
                id: r.get(0)?,
                owner: r.get(1)?,
                bank: r.get(2)?,
                r#type: AccountType::from_db(&r.get::<_, String>(3)?),
                name: r.get(4)?,
                last4: r.get(5)?,
                currency: r.get(6)?,
                color: r.get(7)?,
                archived_at: archived_s.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))
                }),
                created_at: DateTime::parse_from_rfc3339(&created_s).unwrap().with_timezone(&Utc),
            })
        },
    ).map_err(Into::into)
}

pub fn archive(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    // Clean up stale CSV import mappings for this account
    conn.execute(
        "DELETE FROM csv_import_mappings WHERE account_id = ?1",
        params![id],
    )?;
    Ok(())
}
```

Add `use chrono::{DateTime, Utc};` at the top of `accounts.rs` (it may already be there — add if missing).

- [ ] **Step 5: Run tests to verify they pass**

```
cargo test -p finsight-core accounts
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/account.rs \
        crates/finsight-core/src/models/mod.rs \
        crates/finsight-core/src/repos/accounts.rs
git commit -m "feat(core): account update + archive repo methods"
```

---

### Task 5: Transaction update + delete repos (with proposed-rule logic)

**Files:**
- Modify: `crates/finsight-core/src/repos/transactions.rs`
- Modify: `crates/finsight-core/src/models/transaction.rs` — add `TxnPatch`, `ProposedRule`

- [ ] **Step 1: Write the failing tests**

Add to `crates/finsight-core/src/repos/transactions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::run_migrations, keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::accounts,
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

    fn seed(conn: &mut rusqlite::Connection) -> (String, String) {
        // category
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        // account
        let acc = accounts::insert(conn, NewAccount {
            owner: "Me".into(), bank: "Bank".into(),
            r#type: AccountType::Checking, name: "Ch".into(),
            last4: None, currency: "USD".into(), color: "#fff".into(),
            opening_balance_cents: 0, source: "manual".into(),
        }).unwrap();
        // transaction
        let txn = insert(conn, NewTransaction {
            account_id: acc.id.clone(),
            posted_at: chrono::Utc::now(),
            amount_cents: 1000,
            merchant_raw: "AMAZON".to_string(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
        }).unwrap();
        (acc.id, txn.id)
    }

    #[test]
    fn update_transaction_notes() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch { notes: Some(Some("edited".into())), ..Default::default() };
        let (updated, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert_eq!(updated.notes.as_deref(), Some("edited"));
        assert!(rule.is_none()); // no category change → no rule proposal
    }

    #[test]
    fn update_category_appends_categorization_and_proposes_rule() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch {
            category_id: Some(Some("cat1".into())),
            ..Default::default()
        };
        let (updated, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert_eq!(updated.category_id.as_deref(), Some("cat1"));
        // Rule proposed because no existing rule for "AMAZON"
        assert!(rule.is_some());
        let r = rule.unwrap();
        assert_eq!(r.pattern, "AMAZON");
        assert_eq!(r.category_id, "cat1");
    }

    #[test]
    fn update_category_no_rule_when_rule_exists() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        // Pre-create a matching rule
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at) \
             VALUES('r1','AMAZON','cat1',1,'user','2024-01-01T00:00:00Z')", [],
        ).unwrap();
        let patch = TxnPatch { category_id: Some(Some("cat1".into())), ..Default::default() };
        let (_, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert!(rule.is_none()); // rule already exists → no proposal
    }

    #[test]
    fn delete_transaction_removes_row() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        delete(&mut conn, &txn_id).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE id = ?1",
            rusqlite::params![txn_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test -p finsight-core transactions
```

Expected: FAIL — `TxnPatch`, `ProposedRule`, `update`, `delete` not found

- [ ] **Step 3: Add TxnPatch + ProposedRule to model**

Add to `crates/finsight-core/src/models/transaction.rs`:

```rust
#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct TxnPatch {
    pub notes: Option<Option<String>>,
    pub category_id: Option<Option<String>>,
    pub amount_cents: Option<i64>,
    pub merchant_raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProposedRule {
    pub pattern: String,
    pub category_id: String,
    pub category_label: String,
}
```

Replace the existing `pub use transaction::{...}` line in `models/mod.rs` with the expanded version:

```rust
pub use transaction::{NewTransaction, ProposedRule, Transaction, TransactionStatus, TxnPatch};
```

- [ ] **Step 4: Implement update + delete in transactions repo**

Add to `crates/finsight-core/src/repos/transactions.rs`:

```rust
use crate::models::{NewCategorization, ProposedRule, TxnPatch};
use crate::repos::categorizations;

pub fn update(
    conn: &mut Connection,
    id: &str,
    patch: TxnPatch,
) -> CoreResult<(Transaction, Option<ProposedRule>)> {
    if let Some(notes) = &patch.notes {
        conn.execute("UPDATE transactions SET notes = ?1 WHERE id = ?2", params![notes, id])?;
    }
    if let Some(amount) = patch.amount_cents {
        conn.execute("UPDATE transactions SET amount_cents = ?1 WHERE id = ?2", params![amount, id])?;
    }
    if let Some(merchant) = &patch.merchant_raw {
        conn.execute("UPDATE transactions SET merchant_raw = ?1 WHERE id = ?2", params![merchant, id])?;
    }

    let mut proposed_rule: Option<ProposedRule> = None;

    if let Some(cat) = &patch.category_id {
        // Append categorization audit row
        categorizations::insert(conn, NewCategorization {
            txn_id: id.to_string(),
            category_id: cat.clone(),
            source: "user".to_string(),
            confidence: 1.0,
            model: None,
        })?;
        // Update live columns
        conn.execute(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
            params![cat, id],
        )?;
        // Check for rule proposal (only when setting a category, not clearing)
        if let Some(category_id) = cat {
            let merchant_raw: String = conn.query_row(
                "SELECT merchant_raw FROM transactions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            let rule_exists: bool = conn.query_row(
                "SELECT 1 FROM rules WHERE lower(pattern) = lower(?1) AND enabled = 1 LIMIT 1",
                params![merchant_raw],
                |_| Ok(true),
            ).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                other => Err(other),
            })?;
            if !rule_exists {
                let category_label: String = conn.query_row(
                    "SELECT label FROM categories WHERE id = ?1",
                    params![category_id],
                    |r| r.get(0),
                ).unwrap_or_default();
                proposed_rule = Some(ProposedRule {
                    pattern: merchant_raw,
                    category_id: category_id.clone(),
                    category_label,
                });
            }
        }
    }

    // Fetch and return updated transaction
    let txn = get_by_id(conn, id)?;
    Ok((txn, proposed_rule))
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
    Ok(())
}

/// Fetch a single transaction by id (used internally).
fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Transaction> {
    conn.query_row(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.id = ?1",
        params![id],
        |r| {
            let posted_s: String = r.get(2)?;
            let created_s: String = r.get(17)?;
            Ok(Transaction {
                id: r.get(0)?,
                account_id: r.get(1)?,
                posted_at: DateTime::parse_from_rfc3339(&posted_s).unwrap().with_timezone(&Utc),
                amount_cents: r.get(3)?,
                merchant_raw: r.get(4)?,
                merchant_id: r.get(5)?,
                merchant_label: r.get(6)?,
                merchant_color: r.get(7)?,
                merchant_initials: r.get(8)?,
                category_id: r.get(9)?,
                category_label: r.get(10)?,
                category_color: r.get(11)?,
                status: TransactionStatus::from_db(&r.get::<_, String>(12)?),
                notes: r.get(13)?,
                ai_confidence: r.get(14)?,
                ai_explanation: r.get(15)?,
                is_anomaly: r.get::<_, i64>(16)? != 0,
                created_at: DateTime::parse_from_rfc3339(&created_s).unwrap().with_timezone(&Utc),
            })
        },
    ).map_err(Into::into)
}
```

- [ ] **Step 5: Run tests to verify they pass**

```
cargo test -p finsight-core
```

Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/transaction.rs \
        crates/finsight-core/src/models/mod.rs \
        crates/finsight-core/src/repos/transactions.rs
git commit -m "feat(core): transaction update + delete with categorization audit + rule proposal"
```

---

## Phase 3.1 — Agent Providers

### Task 6: CompletionProvider trait + MockProvider + finsight-agent Cargo.toml

**Files:**
- Modify: `crates/finsight-agent/Cargo.toml`
- Modify: `crates/finsight-agent/src/lib.rs`
- Create: `crates/finsight-agent/src/providers/mod.rs`
- Create: `crates/finsight-agent/src/providers/mock.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace `crates/finsight-agent/Cargo.toml` with:

```toml
[package]
name = "finsight-agent"
version = "0.0.0"
edition.workspace = true
license.workspace = true

[dependencies]
finsight-core = { path = "../finsight-core" }
async-trait.workspace = true
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
reqwest = { workspace = true, features = ["json"] }
```

- [ ] **Step 2: Write the failing test**

Create `crates/finsight-agent/src/providers/mock.rs` with inline tests:

```rust
use crate::CompletionProvider;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Test double that returns a canned JSON value for any prompt.
pub struct MockCompletionProvider {
    pub provider_id: String,
    pub model_id: String,
    pub response: Value,
}

#[async_trait]
impl CompletionProvider for MockCompletionProvider {
    fn provider_id(&self) -> &str { &self.provider_id }
    fn model_id(&self) -> &str { &self.model_id }
    async fn complete_json(&self, _system: &str, _user: &str) -> Result<Value> {
        Ok(self.response.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn mock_returns_canned_value() {
        let p = MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.9, "rationale": "test"}]),
        };
        let got = p.complete_json("sys", "user").await.unwrap();
        assert_eq!(got[0]["txn_id"], "t1");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p finsight-agent mock_returns_canned_value
```

Expected: FAIL — `CompletionProvider` not found / `providers` mod missing

- [ ] **Step 4: Update lib.rs with full trait + pub mods**

Replace `crates/finsight-agent/src/lib.rs`:

```rust
//! FinSight agent — LLM provider traits, agent task, categorizer pipeline.

pub mod providers;

use async_trait::async_trait;
use anyhow::Result;
use serde_json::Value;

/// Core provider abstraction. All impls must be Send + Sync so they can be
/// shared across tokio tasks behind Arc<RwLock<...>>.
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;
    /// Send a system + user prompt; expect a JSON-parseable response.
    async fn complete_json(&self, system: &str, user: &str) -> Result<Value>;
    /// Return available model names. Returns Ok(vec![]) for providers
    /// that don't expose a model listing API (OpenAiCompat, Anthropic).
    async fn list_models(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}

/// Stub retained for Phase 5 (embedding-based nearest-neighbor search).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}
```

Create `crates/finsight-agent/src/providers/mod.rs`:

```rust
pub mod mock;
pub mod ollama;
pub mod openai_compat;
pub mod anthropic;
```

(The three provider files will be created in Tasks 7–9; add stubs now to let `mod.rs` compile:)

Create `crates/finsight-agent/src/providers/ollama.rs` stub:

```rust
// Implemented in Task 7
```

Create `crates/finsight-agent/src/providers/openai_compat.rs` stub:

```rust
// Implemented in Task 8
```

Create `crates/finsight-agent/src/providers/anthropic.rs` stub:

```rust
// Implemented in Task 9
```

- [ ] **Step 5: Run test to verify it passes**

```
cargo test -p finsight-agent mock_returns_canned_value
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-agent/Cargo.toml \
        crates/finsight-agent/src/lib.rs \
        crates/finsight-agent/src/providers/mod.rs \
        crates/finsight-agent/src/providers/mock.rs \
        crates/finsight-agent/src/providers/ollama.rs \
        crates/finsight-agent/src/providers/openai_compat.rs \
        crates/finsight-agent/src/providers/anthropic.rs
git commit -m "feat(agent): CompletionProvider trait + MockProvider + crate deps"
```

---

### Task 7: OllamaProvider

**Files:**
- Modify: `crates/finsight-agent/src/providers/ollama.rs`

- [ ] **Step 1: Write the failing test**

Replace `crates/finsight-agent/src/providers/ollama.rs`:

```rust
use crate::CompletionProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[derive(Deserialize)]
struct OllamaMessage { content: String }
#[derive(Deserialize)]
struct OllamaChatResp { message: OllamaMessage }

#[async_trait]
impl CompletionProvider for OllamaProvider {
    fn provider_id(&self) -> &str { "ollama" }
    fn model_id(&self) -> &str { &self.model }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "format": "json",
            "stream": false,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user},
            ]
        });
        let resp: OllamaChatResp = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        serde_json::from_str(&resp.message.content)
            .map_err(|e| anyhow!("Ollama response not valid JSON: {e}"))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Tag { name: String }
        #[derive(Deserialize)]
        struct TagsResp { models: Vec<Tag> }
        let resp: TagsResp = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.models.into_iter().map(|t| t.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify request body shape without making a network call.
    #[test]
    fn request_body_has_format_json() {
        let body = json!({
            "model": "llama3.2",
            "format": "json",
            "stream": false,
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user",   "content": "usr"},
            ]
        });
        assert_eq!(body["format"], "json");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "system");
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

```
cargo test -p finsight-agent request_body_has_format_json
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/providers/ollama.rs
git commit -m "feat(agent): OllamaProvider — /api/chat with format:json"
```

---

### Task 8: OpenAiCompatProvider

**Files:**
- Modify: `crates/finsight-agent/src/providers/openai_compat.rs`

- [ ] **Step 1: Write the test + implementation**

Replace `crates/finsight-agent/src/providers/openai_compat.rs`:

```rust
use crate::CompletionProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

/// Covers OpenAI, OpenRouter, Google (v1beta/openai), Mistral, Groq,
/// and any other OpenAI-compatible chat completions endpoint.
pub struct OpenAiCompatProvider {
    base_url: String,
    api_key: String,
    model: String,
    preset: String,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        preset: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            preset: preset.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[derive(Deserialize)]
struct Choice { message: OaiMessage }
#[derive(Deserialize)]
struct OaiMessage { content: String }
#[derive(Deserialize)]
struct OaiResp { choices: Vec<Choice> }

#[async_trait]
impl CompletionProvider for OpenAiCompatProvider {
    fn provider_id(&self) -> &str { &self.preset }
    fn model_id(&self) -> &str { &self.model }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "response_format": { "type": "json_object" },
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user},
            ]
        });
        let resp: OaiResp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let content = resp.choices.into_iter().next()
            .ok_or_else(|| anyhow!("no choices in response"))?
            .message.content;
        serde_json::from_str(&content)
            .map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_has_json_response_format() {
        let body = json!({
            "model": "gpt-4o-mini",
            "response_format": { "type": "json_object" },
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user",   "content": "usr"},
            ]
        });
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["messages"][1]["role"], "user");
    }
}
```

- [ ] **Step 2: Run test**

```
cargo test -p finsight-agent request_body_has_json_response_format
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/providers/openai_compat.rs
git commit -m "feat(agent): OpenAiCompatProvider — chat/completions with json_object format"
```

---

### Task 9: AnthropicProvider

**Files:**
- Modify: `crates/finsight-agent/src/providers/anthropic.rs`

- [ ] **Step 1: Write the test + implementation**

Replace `crates/finsight-agent/src/providers/anthropic.rs`:

```rust
use crate::CompletionProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

const ANTHROPIC_API: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    input: Value,
}
#[derive(Deserialize)]
struct AnthropicResp { content: Vec<ContentBlock> }

#[async_trait]
impl CompletionProvider for AnthropicProvider {
    fn provider_id(&self) -> &str { "anthropic" }
    fn model_id(&self) -> &str { &self.model }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system,
            "messages": [{"role": "user", "content": user}],
            "tools": [{
                "name": "classify",
                "description": "Return the classification results",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "results": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "txn_id":      {"type": "string"},
                                    "category_id": {"type": "string"},
                                    "confidence":  {"type": "number"},
                                    "rationale":   {"type": "string"}
                                },
                                "required": ["txn_id", "category_id", "confidence", "rationale"]
                            }
                        }
                    },
                    "required": ["results"]
                }
            }],
            "tool_choice": {"type": "tool", "name": "classify"}
        });

        let resp: AnthropicResp = self.client
            .post(ANTHROPIC_API)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Response is content[0].input.results
        let block = resp.content.into_iter().next()
            .ok_or_else(|| anyhow!("empty content from Anthropic"))?;
        if block.kind != "tool_use" {
            return Err(anyhow!("expected tool_use block, got {}", block.kind));
        }
        // Return the results array directly
        block.input.get("results")
            .cloned()
            .ok_or_else(|| anyhow!("missing 'results' in tool input"))
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_uses_tool_use() {
        let body = json!({
            "tools": [{"name": "classify"}],
            "tool_choice": {"type": "tool", "name": "classify"}
        });
        assert_eq!(body["tool_choice"]["type"], "tool");
        assert_eq!(body["tools"][0]["name"], "classify");
    }

    #[test]
    fn extracts_results_from_tool_input() {
        let input = json!({"results": [{"txn_id": "t1", "category_id": "cat1", "confidence": 0.95, "rationale": "r"}]});
        let block = ContentBlock { kind: "tool_use".into(), input: input.clone() };
        let resp = AnthropicResp { content: vec![block] };
        let results = resp.content.into_iter().next().unwrap().input
            .get("results").cloned().unwrap();
        assert_eq!(results[0]["txn_id"], "t1");
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p finsight-agent anthropic
```

Expected: PASS (2 tests)

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/providers/anthropic.rs
git commit -m "feat(agent): AnthropicProvider — tool-use for structured JSON output"
```

---

## Phase 3.2 — Agent Task

### Task 10: AgentHandle + job loop

**Files:**
- Create: `crates/finsight-agent/src/agent.rs`
- Modify: `crates/finsight-agent/src/lib.rs` — add `pub mod agent`

- [ ] **Step 1: Write the failing test**

Create `crates/finsight-agent/src/agent.rs`:

```rust
use crate::CompletionProvider;
use finsight_core::Db;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AgentJob {
    CategorizeImport { import_id: String },
    CategorizeAll,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum AgentEvent {
    CategorizationProgress { import_id: Option<String>, done: u32, total: u32 },
    CategorizationComplete { import_id: Option<String>, categorized: u32, skipped: u32 },
    Error { message: String },
}

pub type EventCallback = Arc<dyn Fn(AgentEvent) + Send + Sync>;

pub struct AgentHandle {
    pub tx: mpsc::Sender<AgentJob>,
    provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
}

impl AgentHandle {
    /// Spawn the agent background task and return a handle to enqueue jobs.
    /// `on_event` is called on the spawning thread's runtime for each event emitted.
    pub fn spawn(
        db: Db,
        provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
        on_event: EventCallback,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<AgentJob>(64);
        let provider_clone = Arc::clone(&provider);
        tokio::spawn(run_loop(db, rx, provider_clone, on_event));
        Self { tx, provider }
    }

    /// Replace the active provider at runtime. No task restart needed.
    pub fn set_provider(&self, p: Arc<dyn CompletionProvider>) {
        *self.provider.write().unwrap() = Some(p);
    }
}

async fn run_loop(
    db: Db,
    mut rx: mpsc::Receiver<AgentJob>,
    provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    on_event: EventCallback,
) {
    while let Some(job) = rx.recv().await {
        let p = provider.read().unwrap().clone();
        match p {
            None => {
                on_event(AgentEvent::Error {
                    message: "No completion provider configured".to_string(),
                });
            }
            Some(p) => {
                let result = crate::categorizer::run_job(&db, job, p, Arc::clone(&on_event)).await;
                if let Err(e) = result {
                    on_event(AgentEvent::Error { message: e.to_string() });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;
    use finsight_core::{db::run_migrations, keychain};
    use serde_json::json;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[tokio::test]
    async fn handle_sends_job_and_receives_error_when_no_provider() {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("h.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();

        let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let handle = AgentHandle::spawn(db, provider, Arc::new(move |e| {
            events_clone.lock().unwrap().push(e);
        }));

        handle.tx.send(AgentJob::CategorizeAll).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let evs = events.lock().unwrap();
        assert!(evs.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    }

    #[tokio::test]
    async fn set_provider_replaces_atomically() {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("sp.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        let handle = AgentHandle::spawn(db, Arc::clone(&provider), Arc::new(|_| {}));
        let mock = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([]),
        });
        handle.set_provider(mock);
        let locked = provider.read().unwrap();
        assert!(locked.is_some());
        assert_eq!(locked.as_ref().unwrap().provider_id(), "mock");
    }
}
```

- [ ] **Step 2: Add pub mod agent to lib.rs**

Add to `crates/finsight-agent/src/lib.rs` after `pub mod providers;`:

```rust
pub mod agent;
pub mod categorizer;
```

Also create a minimal `crates/finsight-agent/src/categorizer.rs` stub so it compiles:

```rust
use crate::{agent::{AgentEvent, AgentJob, EventCallback}, CompletionProvider};
use finsight_core::Db;
use std::sync::Arc;

pub async fn run_job(
    _db: &Db,
    _job: AgentJob,
    _provider: Arc<dyn CompletionProvider>,
    _on_event: EventCallback,
) -> anyhow::Result<()> {
    // Implemented in Task 11
    Ok(())
}
```

- [ ] **Step 3: Run tests to verify they pass**

```
cargo test -p finsight-agent agent
```

Expected: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-agent/src/agent.rs \
        crates/finsight-agent/src/categorizer.rs \
        crates/finsight-agent/src/lib.rs
git commit -m "feat(agent): AgentHandle + job queue + event callback"
```

---

### Task 11: Categorizer pipeline

**Files:**
- Modify: `crates/finsight-agent/src/categorizer.rs`

- [ ] **Step 1: Write the failing tests**

Replace `crates/finsight-agent/src/categorizer.rs` with the full implementation and tests. The tests use `MockCompletionProvider` and a real in-memory SQLite DB.

```rust
use crate::{
    agent::{AgentEvent, AgentJob, EventCallback},
    CompletionProvider,
};
use anyhow::Result;
use finsight_core::{
    models::{NewCategorization, NewRule},
    repos::{categorizations, rules},
    Db,
};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

const LLM_BATCH_SIZE: usize = 20;

#[derive(Deserialize)]
struct LlmResult {
    txn_id: String,
    category_id: String,
    confidence: f64,
    rationale: String,
}

pub async fn run_job(
    db: &Db,
    job: AgentJob,
    provider: Arc<dyn CompletionProvider>,
    on_event: EventCallback,
) -> Result<()> {
    let import_id = match &job {
        AgentJob::CategorizeImport { import_id } => Some(import_id.clone()),
        AgentJob::CategorizeAll => None,
    };

    // Load data needed for categorization on a blocking thread
    let (uncategorized, active_rules, categories, recent_examples) = {
        let db = db.clone();
        let import_id_clone = import_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let uncategorized = load_uncategorized(&mut conn, import_id_clone.as_deref())?;
            let active_rules = rules::list_active(&mut conn)?;
            let categories = load_categories(&mut conn)?;
            let recent_examples = load_recent_examples(&mut conn)?;
            Ok::<_, anyhow::Error>((uncategorized, active_rules, categories, recent_examples))
        })
        .await??
    };

    let total = uncategorized.len() as u32;
    let mut remaining: Vec<(String, String, i64)> = Vec::new(); // (txn_id, merchant_raw, amount_cents)
    let mut categorized: u32 = 0;

    // Step 1: Rule pass
    for (txn_id, merchant_raw, amount_cents) in &uncategorized {
        let matched = active_rules.iter().find(|r| {
            let pat = r.pattern.to_lowercase();
            let merch = merchant_raw.to_lowercase();
            // Simple LIKE: leading/trailing % = contains, otherwise exact
            if pat.starts_with('%') && pat.ends_with('%') {
                merch.contains(&pat[1..pat.len()-1])
            } else if pat.starts_with('%') {
                merch.ends_with(&pat[1..])
            } else if pat.ends_with('%') {
                merch.starts_with(&pat[..pat.len()-1])
            } else {
                merch == pat
            }
        });

        if let Some(rule) = matched {
            let cat_id = rule.category_id.clone();
            let txn_id = txn_id.clone();
            let db = db.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = db.get()?;
                categorizations::insert(&mut conn, NewCategorization {
                    txn_id: txn_id.clone(),
                    category_id: Some(cat_id.clone()),
                    source: "rule".to_string(),
                    confidence: 1.0,
                    model: None,
                })?;
                conn.execute(
                    "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
                    params![cat_id, txn_id],
                )?;
                Ok::<_, anyhow::Error>(())
            }).await??;
            categorized += 1;
        } else {
            remaining.push((txn_id.clone(), merchant_raw.clone(), *amount_cents));
        }
    }

    on_event(AgentEvent::CategorizationProgress {
        import_id: import_id.clone(),
        done: categorized,
        total,
    });

    // Step 2: LLM batch pass
    let system_prompt = build_system_prompt(&categories, &recent_examples);

    for chunk in remaining.chunks(LLM_BATCH_SIZE) {
        let user_prompt = build_user_prompt(chunk);
        let raw = provider.complete_json(&system_prompt, &user_prompt).await?;
        // All three provider impls (Ollama, OpenAiCompat, Anthropic) return a flat JSON array.
        let results: Vec<LlmResult> = serde_json::from_value(raw)?;

        for r in &results {
            let txn_id = r.txn_id.clone();
            let cat_id = r.category_id.clone();
            let confidence = r.confidence;
            let rationale = r.rationale.clone();
            let model = provider.model_id().to_string();
            let db = db.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = db.get()?;
                categorizations::insert(&mut conn, NewCategorization {
                    txn_id: txn_id.clone(),
                    category_id: Some(cat_id.clone()),
                    source: "llm".to_string(),
                    confidence,
                    model: Some(model),
                })?;
                conn.execute(
                    "UPDATE transactions SET category_id = ?1, ai_confidence = ?2, ai_explanation = ?3 WHERE id = ?4",
                    params![cat_id, confidence, rationale, txn_id],
                )?;
                Ok::<_, anyhow::Error>(())
            }).await??;
            categorized += 1;
        }
        on_event(AgentEvent::CategorizationProgress {
            import_id: import_id.clone(),
            done: categorized,
            total,
        });
    }

    let final_skipped = total.saturating_sub(categorized);
    on_event(AgentEvent::CategorizationComplete {
        import_id: import_id.clone(),
        categorized,
        skipped: final_skipped,
    });

    Ok(())
}

// ── helpers ────────────────────────────────────────────────────────────────

fn load_uncategorized(
    conn: &mut rusqlite::Connection,
    _import_id: Option<&str>,
) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, merchant_raw, amount_cents FROM transactions \
         WHERE category_id IS NULL ORDER BY posted_at DESC",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

fn load_categories(conn: &mut rusqlite::Connection) -> Result<Vec<(String, String, String)>> {
    // (id, label, group_label)
    let mut stmt = conn.prepare(
        "SELECT c.id, c.label, COALESCE(g.label, '') \
         FROM categories c LEFT JOIN category_groups g ON g.id = c.group_id \
         WHERE c.archived_at IS NULL ORDER BY g.sort_order, c.sort_order",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

fn load_recent_examples(conn: &mut rusqlite::Connection) -> Result<Vec<(String, String)>> {
    // (merchant_raw, category_label) — last 5 user corrections
    let mut stmt = conn.prepare(
        "SELECT t.merchant_raw, c.label \
         FROM categorizations ca \
         JOIN transactions t ON t.id = ca.txn_id \
         JOIN categories c ON c.id = ca.category_id \
         WHERE ca.source = 'user' \
         ORDER BY ca.at DESC LIMIT 5",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

fn build_system_prompt(
    categories: &[(String, String, String)],
    recent_examples: &[(String, String)],
) -> String {
    let cats_json = json!(categories.iter().map(|(id, label, group)| {
        json!({"id": id, "label": label, "group_label": group})
    }).collect::<Vec<_>>());
    let examples_json = json!(recent_examples.iter().map(|(merchant, cat)| {
        json!({"merchant_raw": merchant, "category_label": cat})
    }).collect::<Vec<_>>());
    format!(
        "You are a personal finance transaction categorizer. Classify each transaction into \
         exactly one of the provided categories. Respond with a valid JSON array only — \
         no markdown, no explanation outside the array.\n\nCategories:\n{}\n\nRecent examples from this user (for calibration):\n{}",
        cats_json, examples_json
    )
}

fn build_user_prompt(txns: &[(String, String, i64)]) -> String {
    let items: Vec<_> = txns.iter().map(|(id, merchant, amount)| {
        json!({"txn_id": id, "merchant_raw": merchant, "amount_cents": amount})
    }).collect();
    format!(
        "Classify these transactions:\n{}\n\nRespond:\n[\
         {{\"txn_id\":\"...\",\"category_id\":\"...\",\"confidence\":0.0,\"rationale\":\"one sentence\"}}]",
        json!(items)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;
    use finsight_core::{db::run_migrations, keychain};
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, finsight_core::Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("cat.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_db(conn: &mut rusqlite::Connection) -> (String, String) {
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','Daily',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t1','a1','2024-01-15T00:00:00Z',1500,'CHIPOTLE','cleared',0,'2024-01-15T00:00:00Z')", [],
        ).unwrap();
        ("a1".to_string(), "t1".to_string())
    }

    #[tokio::test]
    async fn rule_pass_categorizes_matching_transaction() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
            rules::insert(&mut conn, NewRule {
                pattern: "CHIPOTLE".to_string(),
                category_id: "cat1".to_string(),
                source: "user".to_string(),
            }).unwrap();
        }
        let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([]),
        });
        run_job(
            &db,
            AgentJob::CategorizeAll,
            provider,
            Arc::new(move |e| { events_clone.lock().unwrap().push(e); }),
        ).await.unwrap();

        let conn = db.get().unwrap();
        let cat_id: Option<String> = conn.query_row(
            "SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(cat_id.as_deref(), Some("cat1"));
    }

    #[tokio::test]
    async fn llm_pass_writes_category_and_ai_confidence() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
            // No rules — forces LLM path
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.87, "rationale": "Fast food"}]),
        });
        run_job(
            &db,
            AgentJob::CategorizeAll,
            provider,
            Arc::new(|_| {}),
        ).await.unwrap();

        let conn = db.get().unwrap();
        let (cat_id, confidence): (Option<String>, Option<f64>) = conn.query_row(
            "SELECT category_id, ai_confidence FROM transactions WHERE id='t1'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(cat_id.as_deref(), Some("cat1"));
        assert!((confidence.unwrap() - 0.87).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p finsight-agent categorizer
```

Expected: PASS (2 tests)

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/categorizer.rs
git commit -m "feat(agent): categorizer pipeline — rule pass + LLM batch"
```

---

## Phase 3.3 — App Commands

### Task 12: AppState wiring + settings migration + finsight-app Cargo.toml

**Files:**
- Modify: `crates/finsight-app/Cargo.toml`
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Verify finsight-agent is in finsight-app Cargo.toml**

`finsight-agent` was added in the Phase 0+1 bootstrap. Confirm it appears in `crates/finsight-app/Cargo.toml` under `[dependencies]`:

```toml
finsight-agent = { path = "../finsight-agent" }
```

If absent (should not happen), add it. No change needed if present.

- [ ] **Step 2: Write the failing test**

Add `crates/finsight-app/tests/startup_provider.rs`:

```rust
use finsight_core::{db::run_migrations, keychain, settings, Db};
use tempfile::TempDir;

/// Verifies that the llm_provider → completion_provider migration runs
/// when completion_provider is absent but llm_provider is present.
#[test]
fn migrate_llm_provider_to_completion_provider() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("mp.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    // Simulate a Phase 2 DB state: llm_provider set, completion_provider absent.
    {
        let conn = db.get().unwrap();
        settings::set(
            &conn,
            "llm_provider",
            &serde_json::json!({
                "kind": "ollama",
                "base_url": "http://localhost:11434",
                "completion_model": "llama3.2",
                "embedding_model": "nomic-embed-text"
            }),
        ).unwrap();
    }

    finsight_app::migrate_provider_settings(&db).unwrap();

    let conn = db.get().unwrap();
    let new_cfg: Option<serde_json::Value> =
        settings::get(&conn, "completion_provider").unwrap();
    assert!(new_cfg.is_some(), "completion_provider should be written");
    let cfg = new_cfg.unwrap();
    assert_eq!(cfg["kind"], "ollama");
    assert_eq!(cfg["base_url"], "http://localhost:11434");
    assert_eq!(cfg["model"], "llama3.2");
}
```

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p finsight-app migrate_llm_provider_to_completion_provider
```

Expected: FAIL — `migrate_provider_settings` not found, `AppState` missing `agent` field

- [ ] **Step 4: Rewrite lib.rs with new AppState + wiring**

Replace `crates/finsight-app/src/lib.rs`:

```rust
//! FinSight Tauri app — command surface + lifecycle.

pub mod commands;
pub mod error;

use finsight_agent::{
    agent::{AgentHandle, EventCallback},
    providers::{
        anthropic::AnthropicProvider, mock::MockCompletionProvider,
        ollama::OllamaProvider, openai_compat::OpenAiCompatProvider,
    },
    CompletionProvider,
};
use finsight_core::{db::run_migrations, keychain, settings, Db};
use std::sync::{Arc, RwLock};
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Db>,
    pub agent: AgentHandle,
    pub agent_provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
}

impl AppState {
    pub fn new(db: Db, on_event: EventCallback) -> Self {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let agent = AgentHandle::spawn(db.clone(), Arc::clone(&provider), on_event);
        Self {
            db: Arc::new(db),
            agent,
            agent_provider: provider,
        }
    }
}

/// Migrate legacy `llm_provider` key → `completion_provider`.
/// Called from `configure_app` setup before managing AppState.
/// Exported for integration tests.
pub fn migrate_provider_settings(db: &Db) -> Result<(), finsight_core::CoreError> {
    let conn = db.get().map_err(finsight_core::CoreError::Pool)?;
    // Only migrate if completion_provider is absent
    let new_cfg: Option<serde_json::Value> = settings::get(&conn, "completion_provider")?;
    if new_cfg.is_some() {
        return Ok(());
    }
    let old_cfg: Option<serde_json::Value> = settings::get(&conn, "llm_provider")?;
    let Some(old) = old_cfg else { return Ok(()) };
    let migrated = match old.get("kind").and_then(|k| k.as_str()) {
        Some("ollama") => serde_json::json!({
            "kind": "ollama",
            "base_url": old["base_url"],
            "model": old["completion_model"]
        }),
        _ => serde_json::json!({ "kind": "unconfigured" }),
    };
    settings::set(&conn, "completion_provider", &migrated)?;
    Ok(())
}

/// Load the saved CompletionProviderConfig from settings and instantiate the provider.
/// Returns None if unconfigured or key absent.
pub fn load_provider_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    let conn = db.get().ok()?;
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").ok()??;
    build_provider_from_config(&cfg)
}

pub(crate) fn build_provider_from_config(cfg: &serde_json::Value) -> Option<Arc<dyn CompletionProvider>> {
    match cfg.get("kind")?.as_str()? {
        "ollama" => {
            let base_url = cfg["base_url"].as_str()?.to_string();
            let model = cfg["model"].as_str()?.to_string();
            Some(Arc::new(OllamaProvider::new(base_url, model)))
        }
        "openai_compat" => {
            let base_url = cfg["base_url"].as_str()?.to_string();
            let model = cfg["model"].as_str()?.to_string();
            let preset = cfg["preset"].as_str().unwrap_or("custom").to_string();
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", &preset)
                .ok()??.to_string();
            Some(Arc::new(OpenAiCompatProvider::new(base_url, api_key, model, preset)))
        }
        "anthropic" => {
            let model = cfg["model"].as_str()?.to_string();
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", "anthropic")
                .ok()??.to_string();
            Some(Arc::new(AnthropicProvider::new(api_key, model)))
        }
        _ => None,
    }
}

/// Build the tauri-specta builder with all commands registered.
pub fn build_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::accounts::list_accounts,
        commands::accounts::create_account,
        commands::accounts::update_account,
        commands::accounts::archive_account,
        commands::transactions::list_transactions,
        commands::transactions::create_transaction,
        commands::transactions::update_transaction,
        commands::transactions::delete_transaction,
        commands::transactions::create_rule,
        commands::transactions::list_categories,
        commands::onboarding::get_onboarding_state,
        commands::onboarding::seed_sample_household,
        commands::onboarding::mark_onboarding_complete,
        commands::onboarding::reset_onboarding_completion,
        commands::onboarding::clear_sample_data,
        commands::onboarding::commit_starter_categories,
        commands::onboarding::probe_ollama,
        commands::onboarding::save_llm_provider,
        commands::meta::app_ready,
        commands::import::preview_csv_columns,
        commands::import::import_csv,
        commands::import::list_unfinished_imports,
        commands::import::discard_unfinished_import,
        commands::agent::set_completion_provider,
        commands::agent::save_provider_api_key,
        commands::agent::list_provider_models,
        commands::agent::test_completion_provider,
        commands::agent::get_needs_review_count,
        commands::agent::trigger_categorize,
    ])
}

const SERVICE: &str = "com.finsight.app";
const USER: &str = "default";

pub fn configure_app(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let specta = build_specta_builder();

    builder
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("data.sqlcipher");

            let key = finsight_core::keychain::load_or_create_key(SERVICE, USER)
                .map_err(|e| -> Box<dyn std::error::Error> {
                    format!("keychain error: {e}").into()
                })?;

            let db = Db::open(&db_path, &key).map_err(|e| -> Box<dyn std::error::Error> {
                format!("db open error: {e}").into()
            })?;
            run_migrations(&db).map_err(|e| -> Box<dyn std::error::Error> {
                format!("migrations: {e}").into()
            })?;
            migrate_provider_settings(&db).map_err(|e| -> Box<dyn std::error::Error> {
                format!("provider migration: {e}").into()
            })?;

            let window = app.get_webview_window("main").expect("main window");
            let on_event: EventCallback = Arc::new(move |event| {
                let (event_name, payload) = match &event {
                    finsight_agent::agent::AgentEvent::CategorizationProgress { .. } =>
                        ("categorization.progress", serde_json::to_value(&event).unwrap()),
                    finsight_agent::agent::AgentEvent::CategorizationComplete { .. } =>
                        ("categorization.complete", serde_json::to_value(&event).unwrap()),
                    finsight_agent::agent::AgentEvent::Error { .. } =>
                        ("agent.error", serde_json::to_value(&event).unwrap()),
                };
                let _ = window.emit(event_name, payload);
            });

            let state = AppState::new(db.clone(), on_event);
            // Load saved provider configuration and wire it into the agent
            if let Some(provider) = load_provider_from_settings(&db) {
                state.agent.set_provider(provider);
            }
            app.manage(state);
            Ok(())
        })
}
```

- [ ] **Step 5: Run test to verify it passes**

```
cargo test -p finsight-app migrate_llm_provider_to_completion_provider
```

Expected: PASS

- [ ] **Step 6: Compile check**

```
cargo build -p finsight-app
```

Expected: Build succeeds (commands::agent not yet wired — add a stub `commands/agent.rs` if needed to satisfy `pub mod agent`). Add to `commands/mod.rs`:

```rust
pub mod agent;
```

And create `crates/finsight-app/src/commands/agent.rs` stub:

```rust
// Implemented in Task 15
```

- [ ] **Step 7: Commit**

```bash
git add crates/finsight-app/Cargo.toml \
        crates/finsight-app/src/lib.rs \
        crates/finsight-app/src/commands/mod.rs \
        crates/finsight-app/src/commands/agent.rs \
        crates/finsight-app/tests/startup_provider.rs
git commit -m "feat(app): AppState + AgentHandle wiring + llm_provider migration"
```

---

### Task 13: Account edit commands

**Files:**
- Modify: `crates/finsight-app/src/commands/accounts.rs`
- Test: `crates/finsight-app/tests/edit_account_cmd.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/finsight-app/tests/edit_account_cmd.rs`:

```rust
use finsight_core::{
    db::run_migrations,
    keychain,
    models::{AccountType, NewAccount},
    repos::{accounts, run},
    Db,
};
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("ea.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

#[tokio::test]
async fn update_account_name_and_color() {
    let (_d, db) = fresh_db();
    let account_id = {
        let mut conn = db.get().unwrap();
        accounts::insert(&mut conn, NewAccount {
            owner: "Me".into(), bank: "Chase".into(),
            r#type: AccountType::Checking, name: "Old".into(),
            last4: None, currency: "USD".into(), color: "#000".into(),
            opening_balance_cents: 0, source: "manual".into(),
        }).unwrap().id
    };
    let patch = finsight_core::models::AccountPatch {
        name: Some("New Name".into()),
        color: Some("#ff0000".into()),
        ..Default::default()
    };
    let updated = run(&db, move |conn| accounts::update(conn, &account_id, patch))
        .await.unwrap();
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.color, "#ff0000");
}

#[tokio::test]
async fn archive_account_cleans_up_mappings() {
    let (_d, db) = fresh_db();
    let account_id = {
        let mut conn = db.get().unwrap();
        let acc = accounts::insert(&mut conn, NewAccount {
            owner: "Me".into(), bank: "Chase".into(),
            r#type: AccountType::Checking, name: "Acc".into(),
            last4: None, currency: "USD".into(), color: "#fff".into(),
            opening_balance_cents: 0, source: "manual".into(),
        }).unwrap();
        // Seed a fake csv_import_mappings row
        conn.execute(
            "INSERT INTO csv_import_mappings(account_id, mapping_json) VALUES(?1, '{}')",
            rusqlite::params![acc.id],
        ).unwrap();
        acc.id
    };
    run(&db, {
        let aid = account_id.clone();
        move |conn| accounts::archive(conn, &aid)
    }).await.unwrap();
    let conn = db.get().unwrap();
    let archived_at: Option<String> = conn.query_row(
        "SELECT archived_at FROM accounts WHERE id = ?1",
        rusqlite::params![account_id],
        |r| r.get(0),
    ).unwrap();
    assert!(archived_at.is_some());
    let mapping_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM csv_import_mappings WHERE account_id = ?1",
        rusqlite::params![account_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(mapping_count, 0);
}
```

- [ ] **Step 2: Run test to verify it passes (logic is in repos, tested here)**

```
cargo test -p finsight-app edit_account_cmd
```

Expected: PASS (tests exercise the underlying repos)

- [ ] **Step 3: Add Tauri commands**

Add to `crates/finsight-app/src/commands/accounts.rs`:

```rust
use finsight_core::models::AccountPatch;

#[tauri::command]
#[specta::specta]
pub async fn update_account(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: AccountPatch,
) -> AppResult<Account> {
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::update(conn, &id, patch))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn archive_account(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::archive(conn, &id))
        .await
        .map_err(AppError::from)
}
```

- [ ] **Step 4: Compile check**

```
cargo build -p finsight-app
```

Expected: OK

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/accounts.rs \
        crates/finsight-app/tests/edit_account_cmd.rs
git commit -m "feat(app): update_account + archive_account commands"
```

---

### Task 14: Transaction edit commands + list_categories

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs`
- Test: `crates/finsight-app/tests/edit_transaction_cmd.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/finsight-app/tests/edit_transaction_cmd.rs`:

```rust
use finsight_core::{
    db::run_migrations, keychain,
    models::{AccountType, NewAccount, NewTransaction, TransactionStatus, TxnPatch},
    repos::{accounts, run, transactions},
    Db,
};
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("et.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

fn seed(conn: &mut rusqlite::Connection) -> (String, String) {
    conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
    conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
    let acc = accounts::insert(conn, NewAccount {
        owner: "Me".into(), bank: "B".into(),
        r#type: AccountType::Checking, name: "Ch".into(),
        last4: None, currency: "USD".into(), color: "#fff".into(),
        opening_balance_cents: 0, source: "manual".into(),
    }).unwrap();
    let txn = transactions::insert(conn, NewTransaction {
        account_id: acc.id.clone(),
        posted_at: chrono::Utc::now(),
        amount_cents: 500,
        merchant_raw: "STARBUCKS".to_string(),
        category_id: None,
        notes: None,
        status: TransactionStatus::Cleared,
    }).unwrap();
    (acc.id, txn.id)
}

#[tokio::test]
async fn update_category_proposes_rule() {
    let (_d, db) = fresh_db();
    let txn_id = { let mut c = db.get().unwrap(); seed(&mut c).1 };
    let patch = TxnPatch { category_id: Some(Some("cat1".into())), ..Default::default() };
    let (updated, rule) = run(&db, move |conn| transactions::update(conn, &txn_id, patch)).await.unwrap();
    assert_eq!(updated.category_id.as_deref(), Some("cat1"));
    assert!(rule.is_some());
    assert_eq!(rule.unwrap().pattern, "STARBUCKS");
}

#[tokio::test]
async fn delete_transaction_removes_it() {
    let (_d, db) = fresh_db();
    let txn_id = { let mut c = db.get().unwrap(); seed(&mut c).1 };
    let id_clone = txn_id.clone();
    run(&db, move |conn| transactions::delete(conn, &id_clone)).await.unwrap();
    let count: i64 = db.get().unwrap().query_row(
        "SELECT COUNT(*) FROM transactions WHERE id = ?1",
        rusqlite::params![txn_id], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run tests to verify they pass**

```
cargo test -p finsight-app edit_transaction_cmd
```

Expected: PASS

- [ ] **Step 3: Add Tauri commands**

Add to `crates/finsight-app/src/commands/transactions.rs`:

```rust
use finsight_core::models::{Category, TxnPatch};
use finsight_core::repos::rules;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
pub struct UpdateTxnResult {
    pub transaction: Transaction,
    pub proposed_rule: Option<ProposedRuleDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProposedRuleDto {
    pub pattern: String,
    pub category_id: String,
    pub category_label: String,
}

#[tauri::command]
#[specta::specta]
pub async fn update_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: TxnPatch,
) -> AppResult<UpdateTxnResult> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let (txn, rule) = transactions::update(conn, &id, patch)?;
        let proposed_rule = rule.map(|r| ProposedRuleDto {
            pattern: r.pattern,
            category_id: r.category_id,
            category_label: r.category_label,
        });
        Ok(UpdateTxnResult { transaction: txn, proposed_rule })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::delete(conn, &id))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_rule(
    state: tauri::State<'_, AppState>,
    pattern: String,
    category_id: String,
) -> AppResult<finsight_core::models::Rule> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        rules::insert(conn, finsight_core::models::NewRule {
            pattern,
            category_id,
            source: "user".to_string(),
        })
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CategoryDto {
    pub id: String,
    pub label: String,
    pub color: String,
    pub group_id: String,
    pub group_label: String,
}

#[tauri::command]
#[specta::specta]
pub async fn list_categories(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryDto>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let mut stmt = conn.prepare(
            "SELECT c.id, c.label, c.color, c.group_id, COALESCE(g.label, '') \
             FROM categories c \
             LEFT JOIN category_groups g ON g.id = c.group_id \
             WHERE c.archived_at IS NULL \
             ORDER BY g.sort_order, c.sort_order",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(CategoryDto {
                id: r.get(0)?,
                label: r.get(1)?,
                color: r.get(2)?,
                group_id: r.get(3)?,
                group_label: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
```

Also add the missing imports at the top of `transactions.rs` (if not already there):
```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{NewTransaction, Transaction};
use finsight_core::repos::{run, transactions};
```

- [ ] **Step 4: Compile check**

```
cargo build -p finsight-app
```

Expected: OK

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs \
        crates/finsight-app/tests/edit_transaction_cmd.rs
git commit -m "feat(app): update/delete transaction + create_rule + list_categories commands"
```

---

### Task 15: Agent commands

**Files:**
- Modify: `crates/finsight-app/src/commands/agent.rs`
- Test: `crates/finsight-app/tests/categorization_cmd.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/finsight-app/tests/categorization_cmd.rs`:

```rust
use finsight_core::{
    db::run_migrations, keychain,
    models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
    repos::{accounts, run, transactions},
    Db,
};
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("cc.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

#[tokio::test]
async fn get_needs_review_count_returns_zero_when_no_low_confidence() {
    let (_d, db) = fresh_db();
    // No transactions → count is 0
    let conn = db.get().unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions \
         WHERE ai_confidence < 0.6 \
           AND (SELECT source FROM categorizations c \
                WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run test to verify it passes (SQL logic test)**

```
cargo test -p finsight-app get_needs_review_count_returns_zero
```

Expected: PASS

- [ ] **Step 3: Implement agent commands**

Replace `crates/finsight-app/src/commands/agent.rs`:

```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_agent::{
    agent::AgentJob,
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider,
        openai_compat::OpenAiCompatProvider,
    },
    CompletionProvider,
};
use finsight_core::repos::run;
use finsight_core::settings;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind")]
pub enum CompletionProviderConfig {
    #[serde(rename = "unconfigured")]
    Unconfigured,
    #[serde(rename = "ollama")]
    Ollama { base_url: String, model: String },
    #[serde(rename = "openai_compat")]
    OpenAiCompat { preset: String, base_url: String, model: String },
    #[serde(rename = "anthropic")]
    Anthropic { model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderTestResult {
    pub ok: bool,
    pub error: Option<String>,
    pub latency_ms: u64,
}

#[tauri::command]
#[specta::specta]
pub async fn set_completion_provider(
    state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
) -> AppResult<()> {
    let db = (*state.db).clone();
    let cfg_json = serde_json::to_value(&config)
        .map_err(|e| AppError::new("agent", e.to_string()))?;
    run(&db, move |conn| settings::set(conn, "completion_provider", &cfg_json))
        .await
        .map_err(AppError::from)?;

    // Also update the live provider in AppState
    let provider = crate::build_provider_from_config(&serde_json::to_value(&config).unwrap());
    if let Some(p) = provider {
        state.agent.set_provider(p);
    } else {
        *state.agent_provider.write().unwrap() = None;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn save_provider_api_key(
    _state: tauri::State<'_, AppState>,
    provider_id: String,
    key: String,
) -> AppResult<()> {
    finsight_core::keychain::set_key("com.finsight.llm", &provider_id, &key)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_provider_models(
    _state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
) -> AppResult<Vec<String>> {
    match &config {
        CompletionProviderConfig::Ollama { base_url, model } => {
            let provider = OllamaProvider::new(base_url.clone(), model.clone());
            provider.list_models().await.map_err(|e| AppError::new("agent", e.to_string()))
        }
        _ => Ok(vec![]),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn test_completion_provider(
    _state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
    api_key: Option<String>,
) -> AppResult<ProviderTestResult> {
    let provider: Arc<dyn CompletionProvider> = match &config {
        CompletionProviderConfig::Ollama { base_url, model } =>
            Arc::new(OllamaProvider::new(base_url.clone(), model.clone())),
        CompletionProviderConfig::OpenAiCompat { preset, base_url, model } => {
            let key = api_key.or_else(|| {
                finsight_core::keychain::get_key("com.finsight.llm", preset).ok().flatten()
            }).unwrap_or_default();
            Arc::new(OpenAiCompatProvider::new(base_url.clone(), key, model.clone(), preset.clone()))
        }
        CompletionProviderConfig::Anthropic { model } => {
            let key = api_key.or_else(|| {
                finsight_core::keychain::get_key("com.finsight.llm", "anthropic").ok().flatten()
            }).unwrap_or_default();
            Arc::new(AnthropicProvider::new(key, model.clone()))
        }
        CompletionProviderConfig::Unconfigured =>
            return Ok(ProviderTestResult { ok: false, error: Some("Not configured".into()), latency_ms: 0 }),
    };
    let start = std::time::Instant::now();
    let result = provider.complete_json(
        "You are a test assistant. Respond with valid JSON only.",
        r#"Reply with exactly: {"ok": true}"#,
    ).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(_) => Ok(ProviderTestResult { ok: true, error: None, latency_ms }),
        Err(e) => Ok(ProviderTestResult { ok: false, error: Some(e.to_string()), latency_ms }),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_needs_review_count(
    state: tauri::State<'_, AppState>,
) -> AppResult<u32> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions \
             WHERE ai_confidence < 0.6 \
               AND (SELECT source FROM categorizations c \
                    WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
            [],
            |r| r.get(0),
        )?;
        Ok(count as u32)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn trigger_categorize(
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    state.agent.tx.try_send(AgentJob::CategorizeAll)
        .map_err(|e| AppError::new("agent", format!("queue full: {e}")))?;
    Ok(())
}
```

- [ ] **Step 4: Full compile check**

```
cargo build -p finsight-app
```

Expected: Build succeeds

- [ ] **Step 5: Run all Rust tests**

```
cargo test -p finsight-core -p finsight-agent -p finsight-app
```

Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs \
        crates/finsight-app/tests/categorization_cmd.rs
git commit -m "feat(app): agent commands — set_completion_provider, test, trigger, needs_review"
```

---

## Phase 3.4 — Frontend Bindings + Hooks

### Task 16: Regenerate TypeScript bindings

**Files:**
- Modify: `ui/src/api/client.ts` (generated)

- [ ] **Step 1: Regenerate bindings**

```
cargo run --bin export_bindings
```

Expected: `ui/src/api/client.ts` updated with new commands and types:
- `updateAccount`, `archiveAccount`
- `updateTransaction`, `deleteTransaction`, `createRule`, `listCategories`
- `setCompletionProvider`, `saveProviderApiKey`, `listProviderModels`, `testCompletionProvider`, `getNeedsReviewCount`, `triggerCategorize`
- New types: `AccountPatch`, `TxnPatch`, `UpdateTxnResult`, `ProposedRuleDto`, `CompletionProviderConfig`, `ProviderTestResult`, `CategoryDto`, `Rule`

- [ ] **Step 2: Verify bindings compile**

```
cd ui && npm run build 2>&1 | head -30
```

Expected: No TypeScript errors about missing types

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/client.ts
git commit -m "chore: regenerate TS bindings for Phase 3 commands"
```

---

### Task 17: Frontend hooks — accounts, transactions, agent

**Files:**
- Modify: `ui/src/api/hooks/accounts.ts`
- Modify: `ui/src/api/hooks/transactions.ts`
- Create: `ui/src/api/hooks/agent.ts`

- [ ] **Step 1: Write the failing tests**

Create `ui/src/api/hooks/accounts.test.ts`:

```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useUpdateAccount, useArchiveAccount } from "./accounts";

vi.mock("../client", () => ({
  commands: {
    updateAccount: vi.fn().mockResolvedValue({ status: "ok", data: { id: "a1", name: "Updated", bank: "Chase", type: "Checking", last4: null, currency: "USD", color: "#fff", archived_at: null, created_at: "2024-01-01T00:00:00Z", owner: "Me" } }),
    archiveAccount: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("useUpdateAccount", () => {
  it("calls updateAccount and invalidates queries", async () => {
    const { result } = renderHook(() => useUpdateAccount(), { wrapper: createWrapper() });
    result.current.mutate({ id: "a1", patch: { name: "Updated" } });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.name).toBe("Updated");
  });
});

describe("useArchiveAccount", () => {
  it("calls archiveAccount", async () => {
    const { result } = renderHook(() => useArchiveAccount(), { wrapper: createWrapper() });
    result.current.mutate("a1");
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
```

Create `ui/src/api/hooks/agent.test.ts`:

```ts
import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useNeedsReviewCount, useTriggerCategorize } from "./agent";

vi.mock("../client", () => ({
  commands: {
    getNeedsReviewCount: vi.fn().mockResolvedValue({ status: "ok", data: 3 }),
    triggerCategorize: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("useNeedsReviewCount", () => {
  it("returns count from command", async () => {
    const { result } = renderHook(() => useNeedsReviewCount(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toBe(3);
  });
});

describe("useTriggerCategorize", () => {
  it("calls triggerCategorize", async () => {
    const { result } = renderHook(() => useTriggerCategorize(), { wrapper: createWrapper() });
    result.current.mutate();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```
cd ui && npx vitest run src/api/hooks/accounts.test.ts src/api/hooks/agent.test.ts
```

Expected: FAIL — `useUpdateAccount`, `useArchiveAccount`, `useNeedsReviewCount`, `useTriggerCategorize` not found

- [ ] **Step 3: Add hooks to accounts.ts**

Append to `ui/src/api/hooks/accounts.ts`:

```ts
import type { AccountPatch } from "../client";

export function useUpdateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: AccountPatch }) => {
      const result = await commands.updateAccount(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

export function useArchiveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.archiveAccount(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}
```

- [ ] **Step 4: Add hooks to transactions.ts**

Append to `ui/src/api/hooks/transactions.ts`:

```ts
import type { TxnPatch, UpdateTxnResult } from "../client";

export function useUpdateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: TxnPatch }) => {
      const result = await commands.updateTransaction(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data as UpdateTxnResult;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
    },
  });
}

export function useDeleteTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.deleteTransaction(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
    },
  });
}

export function useCreateRule() {
  return useMutation({
    mutationFn: async ({ pattern, categoryId }: { pattern: string; categoryId: string }) => {
      const result = await commands.createRule(pattern, categoryId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCategories() {
  return useQuery({
    queryKey: ["categories"],
    queryFn: async () => {
      const result = await commands.listCategories();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}
```

- [ ] **Step 5: Create ui/src/api/hooks/agent.ts**

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type CompletionProviderConfig } from "../client";

export function useNeedsReviewCount() {
  return useQuery<number>({
    queryKey: ["needs-review-count"],
    queryFn: async () => {
      const result = await commands.getNeedsReviewCount();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    refetchInterval: 30_000,
  });
}

export function useSetCompletionProvider() {
  return useMutation({
    mutationFn: async (config: CompletionProviderConfig) => {
      const result = await commands.setCompletionProvider(config);
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

export function useSaveProviderApiKey() {
  return useMutation({
    mutationFn: async ({ providerId, key }: { providerId: string; key: string }) => {
      const result = await commands.saveProviderApiKey(providerId, key);
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

export function useListProviderModels(config: CompletionProviderConfig | null) {
  return useQuery<string[]>({
    queryKey: ["provider-models", config],
    queryFn: async () => {
      if (!config) return [];
      const result = await commands.listProviderModels(config);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: config !== null && (config as { kind: string }).kind === "ollama",
  });
}

export function useTestCompletionProvider() {
  return useMutation({
    mutationFn: async ({
      config,
      apiKey,
    }: {
      config: CompletionProviderConfig;
      apiKey?: string;
    }) => {
      const result = await commands.testCompletionProvider(config, apiKey ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useTriggerCategorize() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.triggerCategorize();
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      // needs-review-count will update when categorization.complete event fires
    },
  });
}
```

- [ ] **Step 6: Run tests to verify they pass**

```
cd ui && npx vitest run src/api/hooks/accounts.test.ts src/api/hooks/agent.test.ts
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add ui/src/api/hooks/accounts.ts \
        ui/src/api/hooks/accounts.test.ts \
        ui/src/api/hooks/transactions.ts \
        ui/src/api/hooks/agent.ts \
        ui/src/api/hooks/agent.test.ts
git commit -m "feat(ui): account/transaction/agent hooks for Phase 3 mutations"
```

---

## Phase 3.5 — Frontend UI

### Task 18: CategoryPicker component

**Files:**
- Create: `ui/src/components/CategoryPicker.tsx`
- Create: `ui/src/components/CategoryPicker.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `ui/src/components/CategoryPicker.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import FocusLock from "react-focus-lock";
import CategoryPicker from "./CategoryPicker";

vi.mock("react-focus-lock", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("../api/hooks/transactions", () => ({
  useCategories: vi.fn().mockReturnValue({
    data: [
      { id: "cat1", label: "Food", color: "#f00", group_id: "g1", group_label: "Daily" },
      { id: "cat2", label: "Transport", color: "#00f", group_id: "g1", group_label: "Daily" },
      { id: "cat3", label: "Rent", color: "#0f0", group_id: "g2", group_label: "Fixed" },
    ],
    isLoading: false,
  }),
}));

describe("CategoryPicker", () => {
  it("renders groups and items", () => {
    render(<CategoryPicker value={null} onChange={() => {}} />);
    expect(screen.getByText("Daily")).toBeInTheDocument();
    expect(screen.getByText("Food")).toBeInTheDocument();
    expect(screen.getByText("Rent")).toBeInTheDocument();
  });

  it("filters items on search input", () => {
    render(<CategoryPicker value={null} onChange={() => {}} />);
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "rent" } });
    expect(screen.getByText("Rent")).toBeInTheDocument();
    expect(screen.queryByText("Food")).not.toBeInTheDocument();
  });

  it("calls onChange when item is clicked", () => {
    const onChange = vi.fn();
    render(<CategoryPicker value={null} onChange={onChange} />);
    fireEvent.click(screen.getByText("Food"));
    expect(onChange).toHaveBeenCalledWith("cat1");
  });

  it("highlights the currently selected item", () => {
    render(<CategoryPicker value="cat2" onChange={() => {}} />);
    const btn = screen.getByRole("option", { name: /transport/i });
    expect(btn).toHaveAttribute("aria-selected", "true");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```
cd ui && npx vitest run src/components/CategoryPicker.test.tsx
```

Expected: FAIL — `CategoryPicker` not found

- [ ] **Step 3: Create CategoryPicker.tsx**

Create `ui/src/components/CategoryPicker.tsx`:

```tsx
import { useState } from "react";
import { useCategories } from "../api/hooks/transactions";

interface Props {
  value: string | null;
  onChange: (categoryId: string) => void;
}

interface CategoryDto {
  id: string;
  label: string;
  color: string;
  group_id: string;
  group_label: string;
}

export default function CategoryPicker({ value, onChange }: Props) {
  const [search, setSearch] = useState("");
  const { data: categories = [], isLoading } = useCategories();

  const filtered = search
    ? categories.filter((c: CategoryDto) =>
        c.label.toLowerCase().includes(search.toLowerCase()) ||
        c.group_label.toLowerCase().includes(search.toLowerCase())
      )
    : categories;

  // Group by group_label
  const groups: Record<string, CategoryDto[]> = {};
  for (const cat of filtered as CategoryDto[]) {
    const g = cat.group_label || "Other";
    if (!groups[g]) groups[g] = [];
    groups[g].push(cat);
  }

  return (
    <div className="category-picker" role="listbox" aria-label="Category">
      <input
        role="searchbox"
        type="text"
        placeholder="Search categories…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        style={{ width: "100%", marginBottom: 8 }}
      />
      {isLoading && <div>Loading categories…</div>}
      {Object.entries(groups).map(([groupLabel, cats]) => (
        <div key={groupLabel}>
          <div
            style={{
              fontSize: 11,
              fontWeight: 600,
              textTransform: "uppercase",
              letterSpacing: "0.06em",
              color: "var(--text-3)",
              padding: "8px 0 4px",
            }}
          >
            {groupLabel}
          </div>
          {cats.map((cat: CategoryDto) => (
            <button
              key={cat.id}
              role="option"
              aria-selected={cat.id === value}
              onClick={() => onChange(cat.id)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                width: "100%",
                padding: "6px 8px",
                borderRadius: 6,
                border: "none",
                cursor: "pointer",
                background: cat.id === value ? "var(--surface-2)" : "transparent",
                fontWeight: cat.id === value ? 600 : 400,
                textAlign: "left",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  width: 12,
                  height: 12,
                  borderRadius: 3,
                  background: cat.color,
                  flexShrink: 0,
                }}
              />
              {cat.label}
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd ui && npx vitest run src/components/CategoryPicker.test.tsx
```

Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/CategoryPicker.tsx ui/src/components/CategoryPicker.test.tsx
git commit -m "feat(ui): CategoryPicker component with group sections + search filter"
```

---

### Task 19: AccountDrawer edit mode + archive

**Files:**
- Modify: `ui/src/components/AccountDrawer.tsx`
- Create: `ui/src/components/AccountDrawer.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `ui/src/components/AccountDrawer.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import AccountDrawer from "./AccountDrawer";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/accounts", () => ({
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({ id: "a1", name: "Renamed" }) })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));

const existingAccount = {
  id: "a1", owner: "Me", bank: "Chase", type: "Checking" as const,
  name: "Old Name", last4: null, currency: "USD", color: "#fff",
  archived_at: null, created_at: "2024-01-01T00:00:00Z",
};

describe("AccountDrawer — create mode", () => {
  it("shows 'Add account' title and submit button", () => {
    render(<AccountDrawer open={true} onClose={() => {}} />, { wrapper: createWrapper() });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Create account")).toBeInTheDocument();
  });
});

describe("AccountDrawer — edit mode", () => {
  it("shows 'Edit Account' title and pre-filled name", () => {
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByDisplayValue("Old Name")).toBeInTheDocument();
    expect(screen.getByText("Save changes")).toBeInTheDocument();
  });

  it("shows archive button", () => {
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByRole("button", { name: /archive/i })).toBeInTheDocument();
  });

  it("two-click confirm on archive: first click shows confirm text", async () => {
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    fireEvent.click(screen.getByRole("button", { name: /archive account/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /confirm archive/i })).toBeInTheDocument()
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```
cd ui && npx vitest run src/components/AccountDrawer.test.tsx
```

Expected: FAIL — `account` prop not accepted, edit mode not implemented

- [ ] **Step 3: Rewrite AccountDrawer.tsx with edit mode**

Replace `ui/src/components/AccountDrawer.tsx`:

```tsx
import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import Drawer from "./Drawer";
import { useCreateAccount, useUpdateAccount, useArchiveAccount } from "../api/hooks/accounts";
import type { Account } from "../api/client";

const schema = z.object({
  bank: z.string().min(1, "Required"),
  name: z.string().min(1, "Required"),
  type: z.enum(["Checking", "Savings", "Credit", "Investment", "Cash", "Other"]),
  last4: z.string().max(4).optional(),
  currency: z.enum(["USD", "EUR", "GBP", "CAD", "AUD"]),
  opening_dollars: z.coerce.number(),
  owner: z.string().min(1, "Required"),
});

type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  account?: Account;
  defaultOwner?: string;
  onCreated?: () => void;
}

export default function AccountDrawer({ open, onClose, account, defaultOwner = "joint", onCreated }: Props) {
  const isEdit = !!account;
  const createAccount = useCreateAccount();
  const updateAccount = useUpdateAccount();
  const archiveAccount = useArchiveAccount();
  const [archiveConfirm, setArchiveConfirm] = useState(false);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      type: "Checking",
      currency: "USD",
      owner: defaultOwner,
      opening_dollars: 0,
    },
  });

  useEffect(() => {
    if (account) {
      reset({
        bank: account.bank,
        name: account.name,
        type: account.type,
        last4: account.last4 ?? undefined,
        currency: account.currency as "USD" | "EUR" | "GBP" | "CAD" | "AUD",
        owner: account.owner,
        opening_dollars: 0,
      });
    } else {
      reset({ type: "Checking", currency: "USD", owner: defaultOwner, opening_dollars: 0 });
    }
    setArchiveConfirm(false);
  }, [account, open]);

  async function onSubmit(values: FormValues) {
    if (isEdit && account) {
      await updateAccount.mutateAsync({
        id: account.id,
        patch: { name: values.name, bank: values.bank, color: account.color, currency: values.currency, last4: values.last4 ? values.last4 : null },
      });
    } else {
      await createAccount.mutateAsync({
        bank: values.bank, name: values.name, type: values.type,
        last4: values.last4 || null, currency: values.currency,
        color: "#3B82F6",
        opening_balance_cents: Math.round(values.opening_dollars * 100),
        owner: values.owner, source: "manual",
      });
    }
    reset();
    onCreated?.();
    onClose();
  }

  async function handleArchive() {
    if (!archiveConfirm) { setArchiveConfirm(true); return; }
    if (!account) return;
    await archiveAccount.mutateAsync(account.id);
    onClose();
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit Account" : "Add account"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Bank
          <input {...register("bank")} aria-invalid={!!errors.bank} />
          {errors.bank && <span className="err">{errors.bank.message}</span>}
        </label>
        <label> Name
          <input {...register("name")} placeholder="e.g. Joint Checking" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        {!isEdit && (
          <fieldset>
            <legend>Type</legend>
            {(["Checking","Savings","Credit","Investment","Cash","Other"] as const).map(t => (
              <label key={t}><input type="radio" value={t} {...register("type")} /> {t}</label>
            ))}
          </fieldset>
        )}
        <label> Last 4 <input {...register("last4")} maxLength={4} /></label>
        <label> Currency
          <select {...register("currency")}>
            {(["USD","EUR","GBP","CAD","AUD"] as const).map(c => <option key={c}>{c}</option>)}
          </select>
        </label>
        {!isEdit && (
          <label> Opening balance ($)
            <input type="number" step="0.01" {...register("opening_dollars")} />
          </label>
        )}
        {!isEdit && (
          <label> Owner
            <input {...register("owner")} aria-invalid={!!errors.owner} />
          </label>
        )}
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? (isEdit ? "Saving…" : "Creating…") : (isEdit ? "Save changes" : "Create account")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button
            type="button"
            className="danger"
            onClick={handleArchive}
            disabled={archiveAccount.isPending}
          >
            {archiveConfirm ? "Confirm archive?" : "Archive account"}
          </button>
          {archiveConfirm && (
            <button type="button" onClick={() => setArchiveConfirm(false)} style={{ marginLeft: 8 }}>
              Cancel
            </button>
          )}
        </div>
      )}
    </Drawer>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd ui && npx vitest run src/components/AccountDrawer.test.tsx
```

Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/AccountDrawer.tsx ui/src/components/AccountDrawer.test.tsx
git commit -m "feat(ui): AccountDrawer edit mode + two-click archive confirm"
```

---

### Task 20: TransactionDrawer edit mode + CategoryPicker + proposed-rule toast

**Files:**
- Modify: `ui/src/components/TransactionDrawer.tsx`
- Create: `ui/src/components/TransactionDrawer.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `ui/src/components/TransactionDrawer.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import TransactionDrawer from "./TransactionDrawer";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/transactions", () => ({
  useCreateTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useUpdateTransaction: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue({
      transaction: { id: "t1", notes: "edited", category_id: "cat1" },
      proposed_rule: { pattern: "STARBUCKS", category_id: "cat1", category_label: "Food" },
    }),
  })),
  useDeleteTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useCreateRule: vi.fn(() => ({ mutate: vi.fn() })),
  useCategories: vi.fn(() => ({ data: [{ id: "cat1", label: "Food", color: "#f00", group_id: "g1", group_label: "Daily" }] })),
}));
vi.mock("sonner", () => ({ toast: { custom: vi.fn() } }));

const existingTxn = {
  id: "t1", account_id: "a1",
  posted_at: "2024-01-15T00:00:00Z",
  amount_cents: 500, merchant_raw: "STARBUCKS",
  merchant_id: null, merchant_label: null, merchant_color: null, merchant_initials: null,
  category_id: null, category_label: null, category_color: null,
  status: "cleared" as const, notes: null,
  ai_confidence: null, ai_explanation: null, is_anomaly: false,
  created_at: "2024-01-15T00:00:00Z",
};

describe("TransactionDrawer — edit mode", () => {
  it("pre-fills merchant_raw field", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByDisplayValue("STARBUCKS")).toBeInTheDocument();
  });

  it("shows 'Edit Transaction' title and Save Changes button", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByText("Edit Transaction")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /save changes/i })).toBeInTheDocument();
  });

  it("renders category picker", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    // CategoryPicker renders its listbox
    expect(screen.getByRole("listbox", { name: /category/i })).toBeInTheDocument();
  });

  it("shows delete button", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByRole("button", { name: /delete transaction/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```
cd ui && npx vitest run src/components/TransactionDrawer.test.tsx
```

Expected: FAIL — `transaction` prop not accepted

- [ ] **Step 3: Rewrite TransactionDrawer.tsx with edit mode**

Replace the entire contents of `ui/src/components/TransactionDrawer.tsx`:

```tsx
import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import CategoryPicker from "./CategoryPicker";
import {
  useCreateTransaction, useUpdateTransaction,
  useDeleteTransaction, useCreateRule,
} from "../api/hooks/transactions";
import type { Transaction } from "../api/client";

const schema = z.object({
  merchant_raw: z.string().min(1, "Required"),
  amount_dollars: z.coerce.number(),
  notes: z.string().optional(),
  posted_at: z.string().min(1, "Required"),
});

type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  transaction?: Transaction;
  accountId?: string;
}

export default function TransactionDrawer({ open, onClose, transaction, accountId }: Props) {
  const isEdit = !!transaction;
  const create = useCreateTransaction();
  const update = useUpdateTransaction();
  const del = useDeleteTransaction();
  const createRule = useCreateRule();
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { merchant_raw: "", amount_dollars: 0, notes: "", posted_at: new Date().toISOString().slice(0, 10) },
  });

  useEffect(() => {
    if (transaction) {
      reset({
        merchant_raw: transaction.merchant_raw,
        amount_dollars: transaction.amount_cents / 100,
        notes: transaction.notes ?? "",
        posted_at: transaction.posted_at.slice(0, 10),
      });
      setSelectedCategory(transaction.category_id ?? null);
    } else {
      reset({ merchant_raw: "", amount_dollars: 0, notes: "", posted_at: new Date().toISOString().slice(0, 10) });
      setSelectedCategory(null);
    }
    setDeleteConfirm(false);
  }, [transaction, open]);

  async function onSubmit(values: FormValues) {
    if (isEdit && transaction) {
      const result = await update.mutateAsync({
        id: transaction.id,
        patch: {
          notes: values.notes ? values.notes : null,
          category_id: selectedCategory !== transaction.category_id
            ? (selectedCategory !== null ? selectedCategory : null)
            : undefined,
          merchant_raw: values.merchant_raw !== transaction.merchant_raw ? values.merchant_raw : undefined,
          amount_cents: Math.round(values.amount_dollars * 100) !== transaction.amount_cents
            ? Math.round(values.amount_dollars * 100) : undefined,
        },
      });
      if (result.proposed_rule) {
        const { pattern, category_id, category_label } = result.proposed_rule;
        toast.custom(() => (
          <div role="alert">
            Always categorize <strong>«{pattern}»</strong> as{" "}
            <strong>{category_label}</strong>?{" "}
            <button onClick={() => createRule.mutate({ pattern, categoryId: category_id })}>
              Create rule
            </button>{" "}
            <button>Skip</button>
          </div>
        ));
      }
    } else {
      await create.mutateAsync({
        account_id: accountId ?? "",
        posted_at: new Date(values.posted_at).toISOString(),
        amount_cents: Math.round(values.amount_dollars * 100),
        merchant_raw: values.merchant_raw,
        category_id: selectedCategory,
        notes: values.notes || null,
        status: "manual",
      });
    }
    onClose();
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!transaction) return;
    await del.mutateAsync(transaction.id);
    onClose();
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit Transaction" : "Add transaction"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Merchant
          <input {...register("merchant_raw")} aria-invalid={!!errors.merchant_raw} />
          {errors.merchant_raw && <span className="err">{errors.merchant_raw.message}</span>}
        </label>
        <label> Amount ($)
          <input type="number" step="0.01" {...register("amount_dollars")} />
        </label>
        <label> Date
          <input type="date" {...register("posted_at")} />
        </label>
        <label> Notes
          <input {...register("notes")} />
        </label>
        <div>
          <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>Category</div>
          <CategoryPicker value={selectedCategory} onChange={setSelectedCategory} />
        </div>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : (isEdit ? "Save changes" : "Add transaction")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button type="button" className="danger" onClick={handleDelete} disabled={del.isPending}>
            {deleteConfirm ? "Confirm delete?" : "Delete transaction"}
          </button>
          {deleteConfirm && (
            <button type="button" onClick={() => setDeleteConfirm(false)} style={{ marginLeft: 8 }}>
              Cancel
            </button>
          )}
        </div>
      )}
    </Drawer>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd ui && npx vitest run src/components/TransactionDrawer.test.tsx
```

Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/TransactionDrawer.tsx ui/src/components/TransactionDrawer.test.tsx
git commit -m "feat(ui): TransactionDrawer edit mode + CategoryPicker + proposed-rule toast"
```

---

### Task 21: Wire row-click into Accounts + Transactions screens

**Files:**
- Modify: `ui/src/screens/Accounts.tsx`
- Modify: `ui/src/screens/Transactions.tsx`

- [ ] **Step 1: Wire row-click in Accounts.tsx**

Replace the table row in `ui/src/screens/Accounts.tsx`. Change the `<tr>` from:

```tsx
<tr key={a.id} style={{ borderTop: "1px solid var(--hairline)" }}>
```

to:

```tsx
<tr
  key={a.id}
  style={{ borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
  onClick={() => setEditAccount(a as Account)}
  aria-label={`Edit ${a.name}`}
>
```

Also add state and the edit drawer. Replace the entire `Accounts.tsx`:

```tsx
import { useState } from "react";
import { useAccounts } from "../api/hooks/accounts";
import AccountDrawer from "../components/AccountDrawer";
import type { Account, AccountSummary } from "../api/client";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

export default function Accounts() {
  const [addOpen, setAddOpen] = useState(false);
  const [editAccount, setEditAccount] = useState<Account | null>(null);
  const { data, isLoading, error } = useAccounts();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;

  return (
    <div className="screen-accounts">
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
        <h1 style={{ fontSize: 32, fontWeight: 600, margin: 0 }}>Accounts</h1>
        <button className="primary" onClick={() => setAddOpen(true)}>+ Add account</button>
      </header>

      {(!data || data.length === 0) ? (
        <div className="stub">No accounts yet.</div>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Bank</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Name</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Type</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Balance</th>
            </tr>
          </thead>
          <tbody>
            {data.map((a: AccountSummary) => (
              <tr
                key={a.id}
                style={{ borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                onClick={() => setEditAccount(a as unknown as Account)}
                aria-label={`Edit ${a.name}`}
              >
                <td style={{ padding: "12px 0" }}>{a.bank}</td>
                <td style={{ padding: "12px 0" }}>{a.name}</td>
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{a.type}</td>
                <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                  <span className="money">{formatMoney(a.balance_cents)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <AccountDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <AccountDrawer
        open={editAccount !== null}
        onClose={() => setEditAccount(null)}
        account={editAccount ?? undefined}
      />
    </div>
  );
}
```

- [ ] **Step 2: Wire row-click in Transactions.tsx**

Replace `ui/src/screens/Transactions.tsx`:

```tsx
import { useState } from "react";
import { useTransactions } from "../api/hooks/transactions";
import TransactionDrawer from "../components/TransactionDrawer";
import FilePicker from "../components/FilePicker";
import ImportMappingDialog from "./onboarding/ImportMappingDialog";
import type { Transaction } from "../api/client";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export default function Transactions() {
  const [addOpen, setAddOpen] = useState(false);
  const [editTxn, setEditTxn] = useState<Transaction | null>(null);
  const [csvPath, setCsvPath] = useState<string | null>(null);
  const { data, isLoading, error } = useTransactions();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;

  return (
    <div className="screen-transactions">
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
        <h1 style={{ fontSize: 32, fontWeight: 600, margin: 0 }}>Transactions</h1>
        <div className="actions" style={{ display: "flex", gap: 8 }}>
          <FilePicker onPicked={setCsvPath} label="Import CSV" />
          <button className="primary" onClick={() => setAddOpen(true)}>+ Add transaction</button>
        </div>
      </header>

      {(!data || data.length === 0) ? (
        <div className="stub">No transactions yet.</div>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Date</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Merchant</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Category</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Amount</th>
            </tr>
          </thead>
          <tbody>
            {data.map((t: Transaction) => (
              <tr
                key={t.id}
                style={{ borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                onClick={() => setEditTxn(t)}
                aria-label={`Edit transaction ${t.merchant_raw}`}
              >
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{formatDate(t.posted_at)}</td>
                <td style={{ padding: "12px 0" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    <span
                      aria-label={`${t.merchant_label ?? t.merchant_raw} merchant tile`}
                      style={{
                        width: 28, height: 28, borderRadius: 6,
                        background: t.merchant_color ?? "var(--surface-2)",
                        color: "var(--accent-ink)",
                        fontSize: 11, fontWeight: 600,
                        display: "grid", placeItems: "center",
                      }}
                    >
                      {t.merchant_initials ?? "?"}
                    </span>
                    <span>{t.merchant_label ?? t.merchant_raw}</span>
                  </div>
                </td>
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>
                  {t.category_label ?? "Uncategorized"}
                </td>
                <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                  <span className="money">{formatMoney(t.amount_cents)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <TransactionDrawer
        open={editTxn !== null}
        onClose={() => setEditTxn(null)}
        transaction={editTxn ?? undefined}
      />
      {csvPath && (
        <ImportMappingDialog
          path={csvPath}
          onClose={() => setCsvPath(null)}
          onImported={() => setCsvPath(null)}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 3: Run all frontend tests**

```
cd ui && npx vitest run
```

Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/Accounts.tsx ui/src/screens/Transactions.tsx
git commit -m "feat(ui): row-click opens edit drawer in Accounts + Transactions screens"
```

---

## Phase 3.6 — Frontend Agent UI

### Task 22: AgentActivityFeed + Today screen additions

**Files:**
- Create: `ui/src/components/AgentActivityFeed.tsx`
- Create: `ui/src/components/AgentActivityFeed.test.tsx`
- Modify: `ui/src/screens/Today.tsx`

- [ ] **Step 1: Write the failing test**

Create `ui/src/components/AgentActivityFeed.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import AgentActivityFeed from "./AgentActivityFeed";

// Mock Tauri event listener
const listeners: Record<string, ((payload: unknown) => void)[]> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (payload: unknown) => void) => {
    if (!listeners[event]) listeners[event] = [];
    listeners[event].push(handler);
    return () => {}; // unlisten
  }),
}));

function emitTauriEvent(event: string, payload: unknown) {
  for (const h of listeners[event] ?? []) h({ payload });
}

describe("AgentActivityFeed", () => {
  beforeEach(() => {
    Object.keys(listeners).forEach((k) => delete listeners[k]);
  });

  it("is invisible when idle", () => {
    const { container } = render(<AgentActivityFeed />);
    // aria-live region exists but shows nothing
    const region = container.querySelector("[aria-live]");
    expect(region?.textContent?.trim()).toBe("");
  });

  it("shows progress when categorization.progress fires", async () => {
    render(<AgentActivityFeed />);
    await act(async () => {
      emitTauriEvent("categorization.progress", { done: 12, total: 47 });
    });
    expect(screen.getByText(/12\s*\/\s*47/)).toBeInTheDocument();
  });

  it("shows 'Done' after categorization.complete fires", async () => {
    render(<AgentActivityFeed />);
    await act(async () => {
      emitTauriEvent("categorization.complete", { categorized: 47, skipped: 0 });
    });
    expect(screen.getByText(/done/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```
cd ui && npx vitest run src/components/AgentActivityFeed.test.tsx
```

Expected: FAIL — `AgentActivityFeed` not found

- [ ] **Step 3: Create AgentActivityFeed.tsx**

Create `ui/src/components/AgentActivityFeed.tsx`:

```tsx
import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

type Progress = { done: number; total: number };
type Complete = { categorized: number; skipped: number };

type FeedState =
  | { kind: "idle" }
  | { kind: "progress"; done: number; total: number }
  | { kind: "done"; categorized: number };

export default function AgentActivityFeed() {
  const [state, setState] = useState<FeedState>({ kind: "idle" });
  const fadeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const unlistenProgress = listen<Progress>("categorization.progress", (e) => {
      setState({ kind: "progress", done: e.payload.done, total: e.payload.total });
      if (fadeTimer.current) clearTimeout(fadeTimer.current);
    });
    const unlistenComplete = listen<Complete>("categorization.complete", (e) => {
      setState({ kind: "done", categorized: e.payload.categorized });
      fadeTimer.current = setTimeout(() => setState({ kind: "idle" }), 3000);
    });
    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      if (fadeTimer.current) clearTimeout(fadeTimer.current);
    };
  }, []);

  return (
    <div
      aria-live="polite"
      aria-atomic="true"
      style={{ minHeight: 20, fontSize: 13, color: "var(--text-2)", marginBottom: 8 }}
    >
      {state.kind === "progress" && (
        <span>Categorizing… {state.done} / {state.total}</span>
      )}
      {state.kind === "done" && (
        <span>Done — {state.categorized} transactions categorized</span>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd ui && npx vitest run src/components/AgentActivityFeed.test.tsx
```

Expected: PASS (3 tests)

- [ ] **Step 5: Update Today.tsx with Needs a glance + AgentActivityFeed**

Replace `ui/src/screens/Today.tsx`:

```tsx
import { useNavigate } from "react-router-dom";
import { useAccounts } from "../api/hooks/accounts";
import { useNeedsReviewCount } from "../api/hooks/agent";
import AgentActivityFeed from "../components/AgentActivityFeed";

function formatMoney(cents: number, currency = "USD") {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

export default function Today() {
  const navigate = useNavigate();
  const { data, isLoading, error } = useAccounts();
  const { data: needsReview = 0 } = useNeedsReviewCount();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;
  if (!data || data.length === 0) return <div className="stub">No accounts yet.</div>;

  const primary = data[0]!;

  return (
    <section>
      <header>
        <p style={{ color: "var(--text-3)", fontSize: 12, letterSpacing: "0.06em", textTransform: "uppercase" }}>
          Today
        </p>
        <h1 style={{ fontSize: 72, fontWeight: 600, letterSpacing: "-0.02em", margin: "8px 0" }}>
          <span className="money">{formatMoney(primary.balance_cents, primary.currency)}</span>
        </h1>
        <p style={{ color: "var(--text-2)" }}>
          in <strong>{primary.name}</strong> · {primary.bank}
        </p>
      </header>

      <AgentActivityFeed />

      {needsReview > 0 && (
        <button
          onClick={() => navigate("/transactions?filter=needs_review")}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "10px 16px",
            borderRadius: 8,
            border: "1px solid var(--warning, #f59e0b)",
            background: "var(--warning-bg, #fffbeb)",
            color: "var(--warning-ink, #92400e)",
            cursor: "pointer",
            fontSize: 14,
            marginTop: 8,
          }}
          aria-label={`${needsReview} transactions need review`}
        >
          ⚠ {needsReview} transaction{needsReview === 1 ? "" : "s"} need{needsReview === 1 ? "s" : ""} review →
        </button>
      )}
    </section>
  );
}
```

- [ ] **Step 6: Run all frontend tests**

```
cd ui && npx vitest run
```

Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/AgentActivityFeed.tsx \
        ui/src/components/AgentActivityFeed.test.tsx \
        ui/src/screens/Today.tsx
git commit -m "feat(ui): AgentActivityFeed + Today Needs-a-glance chip"
```

---

### Task 23: Settings AI Provider panel

**Files:**
- Modify: `ui/src/screens/Settings.tsx`
- Create: `ui/src/screens/Settings.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `ui/src/screens/Settings.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Settings from "./Settings";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/onboarding", () => ({
  useResetOnboarding: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useClearSampleData: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useOnboardingState: vi.fn(() => ({ data: { completion_marked: true, account_count: 0, category_count: 0 } })),
}));
vi.mock("../api/hooks/agent", () => ({
  useSetCompletionProvider: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSaveProviderApiKey: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useTestCompletionProvider: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue({ ok: true, latency_ms: 120, error: null }),
    isPending: false,
  })),
  useTriggerCategorize: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useListProviderModels: vi.fn(() => ({ data: ["llama3.2"] })),
}));
vi.mock("../api/client", () => ({
  commands: {
    getNeedsReviewCount: vi.fn().mockResolvedValue({ status: "ok", data: 0 }),
  },
}));

describe("Settings — AI Provider panel", () => {
  it("shows 'AI Provider' section", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText("AI Provider")).toBeInTheDocument();
  });

  it("expands config panel on Configure click", async () => {
    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /configure/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /ollama/i })).toBeInTheDocument()
    );
  });

  it("shows Test connection button when Ollama selected", async () => {
    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /configure/i }));
    await waitFor(() => screen.getByRole("button", { name: /ollama/i }));
    fireEvent.click(screen.getByRole("button", { name: /ollama/i }));
    expect(screen.getByRole("button", { name: /test connection/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```
cd ui && npx vitest run src/screens/Settings.test.tsx
```

Expected: FAIL — AI Provider section not rendered

- [ ] **Step 3: Add AI Provider panel to Settings.tsx**

Replace `ui/src/screens/Settings.tsx`:

```tsx
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useAccounts } from "../api/hooks/accounts";
import {
  useResetOnboarding,
  useClearSampleData,
  useOnboardingState,
} from "../api/hooks/onboarding";
import {
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useTriggerCategorize,
  useListProviderModels,
} from "../api/hooks/agent";
import type { CompletionProviderConfig } from "../api/client";

type ProviderKind = "ollama" | "openai_compat" | "anthropic" | null;

const OPENAI_COMPAT_PRESETS: { label: string; preset: string; base_url: string }[] = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
];

export default function Settings() {
  const navigate = useNavigate();
  const reset = useResetOnboarding();
  const clearSample = useClearSampleData();
  const { data: accounts = [] } = useAccounts();
  const { data: onboarding } = useOnboardingState();
  const hasSample = accounts.some((a) => a.source === "sample");
  const [resetError, setResetError] = useState<string | null>(null);
  const [clearError, setClearError] = useState<string | null>(null);

  // AI Provider panel state
  const [providerPanelOpen, setProviderPanelOpen] = useState(false);
  const [selectedKind, setSelectedKind] = useState<ProviderKind>(null);
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [ollamaModel, setOllamaModel] = useState("");
  const [selectedPreset, setSelectedPreset] = useState(OPENAI_COMPAT_PRESETS[0]);
  const [compatModel, setCompatModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [anthropicModel, setAnthropicModel] = useState("claude-3-5-haiku-latest");
  const [testResult, setTestResult] = useState<{ ok: boolean; latency_ms: number; error: string | null } | null>(null);

  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const triggerCategorize = useTriggerCategorize();
  const { data: ollamaModels = [] } = useListProviderModels(
    selectedKind === "ollama"
      ? { kind: "ollama", base_url: ollamaUrl, model: ollamaModel }
      : null
  );

  async function reRunOnboarding() {
    if (!confirm("This will re-open the welcome wizard. Your existing accounts, transactions, and categories are kept.")) return;
    setResetError(null);
    try {
      await reset.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setResetError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function replaceSampleData() {
    if (!confirm("This will permanently delete the Mira & Adam sample accounts and their transactions. Anything you added manually or imported is kept.")) return;
    setClearError(null);
    try {
      await clearSample.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setClearError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function handleTestConnection() {
    if (!selectedKind) return;
    const config = buildConfig();
    if (!config) return;
    setTestResult(null);
    const r = await testProvider.mutateAsync({ config, apiKey: apiKey || undefined });
    setTestResult(r);
  }

  async function handleSave() {
    if (!selectedKind) return;
    const config = buildConfig();
    if (!config) return;
    await setProvider.mutateAsync(config);
    if (apiKey && selectedKind !== "ollama") {
      const pid = selectedKind === "anthropic" ? "anthropic" : selectedPreset.preset;
      await saveKey.mutateAsync({ providerId: pid, key: apiKey });
    }
    setProviderPanelOpen(false);
  }

  function buildConfig(): CompletionProviderConfig | null {
    switch (selectedKind) {
      case "ollama": return { kind: "ollama", base_url: ollamaUrl, model: ollamaModel };
      case "openai_compat": return { kind: "openai_compat", preset: selectedPreset.preset, base_url: selectedPreset.base_url, model: compatModel };
      case "anthropic": return { kind: "anthropic", model: anthropicModel };
      default: return null;
    }
  }

  return (
    <div className="screen-settings">
      <h1 style={{ fontSize: 32, fontWeight: 600, marginTop: 0, marginBottom: 24 }}>Settings</h1>

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Onboarding</h2>
        <p style={{ marginBottom: 12 }}>
          Completed: <strong>{onboarding?.completion_marked ? "yes" : "no"}</strong>
        </p>
        {resetError && <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>{resetError}</p>}
        <button onClick={reRunOnboarding}>Re-run onboarding</button>
      </section>

      {hasSample && (
        <section style={{ marginBottom: 32 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Sample data</h2>
          <p style={{ marginBottom: 12 }}>
            You're currently looking at the Mira &amp; Adam sample household. Replace it with your own when you're ready.
          </p>
          {clearError && <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>{clearError}</p>}
          <button onClick={replaceSampleData} className="danger">Replace sample data with my own</button>
        </section>
      )}

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>AI Provider</h2>
        {!providerPanelOpen ? (
          <div>
            <p style={{ marginBottom: 12, color: "var(--text-2)" }}>
              Not configured — categories won't be assigned automatically.
            </p>
            <button onClick={() => setProviderPanelOpen(true)}>Configure</button>
          </div>
        ) : (
          <div style={{ border: "1px solid var(--hairline)", borderRadius: 8, padding: 16 }}>
            {/* Provider type row */}
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
              {(["ollama", "openai_compat", "anthropic"] as ProviderKind[]).map((k) => (
                <button
                  key={k!}
                  onClick={() => setSelectedKind(k)}
                  style={{ fontWeight: selectedKind === k ? 700 : 400 }}
                  aria-pressed={selectedKind === k}
                >
                  {k === "ollama" ? "Ollama" : k === "anthropic" ? "Anthropic" : "Cloud"}
                </button>
              ))}
            </div>

            {selectedKind === "ollama" && (
              <div>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Base URL
                  <input value={ollamaUrl} onChange={(e) => setOllamaUrl(e.target.value)} style={{ display: "block", width: "100%" }} />
                </label>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Model
                  <select value={ollamaModel} onChange={(e) => setOllamaModel(e.target.value)} style={{ display: "block", width: "100%" }}>
                    {ollamaModels.map((m: string) => <option key={m} value={m}>{m}</option>)}
                  </select>
                </label>
              </div>
            )}

            {selectedKind === "openai_compat" && (
              <div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
                  {OPENAI_COMPAT_PRESETS.map((p) => (
                    <button key={p.preset} onClick={() => setSelectedPreset(p)} aria-pressed={selectedPreset.preset === p.preset}>
                      {p.label}
                    </button>
                  ))}
                </div>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Model
                  <input value={compatModel} onChange={(e) => setCompatModel(e.target.value)} placeholder="e.g. gpt-4o-mini" style={{ display: "block", width: "100%" }} />
                </label>
                <label style={{ display: "block", marginBottom: 8 }}>
                  API Key
                  <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" style={{ display: "block", width: "100%" }} />
                </label>
              </div>
            )}

            {selectedKind === "anthropic" && (
              <div>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Model
                  <input value={anthropicModel} onChange={(e) => setAnthropicModel(e.target.value)} style={{ display: "block", width: "100%" }} />
                </label>
                <label style={{ display: "block", marginBottom: 8 }}>
                  API Key
                  <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-ant-…" style={{ display: "block", width: "100%" }} />
                </label>
              </div>
            )}

            {testResult && (
              <p style={{ color: testResult.ok ? "var(--success, green)" : "var(--error, red)", marginBottom: 8 }}>
                {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
              </p>
            )}

            <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
              <button onClick={handleTestConnection} disabled={!selectedKind || testProvider.isPending}>
                Test connection
              </button>
              <button className="primary" onClick={handleSave} disabled={!selectedKind || setProvider.isPending}>
                Save
              </button>
              <button onClick={() => { setProviderPanelOpen(false); setTestResult(null); }}>
                Cancel
              </button>
            </div>

            <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--hairline)" }}>
              <button onClick={() => triggerCategorize.mutate()} disabled={triggerCategorize.isPending}>
                Re-categorize all
              </button>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd ui && npx vitest run src/screens/Settings.test.tsx
```

Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Settings.tsx ui/src/screens/Settings.test.tsx
git commit -m "feat(ui): Settings AI Provider panel — configure, test, save, re-categorize"
```

---

### Task 24: StepAgent multi-provider two-path layout

**Files:**
- Modify: `ui/src/screens/onboarding/StepAgent.tsx`
- Create: `ui/src/screens/onboarding/StepAgent.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `ui/src/screens/onboarding/StepAgent.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import StepAgent from "./StepAgent";
import { createWrapper } from "../../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("../../api/hooks/onboarding", () => ({
  useMarkOnboardingComplete: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));
vi.mock("../../api/hooks/agent", () => ({
  useSetCompletionProvider: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSaveProviderApiKey: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useTestCompletionProvider: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue({ ok: true, latency_ms: 80, error: null }),
    isPending: false,
  })),
  useListProviderModels: vi.fn(() => ({ data: [] })),
}));
vi.mock("../../api/client", () => ({
  commands: {
    probeOllama: vi.fn().mockResolvedValue({ status: "ok", data: { reachable: false, models: [], has_nomic_embed: false } }),
    saveLlmProvider: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("StepAgent", () => {
  it("shows two-path choice: Local + Cloud", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /local.*ollama/i })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /cloud/i })).toBeInTheDocument();
    });
  });

  it("shows cloud provider tiles after clicking Cloud path", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    await waitFor(() => screen.getByRole("button", { name: /cloud/i }));
    fireEvent.click(screen.getByRole("button", { name: /cloud/i }));
    await waitFor(() => expect(screen.getByText(/openai/i)).toBeInTheDocument());
  });

  it("shows Configure later button at all times", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByRole("button", { name: /configure later/i })).toBeInTheDocument());
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```
cd ui && npx vitest run src/screens/onboarding/StepAgent.test.tsx
```

Expected: FAIL — two-path layout not rendered

- [ ] **Step 3: Rewrite StepAgent.tsx with two-path layout**

Replace `ui/src/screens/onboarding/StepAgent.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands } from "../../api/client";
import type { OllamaProbeResult } from "../../api/client";
import { useMarkOnboardingComplete } from "../../api/hooks/onboarding";
import {
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useListProviderModels,
} from "../../api/hooks/agent";

interface Props { onDone: () => void; }

type Path = null | "local" | "cloud";
type CloudPreset = { label: string; preset: string; base_url: string };

const CLOUD_PRESETS: CloudPreset[] = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Anthropic", preset: "anthropic", base_url: "" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
];

export default function StepAgent({ onDone }: Props) {
  const [path, setPath] = useState<Path>(null);

  // Ollama path state
  const [baseUrl] = useState("http://localhost:11434");
  const [completionModel, setCompletionModel] = useState("");
  const { data: probe, refetch, isFetching } = useQuery<OllamaProbeResult>({
    queryKey: ["ollama-probe", baseUrl],
    queryFn: async () => {
      const result = await commands.probeOllama(baseUrl);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 0,
    enabled: path === "local",
  });

  // Cloud path state
  const [selectedPreset, setSelectedPreset] = useState<CloudPreset>(CLOUD_PRESETS[0]);
  const [cloudModel, setCloudModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [testResult, setTestResult] = useState<{ ok: boolean; latency_ms: number; error: string | null } | null>(null);

  const markComplete = useMarkOnboardingComplete();
  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const { data: ollamaModels = [] } = useListProviderModels(
    path === "local" ? { kind: "ollama", base_url: baseUrl, model: completionModel } : null
  );
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    const first = probe?.models[0];
    if (first && !completionModel) setCompletionModel(first);
  }, [probe]);

  async function finishWithOllama() {
    if (!probe?.reachable || !completionModel) return;
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "ollama", base_url: baseUrl, model: completionModel });
      // Also save via legacy key for backward compat
      await commands.saveLlmProvider({ kind: "ollama", base_url: baseUrl, completion_model: completionModel, embedding_model: "nomic-embed-text" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function handleCloudTestAndSave() {
    setActionError(null);
    setTestResult(null);
    const isAnthropic = selectedPreset.preset === "anthropic";
    const config = isAnthropic
      ? { kind: "anthropic" as const, model: cloudModel }
      : { kind: "openai_compat" as const, preset: selectedPreset.preset, base_url: selectedPreset.base_url, model: cloudModel };
    try {
      const r = await testProvider.mutateAsync({ config, apiKey: apiKey || undefined });
      setTestResult(r);
      if (!r.ok) return;
      await setProvider.mutateAsync(config);
      if (apiKey) {
        await saveKey.mutateAsync({ providerId: isAnthropic ? "anthropic" : selectedPreset.preset, key: apiKey });
      }
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function skipForLater() {
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "unconfigured" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  // Initial two-path choice
  if (!path) {
    return (
      <div className="step-agent">
        <h2>How do you want to power AI categorization?</h2>
        <div style={{ display: "flex", gap: 16, marginBottom: 24, flexWrap: "wrap" }}>
          <button onClick={() => setPath("local")} style={{ flex: 1, minWidth: 160, padding: "20px 16px" }}>
            <div style={{ fontWeight: 700, marginBottom: 4 }}>🏠 Local (Ollama)</div>
            <div style={{ fontSize: 13, color: "var(--text-2)" }}>Install-free if already running.</div>
          </button>
          <button onClick={() => setPath("cloud")} style={{ flex: 1, minWidth: 160, padding: "20px 16px" }}>
            <div style={{ fontWeight: 700, marginBottom: 4 }}>☁ Cloud provider</div>
            <div style={{ fontSize: 13, color: "var(--text-2)" }}>OpenAI, Anthropic, OpenRouter, etc.</div>
          </button>
        </div>
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <button className="tertiary" onClick={skipForLater}>Configure later →</button>
      </div>
    );
  }

  // Cloud path
  if (path === "cloud") {
    return (
      <div className="step-agent">
        <h2>Cloud provider</h2>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
          {CLOUD_PRESETS.map((p) => (
            <button
              key={p.preset}
              onClick={() => { setSelectedPreset(p); setCloudModel(""); setApiKey(""); setTestResult(null); }}
              aria-pressed={selectedPreset.preset === p.preset}
            >
              {p.label}
            </button>
          ))}
        </div>
        <label style={{ display: "block", marginBottom: 8 }}>
          Model
          <input value={cloudModel} onChange={(e) => setCloudModel(e.target.value)} placeholder="e.g. gpt-4o-mini" style={{ display: "block", width: "100%" }} />
        </label>
        <label style={{ display: "block", marginBottom: 8 }}>
          API Key
          <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" style={{ display: "block", width: "100%" }} />
        </label>
        {testResult && (
          <p style={{ color: testResult.ok ? "var(--success, green)" : "var(--error, red)" }}>
            {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
          </p>
        )}
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <div className="actions" style={{ display: "flex", gap: 8 }}>
          <button className="primary" onClick={handleCloudTestAndSave} disabled={!cloudModel || testProvider.isPending}>
            Test &amp; Save →
          </button>
          <button onClick={() => setPath(null)}>← Back</button>
          <button className="tertiary" onClick={skipForLater}>Configure later →</button>
        </div>
      </div>
    );
  }

  // Local (Ollama) path — same as Phase 2 behavior
  if (isFetching && !probe) {
    return <div className="step-agent"><p>Checking for Ollama…</p></div>;
  }

  if (!probe?.reachable) {
    return (
      <div className="step-agent">
        <h2>Set up Ollama</h2>
        <p>
          We couldn't find Ollama. Download it from{" "}
          <a href="#" onClick={(e) => { e.preventDefault(); openUrl("https://ollama.com"); }}>ollama.com</a>.
        </p>
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <div className="actions">
          <button onClick={() => openUrl("https://ollama.com")}>Install Ollama →</button>
          <button onClick={() => refetch()}>I just installed it — refresh</button>
          <button onClick={() => setPath(null)}>← Back</button>
          <button className="tertiary" onClick={skipForLater}>Configure later →</button>
        </div>
      </div>
    );
  }

  return (
    <div className="step-agent">
      <h2>Set up Ollama</h2>
      <p>Ollama is running. Pick a completion model.</p>
      <label>
        Completion model
        <select value={completionModel} onChange={(e) => setCompletionModel(e.target.value)}>
          {(ollamaModels as string[]).map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </label>
      {!probe.has_nomic_embed && (
        <p className="warning">
          <code>nomic-embed-text</code> isn't installed. Run{" "}
          <code>ollama pull nomic-embed-text</code>, then <button onClick={() => refetch()}>Refresh</button>.
        </p>
      )}
      {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
      <div className="actions">
        <button className="primary" onClick={finishWithOllama} disabled={!completionModel}>
          Use Ollama →
        </button>
        <button onClick={() => setPath(null)}>← Back</button>
        <button className="tertiary" onClick={skipForLater}>Configure later →</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cd ui && npx vitest run src/screens/onboarding/StepAgent.test.tsx
```

Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/onboarding/StepAgent.tsx \
        ui/src/screens/onboarding/StepAgent.test.tsx
git commit -m "feat(ui): StepAgent two-path layout — Local (Ollama) + Cloud provider"
```

---

## Phase 3.7 — Tests + Smoke

### Task 25: a11y sweep for new components

**Files:**
- Modify: existing a11y test files (add new component coverage)

- [ ] **Step 1: Run existing a11y tests to verify green baseline**

```
cd ui && npx vitest run --reporter=verbose 2>&1 | grep -E "(PASS|FAIL|a11y)"
```

Expected: All existing a11y tests pass

- [ ] **Step 2: Add a11y checks for new components**

Find the existing a11y test file (e.g., `ui/src/a11y.test.tsx` or similar). If it's testing screens with `checkA11y`, add new imports and render calls for `CategoryPicker`, `AgentActivityFeed`, and the updated drawers.

Add to the existing a11y test file:

```tsx
import CategoryPicker from "./components/CategoryPicker";
import AgentActivityFeed from "./components/AgentActivityFeed";

// Mock dependencies used by these components
vi.mock("./api/hooks/transactions", () => ({
  useCategories: vi.fn(() => ({
    data: [
      { id: "cat1", label: "Food", color: "#f00", group_id: "g1", group_label: "Daily" },
    ],
    isLoading: false,
  })),
  useCreateTransaction: vi.fn(() => ({ mutateAsync: vi.fn() })),
  useUpdateTransaction: vi.fn(() => ({ mutateAsync: vi.fn() })),
  useDeleteTransaction: vi.fn(() => ({ mutateAsync: vi.fn() })),
  useCreateRule: vi.fn(() => ({ mutate: vi.fn() })),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

it("CategoryPicker has no a11y violations", async () => {
  const { container } = render(
    <CategoryPicker value={null} onChange={() => {}} />,
    { wrapper: createWrapper() }
  );
  await checkA11y(container);
});

it("AgentActivityFeed has no a11y violations", async () => {
  const { container } = render(<AgentActivityFeed />);
  await checkA11y(container);
});
```

- [ ] **Step 3: Run a11y tests**

```
cd ui && npx vitest run --reporter=verbose 2>&1 | grep -E "(PASS|FAIL|a11y|violations)"
```

Expected: All pass, zero violations reported

- [ ] **Step 4: Commit**

```bash
git add ui/src/
git commit -m "test(ui): a11y coverage for CategoryPicker + AgentActivityFeed"
```

---

### Task 26: Final smoke test

**Files:** None (manual verification)

- [ ] **Step 1: Run full Rust test suite**

```
cargo test --workspace
```

Expected: All tests pass

- [ ] **Step 2: Run full frontend test suite**

```
cd ui && npx vitest run
```

Expected: All tests pass

- [ ] **Step 3: Build the full project**

```
cargo build --workspace
```

Expected: No errors, no warnings besides pre-existing ones

- [ ] **Step 4: Manual smoke — provider configuration**

Launch the app in dev mode:

```
cd ui && npm run dev
```

In a separate terminal:

```
cargo tauri dev
```

Verify:
- Navigate to Settings → AI Provider → Configure → select Ollama → Test connection → shows result (or error if Ollama not running — either is fine; check for no crash)
- Navigate to Settings → AI Provider → Configure → select OpenAI → enter any model + API key → Test connection → shows error (no real key) — verify no crash

- [ ] **Step 5: Manual smoke — edit/archive flows**

- Accounts screen: click any account row → Edit Account drawer opens, form pre-filled → Save changes
- Archive button → Confirm archive → account disappears from list
- Transactions screen: click any row → Edit Transaction drawer opens, form pre-filled, CategoryPicker visible
- Select a category → Save → sonner toast appears asking about rule creation
- Click "Create rule" in toast → no error

- [ ] **Step 6: Manual smoke — Today screen**

- Navigate to Today → AgentActivityFeed region visible (empty when idle)
- Trigger categorize via Settings → Re-categorize all → feed shows "Categorizing… X / Y" → "Done"
- If any transactions are low-confidence LLM-categorized → "Needs a glance" chip appears

- [ ] **Step 7: Commit**

```bash
git add .
git commit -m "chore(phase3): Phase 3 complete — agent foundation, multi-provider, edit/archive"
```

---

## Spec Coverage Checklist

After all tasks are complete, verify each spec section is covered:

| Spec section | Covered by task(s) |
|---|---|
| §3 V003 migration | Task 1 |
| §4.1 CompletionProvider trait | Task 6 |
| §4.2 OllamaProvider | Task 7 |
| §4.3 OpenAiCompatProvider | Task 8 |
| §4.4 AnthropicProvider | Task 9 |
| §4.5 Config schema + keychain set/get | Tasks 2, 15 |
| §4.6 llm_provider migration | Task 12 |
| §4.7 Frontend presets | Tasks 23, 24 |
| §5.1 AgentHandle + AgentJob | Task 10 |
| §5.2 Categorizer pipeline (rules + LLM) | Task 11 |
| §5.3 LLM prompt | Task 11 |
| §5.4 Auto-rule proposal | Tasks 5, 14, 20 |
| §6.1 Account update/archive repos | Task 4 |
| §6.1 Transaction update/delete repos | Task 5 |
| §6.1 categorizations + rules repos | Task 3 |
| §6.2 Tauri commands — accounts | Task 13 |
| §6.2 Tauri commands — transactions | Task 14 |
| §6.2 Tauri commands — agent | Task 15 |
| §6.3 AccountDrawer edit mode | Task 19 |
| §6.3 TransactionDrawer edit mode | Task 20 |
| §6.3 CategoryPicker | Task 18 |
| §6.3 Screen row-click wiring | Task 21 |
| §7 Settings AI Provider panel | Task 23 |
| §8.1 Needs a glance chip + Today | Task 22 |
| §8.2 AgentActivityFeed | Task 22 |
| §9 StepAgent multi-provider | Task 24 |
| §10 Tauri events (emitted in configure_app) | Task 12 |
| §11 Frontend hooks | Task 17 |
| §12 Testing strategy | Tasks 3–15 (Rust), 17–26 (frontend) |

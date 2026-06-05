# Backend Foundations Implementation Plan

> **Status: ✅ Complete** — all 15 tasks implemented via subagent-driven development, reviewed (per-group + final holistic), and merged to `main` on 2026-06-04. Green bar: 66 Rust lib tests, 53 frontend tests, `tsc` clean. The checkboxes below are left unticked as a historical record of the task breakdown.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land all migration-heavy TODO schema (net-worth snapshots, manual assets, liabilities, rule proposals, agent memory, transaction flags) plus repos, Tauri CRUD commands, live wiring, and tests in one clean V006–V011 migration sequence, so feature UIs can later build on stable bindings.

**Architecture:** Rust workspace. SQL + repo functions live in `finsight-core` (one file per table, returning model structs that derive `serde` + `specta::Type`). Tauri commands in `finsight-app` are thin `run(&db, …)` wrappers grouped by screen. Three live wirings: net-worth auto-record on app start, user-correction → agent_memory, categorizer → rule_proposals. Bindings regenerated once at the end.

**Tech Stack:** Rust, rusqlite/SQLCipher, refinery migrations, Tauri 2 + specta, chrono, uuid. TDD via `cargo test -p finsight-core`.

**Spec:** `docs/superpowers/specs/2026-06-04-backend-foundations-design.md`

---

## File Structure

**Migrations** (`crates/finsight-core/migrations/`): `V006`–`V011`, one per concern.

**Models** (`crates/finsight-core/src/models/`): new files `net_worth.rs`, `manual_asset.rs`, `liability.rs`, `rule_proposal.rs`, `agent_memory.rs`; extend `transaction.rs`. All re-exported from `models/mod.rs`.

**Repos** (`crates/finsight-core/src/repos/`): new files `net_worth.rs`, `manual_assets.rs`, `liabilities.rs`, `rule_proposals.rs`, `agent_memory.rs`; extend `transactions.rs`. Registered in `repos/mod.rs`.

**Commands** (`crates/finsight-app/src/commands/`): new files `assets.rs`, `insights.rs`; extend `agent.rs`, `transactions.rs`. Registered in `commands/mod.rs` and `build_specta_builder()` in `lib.rs`.

**Wiring**: `repos/transactions.rs::update` (memory), `crates/finsight-agent/src/categorizer.rs::run_job` (proposals), `crates/finsight-app/src/lib.rs` `.setup` (net worth).

**Bindings**: `ui/src/api/bindings.ts` (regenerated, never hand-edited).

---

## Task 1: Migrations V006–V011

**Files:**
- Create: `crates/finsight-core/migrations/V006__net_worth_snapshots.sql`
- Create: `crates/finsight-core/migrations/V007__manual_assets.sql`
- Create: `crates/finsight-core/migrations/V008__liabilities.sql`
- Create: `crates/finsight-core/migrations/V009__rule_proposals.sql`
- Create: `crates/finsight-core/migrations/V010__agent_memory.sql`
- Create: `crates/finsight-core/migrations/V011__transaction_flags.sql`

- [ ] **Step 1: Write the six migration files**

`V006__net_worth_snapshots.sql`:
```sql
-- V006: daily net-worth snapshots for the Today net-worth chart (§3a)
CREATE TABLE net_worth_snapshots (
  id          TEXT PRIMARY KEY,
  date        TEXT NOT NULL UNIQUE,   -- ISO date 'YYYY-MM-DD'
  total_cents INTEGER NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_nws_date ON net_worth_snapshots(date);
```

`V007__manual_assets.sql`:
```sql
-- V007: manually tracked assets (§4a)
CREATE TABLE manual_assets (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  asset_type  TEXT NOT NULL,
  value_cents INTEGER NOT NULL DEFAULT 0,
  currency    TEXT NOT NULL DEFAULT 'USD',
  notes       TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
```

`V008__liabilities.sql`:
```sql
-- V008: tracked liabilities (§4b)
CREATE TABLE liabilities (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  liability_type TEXT NOT NULL,
  balance_cents  INTEGER NOT NULL DEFAULT 0,
  limit_cents    INTEGER,
  apr_pct        REAL,
  payoff_date    TEXT,
  currency       TEXT NOT NULL DEFAULT 'USD',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
```

`V009__rule_proposals.sql`:
```sql
-- V009: agent-suggested categorization rules awaiting review (§11a)
CREATE TABLE rule_proposals (
  id          TEXT PRIMARY KEY,
  when_label  TEXT NOT NULL,
  description TEXT NOT NULL,
  pattern     TEXT NOT NULL,
  category_id TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'pending',
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_rule_proposals_status ON rule_proposals(status);
```

`V010__agent_memory.sql`:
```sql
-- V010: what the agent has learned from user corrections (§13b)
CREATE TABLE agent_memory (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL,
  description  TEXT NOT NULL,
  merchant_key TEXT,
  created_at   TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_agent_memory_key ON agent_memory(kind, merchant_key);
```

`V011__transaction_flags.sql`:
```sql
-- V011: per-transaction flags (§5d)
ALTER TABLE transactions ADD COLUMN is_reimbursable INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transactions ADD COLUMN is_split        INTEGER NOT NULL DEFAULT 0;
```

- [ ] **Step 2: Verify migrations apply cleanly via an existing test**

Run: `cargo test -p finsight-core --lib repos::scenarios`
Expected: PASS (its `fresh_db()` runs all migrations V001–V011; a malformed SQL file would fail here).

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/migrations/
git commit -m "feat(core): add V006-V011 migrations for backend foundations"
```

---

## Task 2: net_worth model + repo

**Files:**
- Create: `crates/finsight-core/src/models/net_worth.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Create: `crates/finsight-core/src/repos/net_worth.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the model**

`crates/finsight-core/src/models/net_worth.rs`:
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthPoint {
    pub date: String,
    pub total_cents: i64,
}
```

In `crates/finsight-core/src/models/mod.rs`, add the module line (after `mod categorization;`) and the re-export (after the `categorization` re-export):
```rust
mod net_worth;
```
```rust
pub use net_worth::NetWorthPoint;
```

- [ ] **Step 2: Write the failing test** (in `crates/finsight-core/src/repos/net_worth.rs`)

```rust
use crate::error::CoreResult;
use crate::models::NetWorthPoint;
use crate::repos::accounts;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("nw.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn record_snapshot_upserts_one_row_per_day() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        record_snapshot(&mut conn, 100_000).unwrap();
        record_snapshot(&mut conn, 250_000).unwrap();
        let hist = list_history(&mut conn, 30).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].total_cents, 250_000);
    }
}
```

Register the module in `crates/finsight-core/src/repos/mod.rs` (add alphabetically after `pub mod imports;`):
```rust
pub mod net_worth;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::net_worth`
Expected: FAIL to compile — `record_snapshot` / `list_history` not found.

- [ ] **Step 4: Write the implementation** (above the `#[cfg(test)]` block in `net_worth.rs`)

```rust
pub fn record_snapshot(conn: &mut Connection, total_cents: i64) -> CoreResult<()> {
    let id = Uuid::new_v4().to_string();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO net_worth_snapshots(id, date, total_cents, created_at) \
         VALUES(?1, ?2, ?3, ?4) \
         ON CONFLICT(date) DO UPDATE SET total_cents = excluded.total_cents",
        params![id, today, total_cents, now],
    )?;
    Ok(())
}

/// Sum all account balances and upsert today's snapshot. (Decision C: accounts
/// only for now — manual assets/liabilities fold in when those screens ship.)
pub fn record_today(conn: &mut Connection) -> CoreResult<()> {
    let total: i64 = accounts::list_summaries(conn)?
        .iter()
        .map(|a| a.balance_cents)
        .sum();
    record_snapshot(conn, total)
}

pub fn list_history(conn: &mut Connection, days: u32) -> CoreResult<Vec<NetWorthPoint>> {
    let cutoff = format!("-{} days", days);
    let mut stmt = conn.prepare(
        "SELECT date, total_cents FROM net_worth_snapshots \
         WHERE date >= date('now', ?1) ORDER BY date ASC",
    )?;
    let rows = stmt.query_map(params![cutoff], |r| {
        Ok(NetWorthPoint { date: r.get(0)?, total_cents: r.get(1)? })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-core --lib repos::net_worth`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/ crates/finsight-core/src/repos/
git commit -m "feat(core): net_worth snapshot repo + model"
```

---

## Task 3: manual_assets model + repo

**Files:**
- Create: `crates/finsight-core/src/models/manual_asset.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Create: `crates/finsight-core/src/repos/manual_assets.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the model**

`crates/finsight-core/src/models/manual_asset.rs`:
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualAsset {
    pub id: String,
    pub name: String,
    pub asset_type: String,
    pub value_cents: i64,
    pub currency: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewManualAsset {
    pub name: String,
    pub asset_type: String,
    pub value_cents: i64,
    pub currency: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualAssetPatch {
    pub name: Option<String>,
    pub asset_type: Option<String>,
    pub value_cents: Option<i64>,
    pub currency: Option<String>,
    pub notes: Option<Option<String>>,
}
```

In `models/mod.rs` add:
```rust
mod manual_asset;
```
```rust
pub use manual_asset::{ManualAsset, ManualAssetPatch, NewManualAsset};
```

- [ ] **Step 2: Write the failing test** (`crates/finsight-core/src/repos/manual_assets.rs`)

```rust
use crate::error::CoreResult;
use crate::models::{ManualAsset, ManualAssetPatch, NewManualAsset};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("ma.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn create_update_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let a = create(&mut conn, NewManualAsset {
            name: "House".into(), asset_type: "property".into(),
            value_cents: 50_000_000, currency: "USD".into(), notes: None,
        }).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 1);
        let updated = update(&mut conn, &a.id, ManualAssetPatch {
            value_cents: Some(52_000_000), ..Default::default()
        }).unwrap();
        assert_eq!(updated.value_cents, 52_000_000);
        delete(&mut conn, &a.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}
```

Register in `repos/mod.rs` (alphabetically):
```rust
pub mod manual_assets;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::manual_assets`
Expected: FAIL to compile — functions not found.

- [ ] **Step 4: Write the implementation** (above the test block)

```rust
pub fn list(conn: &mut Connection) -> CoreResult<Vec<ManualAsset>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, asset_type, value_cents, currency, notes, created_at, updated_at \
         FROM manual_assets ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], map_row)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn create(conn: &mut Connection, a: NewManualAsset) -> CoreResult<ManualAsset> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO manual_assets(id, name, asset_type, value_cents, currency, notes, created_at, updated_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, a.name, a.asset_type, a.value_cents, a.currency, a.notes, now],
    )?;
    Ok(ManualAsset {
        id, name: a.name, asset_type: a.asset_type, value_cents: a.value_cents,
        currency: a.currency, notes: a.notes, created_at: now.clone(), updated_at: now,
    })
}

pub fn update(conn: &mut Connection, id: &str, patch: ManualAssetPatch) -> CoreResult<ManualAsset> {
    if let Some(v) = &patch.name {
        conn.execute("UPDATE manual_assets SET name = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.asset_type {
        conn.execute("UPDATE manual_assets SET asset_type = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = patch.value_cents {
        conn.execute("UPDATE manual_assets SET value_cents = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.currency {
        conn.execute("UPDATE manual_assets SET currency = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.notes {
        conn.execute("UPDATE manual_assets SET notes = ?1 WHERE id = ?2", params![v, id])?;
    }
    conn.execute("UPDATE manual_assets SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id])?;
    get_by_id(conn, id)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM manual_assets WHERE id = ?1", params![id])?;
    Ok(())
}

fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<ManualAsset> {
    conn.query_row(
        "SELECT id, name, asset_type, value_cents, currency, notes, created_at, updated_at \
         FROM manual_assets WHERE id = ?1",
        params![id], map_row,
    ).map_err(Into::into)
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<ManualAsset> {
    Ok(ManualAsset {
        id: r.get(0)?, name: r.get(1)?, asset_type: r.get(2)?, value_cents: r.get(3)?,
        currency: r.get(4)?, notes: r.get(5)?, created_at: r.get(6)?, updated_at: r.get(7)?,
    })
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-core --lib repos::manual_assets`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/ crates/finsight-core/src/repos/
git commit -m "feat(core): manual_assets CRUD repo + model"
```

---

## Task 4: liabilities model + repo

**Files:**
- Create: `crates/finsight-core/src/models/liability.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Create: `crates/finsight-core/src/repos/liabilities.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the model**

`crates/finsight-core/src/models/liability.rs`:
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Liability {
    pub id: String,
    pub name: String,
    pub liability_type: String,
    pub balance_cents: i64,
    pub limit_cents: Option<i64>,
    pub apr_pct: Option<f64>,
    pub payoff_date: Option<String>,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewLiability {
    pub name: String,
    pub liability_type: String,
    pub balance_cents: i64,
    pub limit_cents: Option<i64>,
    pub apr_pct: Option<f64>,
    pub payoff_date: Option<String>,
    pub currency: String,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityPatch {
    pub name: Option<String>,
    pub liability_type: Option<String>,
    pub balance_cents: Option<i64>,
    pub limit_cents: Option<Option<i64>>,
    pub apr_pct: Option<Option<f64>>,
    pub payoff_date: Option<Option<String>>,
    pub currency: Option<String>,
}
```

In `models/mod.rs` add:
```rust
mod liability;
```
```rust
pub use liability::{Liability, LiabilityPatch, NewLiability};
```

- [ ] **Step 2: Write the failing test** (`crates/finsight-core/src/repos/liabilities.rs`)

```rust
use crate::error::CoreResult;
use crate::models::{Liability, LiabilityPatch, NewLiability};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("li.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn create_update_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let l = create(&mut conn, NewLiability {
            name: "Mortgage".into(), liability_type: "mortgage".into(),
            balance_cents: 30_000_000, limit_cents: Some(35_000_000),
            apr_pct: Some(5.5), payoff_date: Some("2045-01-01".into()),
            currency: "USD".into(),
        }).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 1);
        let updated = update(&mut conn, &l.id, LiabilityPatch {
            balance_cents: Some(29_500_000), ..Default::default()
        }).unwrap();
        assert_eq!(updated.balance_cents, 29_500_000);
        assert_eq!(updated.apr_pct, Some(5.5));
        delete(&mut conn, &l.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}
```

Register in `repos/mod.rs`:
```rust
pub mod liabilities;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::liabilities`
Expected: FAIL to compile — functions not found.

- [ ] **Step 4: Write the implementation** (above the test block)

```rust
pub fn list(conn: &mut Connection) -> CoreResult<Vec<Liability>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, liability_type, balance_cents, limit_cents, apr_pct, payoff_date, currency, created_at, updated_at \
         FROM liabilities ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], map_row)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn create(conn: &mut Connection, l: NewLiability) -> CoreResult<Liability> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO liabilities(id, name, liability_type, balance_cents, limit_cents, apr_pct, payoff_date, currency, created_at, updated_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![id, l.name, l.liability_type, l.balance_cents, l.limit_cents, l.apr_pct, l.payoff_date, l.currency, now],
    )?;
    get_by_id(conn, &id)
}

pub fn update(conn: &mut Connection, id: &str, patch: LiabilityPatch) -> CoreResult<Liability> {
    if let Some(v) = &patch.name {
        conn.execute("UPDATE liabilities SET name = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.liability_type {
        conn.execute("UPDATE liabilities SET liability_type = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = patch.balance_cents {
        conn.execute("UPDATE liabilities SET balance_cents = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.limit_cents {
        conn.execute("UPDATE liabilities SET limit_cents = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.apr_pct {
        conn.execute("UPDATE liabilities SET apr_pct = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.payoff_date {
        conn.execute("UPDATE liabilities SET payoff_date = ?1 WHERE id = ?2", params![v, id])?;
    }
    if let Some(v) = &patch.currency {
        conn.execute("UPDATE liabilities SET currency = ?1 WHERE id = ?2", params![v, id])?;
    }
    conn.execute("UPDATE liabilities SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id])?;
    get_by_id(conn, id)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM liabilities WHERE id = ?1", params![id])?;
    Ok(())
}

fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Liability> {
    conn.query_row(
        "SELECT id, name, liability_type, balance_cents, limit_cents, apr_pct, payoff_date, currency, created_at, updated_at \
         FROM liabilities WHERE id = ?1",
        params![id], map_row,
    ).map_err(Into::into)
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<Liability> {
    Ok(Liability {
        id: r.get(0)?, name: r.get(1)?, liability_type: r.get(2)?, balance_cents: r.get(3)?,
        limit_cents: r.get(4)?, apr_pct: r.get(5)?, payoff_date: r.get(6)?, currency: r.get(7)?,
        created_at: r.get(8)?, updated_at: r.get(9)?,
    })
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-core --lib repos::liabilities`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/ crates/finsight-core/src/repos/
git commit -m "feat(core): liabilities CRUD repo + model"
```

---

## Task 5: rule_proposals model + repo (incl. emission)

**Files:**
- Create: `crates/finsight-core/src/models/rule_proposal.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Create: `crates/finsight-core/src/repos/rule_proposals.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the model**

`crates/finsight-core/src/models/rule_proposal.rs`:
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleProposal {
    pub id: String,
    pub when_label: String,
    pub description: String,
    pub pattern: String,
    pub category_id: String,
    pub status: String,
    pub created_at: String,
}
```

In `models/mod.rs` add:
```rust
mod rule_proposal;
```
```rust
pub use rule_proposal::RuleProposal;
```

- [ ] **Step 2: Write the failing test** (`crates/finsight-core/src/repos/rule_proposals.rs`)

```rust
use crate::error::CoreResult;
use crate::models::RuleProposal;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("rp.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_three_user_corrections(conn: &mut Connection) {
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Streaming','#0f0',0)", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
        for i in 0..3 {
            let tid = format!("t{i}");
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES(?1,'a1','2024-01-01T00:00:00Z',-1500,'NETFLIX','cleared',0,'2024-01-01T00:00:00Z')",
                params![tid],
            ).unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES(?1,?2,'cat1','user',1.0,'2024-01-02T00:00:00Z')",
                params![format!("c{i}"), tid],
            ).unwrap();
        }
    }

    #[test]
    fn emit_creates_one_pending_then_dedupes() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_three_user_corrections(&mut conn);
        let n = emit_from_corrections(&mut conn, 3).unwrap();
        assert_eq!(n, 1);
        let pending = list(&mut conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].pattern, "NETFLIX");
        assert_eq!(pending[0].category_id, "cat1");
        // Re-running must not create a duplicate.
        assert_eq!(emit_from_corrections(&mut conn, 3).unwrap(), 0);
    }

    #[test]
    fn set_status_excludes_from_pending() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let p = insert(&mut conn, "Recurring", "desc", "SPOTIFY", "cat1").unwrap();
        assert_eq!(list(&mut conn, Some("pending")).unwrap().len(), 1);
        set_status(&mut conn, &p.id, "declined").unwrap();
        assert_eq!(list(&mut conn, Some("pending")).unwrap().len(), 0);
        assert!(get(&mut conn, &p.id).unwrap().is_some());
    }
}
```

Register in `repos/mod.rs`:
```rust
pub mod rule_proposals;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::rule_proposals`
Expected: FAIL to compile — functions not found.

- [ ] **Step 4: Write the implementation** (above the test block)

```rust
fn map_row(r: &rusqlite::Row) -> rusqlite::Result<RuleProposal> {
    Ok(RuleProposal {
        id: r.get(0)?, when_label: r.get(1)?, description: r.get(2)?, pattern: r.get(3)?,
        category_id: r.get(4)?, status: r.get(5)?, created_at: r.get(6)?,
    })
}

pub fn list(conn: &mut Connection, status: Option<&str>) -> CoreResult<Vec<RuleProposal>> {
    let mut out = Vec::new();
    match status {
        Some(s) => {
            let mut stmt = conn.prepare(
                "SELECT id, when_label, description, pattern, category_id, status, created_at \
                 FROM rule_proposals WHERE status = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![s], map_row)?;
            for row in rows { out.push(row?); }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, when_label, description, pattern, category_id, status, created_at \
                 FROM rule_proposals ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], map_row)?;
            for row in rows { out.push(row?); }
        }
    }
    Ok(out)
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<RuleProposal>> {
    match conn.query_row(
        "SELECT id, when_label, description, pattern, category_id, status, created_at \
         FROM rule_proposals WHERE id = ?1",
        params![id], map_row,
    ) {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn insert(conn: &mut Connection, when_label: &str, description: &str, pattern: &str, category_id: &str) -> CoreResult<RuleProposal> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO rule_proposals(id, when_label, description, pattern, category_id, status, created_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
        params![id, when_label, description, pattern, category_id, now],
    )?;
    Ok(RuleProposal {
        id, when_label: when_label.to_string(), description: description.to_string(),
        pattern: pattern.to_string(), category_id: category_id.to_string(),
        status: "pending".to_string(), created_at: now,
    })
}

pub fn set_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    conn.execute("UPDATE rule_proposals SET status = ?1 WHERE id = ?2", params![status, id])?;
    Ok(())
}

pub fn exists_pending(conn: &mut Connection, pattern: &str, category_id: &str) -> CoreResult<bool> {
    let found: bool = conn.query_row(
        "SELECT 1 FROM rule_proposals \
         WHERE lower(pattern) = lower(?1) AND category_id = ?2 AND status = 'pending' LIMIT 1",
        params![pattern, category_id],
        |_| Ok(true),
    ).or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(false),
        other => Err(other),
    })?;
    Ok(found)
}

/// Find merchants the user has manually set to the same category at least
/// `threshold` distinct times, and emit a pending proposal for each — unless an
/// enabled rule or a pending proposal already covers it. Returns count inserted.
pub fn emit_from_corrections(conn: &mut Connection, threshold: i64) -> CoreResult<usize> {
    let mut stmt = conn.prepare(
        "SELECT t.merchant_raw, ca.category_id, c.label, COUNT(DISTINCT ca.txn_id) AS n \
         FROM categorizations ca \
         JOIN transactions t ON t.id = ca.txn_id \
         JOIN categories c ON c.id = ca.category_id \
         WHERE ca.source = 'user' \
         GROUP BY lower(t.merchant_raw), ca.category_id \
         HAVING COUNT(DISTINCT ca.txn_id) >= ?1",
    )?;
    let candidates: Vec<(String, String, String, i64)> = stmt
        .query_map(params![threshold], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let mut inserted = 0usize;
    for (merchant_raw, category_id, category_label, n) in candidates {
        let rule_exists: bool = conn.query_row(
            "SELECT 1 FROM rules WHERE lower(pattern) = lower(?1) AND enabled = 1 LIMIT 1",
            params![merchant_raw],
            |_| Ok(true),
        ).or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })?;
        if rule_exists || exists_pending(conn, &merchant_raw, &category_id)? {
            continue;
        }
        let description = format!(
            "You've set \"{}\" to {} {} times — make it a rule?",
            merchant_raw, category_label, n
        );
        insert(conn, "Recurring", &description, &merchant_raw, &category_id)?;
        inserted += 1;
    }
    Ok(inserted)
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-core --lib repos::rule_proposals`
Expected: PASS (both tests).

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/ crates/finsight-core/src/repos/
git commit -m "feat(core): rule_proposals repo + correction-driven emission"
```

---

## Task 6: agent_memory model + repo

**Files:**
- Create: `crates/finsight-core/src/models/agent_memory.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Create: `crates/finsight-core/src/repos/agent_memory.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the model**

`crates/finsight-core/src/models/agent_memory.rs`:
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemory {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub merchant_key: Option<String>,
    pub created_at: String,
}
```

In `models/mod.rs` add:
```rust
mod agent_memory;
```
```rust
pub use agent_memory::AgentMemory;
```

- [ ] **Step 2: Write the failing test** (`crates/finsight-core/src/repos/agent_memory.rs`)

```rust
use crate::error::CoreResult;
use crate::models::AgentMemory;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("am.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn upsert_correction_dedupes_by_merchant() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        upsert_correction(&mut conn, "amzn mktpl", "AMZN -> Shopping (1x)").unwrap();
        upsert_correction(&mut conn, "amzn mktpl", "AMZN -> Shopping (2x)").unwrap();
        let all = list(&mut conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].description, "AMZN -> Shopping (2x)");
        assert_eq!(all[0].kind, "correction");
        forget(&mut conn, &all[0].id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}
```

Register in `repos/mod.rs` (first module, alphabetically before `accounts`? keep it alphabetical: `agent_memory` sorts before `accounts`? "agent_memory" vs "accounts" — 'g' > 'c', so after accounts). Add after `pub mod accounts;`:
```rust
pub mod agent_memory;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::agent_memory`
Expected: FAIL to compile — functions not found.

- [ ] **Step 4: Write the implementation** (above the test block)

```rust
pub fn list(conn: &mut Connection) -> CoreResult<Vec<AgentMemory>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, description, merchant_key, created_at \
         FROM agent_memory ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| Ok(AgentMemory {
        id: r.get(0)?, kind: r.get(1)?, description: r.get(2)?,
        merchant_key: r.get(3)?, created_at: r.get(4)?,
    }))?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn upsert_correction(conn: &mut Connection, merchant_key: &str, description: &str) -> CoreResult<()> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agent_memory(id, kind, description, merchant_key, created_at) \
         VALUES(?1, 'correction', ?2, ?3, ?4) \
         ON CONFLICT(kind, merchant_key) DO UPDATE SET \
            description = excluded.description, created_at = excluded.created_at",
        params![id, description, merchant_key, now],
    )?;
    Ok(())
}

pub fn forget(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM agent_memory WHERE id = ?1", params![id])?;
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-core --lib repos::agent_memory`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/models/ crates/finsight-core/src/repos/
git commit -m "feat(core): agent_memory repo + model"
```

---

## Task 7: Transaction flags (struct + SELECTs + set_flags)

**Files:**
- Modify: `crates/finsight-core/src/models/transaction.rs:32-52`
- Modify: `crates/finsight-core/src/repos/transactions.rs` (insert, list, get_by_id, add `set_flags`)

- [ ] **Step 1: Extend the Transaction struct**

In `crates/finsight-core/src/models/transaction.rs`, add two fields at the **end** of the `Transaction` struct (after `created_at`):
```rust
    pub is_anomaly: bool,
    pub created_at: DateTime<Utc>,
    pub is_reimbursable: bool,
    pub is_split: bool,
}
```

- [ ] **Step 2: Update the two SELECTs and their row closures**

In `crates/finsight-core/src/repos/transactions.rs`, the `list` SELECT (currently ending `t.is_anomaly, t.created_at`) — append the two columns:
```rust
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at, \
                t.is_reimbursable, t.is_split \
```
And in its row closure, after the `created_at: ...` field, add:
```rust
                is_reimbursable: r.get::<_, i64>(18)? != 0,
                is_split: r.get::<_, i64>(19)? != 0,
```
Apply the **identical** change to the `get_by_id` SELECT and its closure (same column list, same two `.get(18)`/`.get(19)` lines). Existing indices 0–17 are unchanged because the new columns are appended last.

- [ ] **Step 3: Update the two Transaction literals**

In `insert` (the returned `Transaction { … }`), add after `created_at: now,`:
```rust
        is_reimbursable: false,
        is_split: false,
```
The `seed()`/`update` paths build `Transaction` only via `insert`/`get_by_id`, so no other literal needs changing. (If `cargo build` later reports another `Transaction { … }` missing fields, add `is_reimbursable: false, is_split: false` there too.)

- [ ] **Step 4: Write the failing test** (append inside the existing `tests` module in `repos/transactions.rs`, before its closing `}`)

```rust
    #[test]
    fn set_flags_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let t = set_flags(&mut conn, &txn_id, true, true).unwrap();
        assert!(t.is_reimbursable);
        assert!(t.is_split);
        let cleared = set_flags(&mut conn, &txn_id, false, true).unwrap();
        assert!(!cleared.is_reimbursable);
        assert!(cleared.is_split);
    }
```

- [ ] **Step 5: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::transactions::tests::set_flags_round_trip`
Expected: FAIL to compile — `set_flags` not found.

- [ ] **Step 6: Add `set_flags`** (in `repos/transactions.rs`, after `delete`)

```rust
pub fn set_flags(conn: &mut Connection, id: &str, is_reimbursable: bool, is_split: bool) -> CoreResult<Transaction> {
    conn.execute(
        "UPDATE transactions SET is_reimbursable = ?1, is_split = ?2 WHERE id = ?3",
        params![is_reimbursable as i64, is_split as i64, id],
    )?;
    get_by_id(conn, id)
}
```

- [ ] **Step 7: Run all transactions tests to verify pass (and no regressions)**

Run: `cargo test -p finsight-core --lib repos::transactions`
Expected: PASS (existing tests + `set_flags_round_trip`).

- [ ] **Step 8: Commit**

```bash
git add crates/finsight-core/src/models/transaction.rs crates/finsight-core/src/repos/transactions.rs
git commit -m "feat(core): transaction is_reimbursable/is_split flags + set_flags"
```

---

## Task 8: Wire user corrections → agent_memory

**Files:**
- Modify: `crates/finsight-core/src/repos/transactions.rs:199-225` (the `if let Some(category_id) = cat` block in `update`)

- [ ] **Step 1: Write the failing test** (append in the `tests` module of `repos/transactions.rs`)

```rust
    #[test]
    fn user_category_change_records_agent_memory() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch { category_id: Some(Some("cat1".into())), ..Default::default() };
        update(&mut conn, &txn_id, patch).unwrap();
        let mem = crate::repos::agent_memory::list(&mut conn).unwrap();
        assert_eq!(mem.len(), 1);
        assert_eq!(mem[0].kind, "correction");
        assert!(mem[0].description.contains("AMAZON"));
        assert!(mem[0].description.contains("Food"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::transactions::tests::user_category_change_records_agent_memory`
Expected: FAIL — `mem.len()` is 0 (no memory written yet).

- [ ] **Step 3: Replace the category block in `update`**

Replace the existing block (from `if let Some(category_id) = cat {` through its closing `}`) with:
```rust
        if let Some(category_id) = cat {
            let merchant_raw: String = conn.query_row(
                "SELECT merchant_raw FROM transactions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            let category_label: String = conn.query_row(
                "SELECT label FROM categories WHERE id = ?1",
                params![category_id],
                |r| r.get(0),
            ).unwrap_or_default();

            // Record what the agent has learned from this user correction.
            let merchant_key = merchant_raw.to_lowercase();
            let user_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM categorizations ca \
                 JOIN transactions t ON t.id = ca.txn_id \
                 WHERE ca.source = 'user' AND lower(t.merchant_raw) = ?1",
                params![merchant_key],
                |r| r.get(0),
            )?;
            let memo = format!("{} → {} · you've set this {}×", merchant_raw, category_label, user_count);
            crate::repos::agent_memory::upsert_correction(conn, &merchant_key, &memo)?;

            // Propose a rule if none exists yet for this merchant.
            let rule_exists: bool = conn.query_row(
                "SELECT 1 FROM rules WHERE lower(pattern) = lower(?1) AND enabled = 1 LIMIT 1",
                params![merchant_raw],
                |_| Ok(true),
            ).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                other => Err(other),
            })?;
            if !rule_exists {
                proposed_rule = Some(ProposedRule {
                    pattern: merchant_raw,
                    category_id: category_id.clone(),
                    category_label,
                });
            }
        }
```

- [ ] **Step 4: Run the transactions tests to verify pass (no regressions)**

Run: `cargo test -p finsight-core --lib repos::transactions`
Expected: PASS (the new memory test plus the existing `update_category_*` tests — the `proposed_rule` behavior is preserved).

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/repos/transactions.rs
git commit -m "feat(core): record agent_memory on user category corrections"
```

---

## Task 9: Wire categorizer → rule_proposals emission

**Files:**
- Modify: `crates/finsight-agent/src/categorizer.rs:6-10` (use), `:144` (post-run step), tests

- [ ] **Step 1: Write the failing test** (append in the `tests` module of `categorizer.rs`)

```rust
    #[tokio::test]
    async fn emits_rule_proposal_for_repeated_user_corrections() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut *conn); // inserts cat1 + account a1 + txn t1
            // Add two more transactions for the same merchant, all user-categorized.
            for i in 2..=3 {
                let tid = format!("t{i}");
                conn.execute(
                    "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,created_at) \
                     VALUES(?1,'a1','2024-01-15T00:00:00Z',1500,'CHIPOTLE','cat1','cleared',0,'2024-01-15T00:00:00Z')",
                    rusqlite::params![tid],
                ).unwrap();
            }
            // t1 also categorized to cat1, all by the user.
            conn.execute("UPDATE transactions SET category_id='cat1' WHERE id='t1'", []).unwrap();
            for (i, tid) in ["t1","t2","t3"].iter().enumerate() {
                conn.execute(
                    "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                     VALUES(?1,?2,'cat1','user',1.0,'2024-01-16T00:00:00Z')",
                    rusqlite::params![format!("uc{i}"), tid],
                ).unwrap();
            }
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(), model_id: "test".into(), response: json!([]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {})).await.unwrap();

        let mut conn = db.get().unwrap();
        let pending = finsight_core::repos::rule_proposals::list(&mut conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].pattern, "CHIPOTLE");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-agent --lib categorizer::tests::emits_rule_proposal_for_repeated_user_corrections`
Expected: FAIL — `pending.len()` is 0 (no emission step yet).

- [ ] **Step 3: Add the import**

In `crates/finsight-agent/src/categorizer.rs`, change the repos import (line 8):
```rust
    repos::{categorizations, rule_proposals, rules},
```

- [ ] **Step 4: Add the post-run emission step**

In `run_job`, immediately before `let final_skipped = total.saturating_sub(categorized);`, insert:
```rust
    // Post-run: surface rule proposals for merchants the user keeps re-categorizing.
    {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            rule_proposals::emit_from_corrections(&mut conn, 3)?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
    }

```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-agent --lib categorizer`
Expected: PASS (new test + existing `rule_pass_*` / `llm_pass_*`).

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-agent/src/categorizer.rs
git commit -m "feat(agent): emit rule_proposals after categorize run"
```

---

## Task 10: Wire net-worth auto-record on app start

**Files:**
- Modify: `crates/finsight-app/src/lib.rs:185-188` (after `migrate_provider_settings`)

- [ ] **Step 1: Add the best-effort snapshot call**

In `crates/finsight-app/src/lib.rs`, in the `.setup` closure, immediately after the `migrate_provider_settings(&db)…?;` block, add:
```rust
            // Best-effort: record today's net-worth snapshot on startup.
            if let Ok(mut conn) = db.get() {
                let _ = finsight_core::repos::net_worth::record_today(&mut conn);
            }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p finsight-app`
Expected: builds with no errors. (Startup wiring is verified by compilation + the `net_worth` repo tests; it is not unit-tested because it runs inside Tauri's `.setup`.)

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-app/src/lib.rs
git commit -m "feat(app): record net-worth snapshot on startup"
```

---

## Task 11: Commands — assets.rs (manual assets, liabilities, net worth)

**Files:**
- Create: `crates/finsight-app/src/commands/assets.rs`
- Modify: `crates/finsight-app/src/commands/mod.rs`
- Modify: `crates/finsight-app/src/lib.rs` (register in `build_specta_builder()`)

- [ ] **Step 1: Write the command module**

`crates/finsight-app/src/commands/assets.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{
    Liability, LiabilityPatch, ManualAsset, ManualAssetPatch, NetWorthPoint, NewLiability,
    NewManualAsset,
};
use finsight_core::repos::{liabilities, manual_assets, net_worth, run};

#[tauri::command]
#[specta::specta]
pub async fn list_manual_assets(state: tauri::State<'_, AppState>) -> AppResult<Vec<ManualAsset>> {
    let db = (*state.db).clone();
    run(&db, manual_assets::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_manual_asset(
    state: tauri::State<'_, AppState>,
    input: NewManualAsset,
) -> AppResult<ManualAsset> {
    let db = (*state.db).clone();
    run(&db, move |conn| manual_assets::create(conn, input)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_manual_asset(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: ManualAssetPatch,
) -> AppResult<ManualAsset> {
    let db = (*state.db).clone();
    run(&db, move |conn| manual_assets::update(conn, &id, patch)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_manual_asset(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| manual_assets::delete(conn, &id)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_liabilities(state: tauri::State<'_, AppState>) -> AppResult<Vec<Liability>> {
    let db = (*state.db).clone();
    run(&db, liabilities::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_liability(
    state: tauri::State<'_, AppState>,
    input: NewLiability,
) -> AppResult<Liability> {
    let db = (*state.db).clone();
    run(&db, move |conn| liabilities::create(conn, input)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_liability(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: LiabilityPatch,
) -> AppResult<Liability> {
    let db = (*state.db).clone();
    run(&db, move |conn| liabilities::update(conn, &id, patch)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_liability(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| liabilities::delete(conn, &id)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn record_net_worth_snapshot(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, net_worth::record_today).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_net_worth_history(
    state: tauri::State<'_, AppState>,
    days: u32,
) -> AppResult<Vec<NetWorthPoint>> {
    let db = (*state.db).clone();
    run(&db, move |conn| net_worth::list_history(conn, days)).await.map_err(AppError::from)
}
```

- [ ] **Step 2: Register the module**

In `crates/finsight-app/src/commands/mod.rs`, add (alphabetically, after `pub mod agent;`):
```rust
pub mod assets;
```

- [ ] **Step 3: Register the commands**

In `crates/finsight-app/src/lib.rs`, inside `collect_commands![…]` (before the closing `])`, after `commands::transactions::get_transaction_count,`):
```rust
        commands::assets::list_manual_assets,
        commands::assets::create_manual_asset,
        commands::assets::update_manual_asset,
        commands::assets::delete_manual_asset,
        commands::assets::list_liabilities,
        commands::assets::create_liability,
        commands::assets::update_liability,
        commands::assets::delete_liability,
        commands::assets::record_net_worth_snapshot,
        commands::assets::list_net_worth_history,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p finsight-app`
Expected: builds cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/ crates/finsight-app/src/lib.rs
git commit -m "feat(app): asset/liability/net-worth commands"
```

---

## Task 12: Commands — insights.rs (agent_memory)

**Files:**
- Create: `crates/finsight-app/src/commands/insights.rs`
- Modify: `crates/finsight-app/src/commands/mod.rs`
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Write the command module**

`crates/finsight-app/src/commands/insights.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::AgentMemory;
use finsight_core::repos::{agent_memory, run};

#[tauri::command]
#[specta::specta]
pub async fn list_agent_memory(state: tauri::State<'_, AppState>) -> AppResult<Vec<AgentMemory>> {
    let db = (*state.db).clone();
    run(&db, agent_memory::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn forget_agent_memory(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| agent_memory::forget(conn, &id)).await.map_err(AppError::from)
}
```

- [ ] **Step 2: Register the module**

In `commands/mod.rs`, add (after `pub mod import;`):
```rust
pub mod insights;
```

- [ ] **Step 3: Register the commands**

In `lib.rs` `collect_commands![…]`, after the assets block:
```rust
        commands::insights::list_agent_memory,
        commands::insights::forget_agent_memory,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p finsight-app`
Expected: builds cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/ crates/finsight-app/src/lib.rs
git commit -m "feat(app): agent_memory commands"
```

---

## Task 13: Commands — rule proposals (extend agent.rs)

**Files:**
- Modify: `crates/finsight-app/src/commands/agent.rs:11` (use) and end of file (new commands)
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Extend imports**

In `crates/finsight-app/src/commands/agent.rs`, replace the repos/models imports (line 11) with:
```rust
use finsight_core::models::{NewRule, RuleProposal};
use finsight_core::repos::{rule_proposals, rules, run};
```

- [ ] **Step 2: Add the three commands** (append at end of `agent.rs`)

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_rule_proposals(state: tauri::State<'_, AppState>) -> AppResult<Vec<RuleProposal>> {
    let db = (*state.db).clone();
    run(&db, |conn| rule_proposals::list(conn, Some("pending")))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn accept_rule_proposal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        if let Some(p) = rule_proposals::get(conn, &id)? {
            rules::insert(conn, NewRule {
                pattern: p.pattern,
                category_id: p.category_id,
                source: "agent".to_string(),
            })?;
            rule_proposals::set_status(conn, &id, "accepted")?;
        }
        Ok(())
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn decline_rule_proposal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| rule_proposals::set_status(conn, &id, "declined"))
        .await
        .map_err(AppError::from)
}
```

- [ ] **Step 3: Register the commands**

In `lib.rs` `collect_commands![…]`, after the insights block:
```rust
        commands::agent::list_rule_proposals,
        commands::agent::accept_rule_proposal,
        commands::agent::decline_rule_proposal,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p finsight-app`
Expected: builds cleanly. (If the existing `agent.rs` already imported `run` via a different path, the new `use … run` may produce a duplicate-import error — if so, merge into one `use` line.)

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/lib.rs
git commit -m "feat(app): rule proposal list/accept/decline commands"
```

---

## Task 14: Command — set_transaction_flags (extend transactions.rs)

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs` (new command)
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Add the command** (append at end of `crates/finsight-app/src/commands/transactions.rs`)

```rust
#[tauri::command]
#[specta::specta]
pub async fn set_transaction_flags(
    state: tauri::State<'_, AppState>,
    id: String,
    is_reimbursable: bool,
    is_split: bool,
) -> AppResult<finsight_core::models::Transaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::set_flags(conn, &id, is_reimbursable, is_split))
        .await
        .map_err(AppError::from)
}
```
(`transactions`, `run`, `AppError`, `AppState` are already imported in this file.)

- [ ] **Step 2: Register the command**

In `lib.rs` `collect_commands![…]`, after the rule-proposal block:
```rust
        commands::transactions::set_transaction_flags,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p finsight-app`
Expected: builds cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs crates/finsight-app/src/lib.rs
git commit -m "feat(app): set_transaction_flags command"
```

---

## Task 15: Regenerate bindings + full verification

**Files:**
- Modify: `ui/src/api/bindings.ts` (generated)

- [ ] **Step 1: Regenerate TypeScript bindings** (from repo root)

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: completes; `git status` shows `ui/src/api/bindings.ts` modified with the new command wrappers and types (`ManualAsset`, `Liability`, `NetWorthPoint`, `RuleProposal`, `AgentMemory`, etc.), and `Transaction` gains `is_reimbursable` / `is_split`.

- [ ] **Step 2: Type-check the frontend**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors (no consumers yet; the regenerated types must still compile).

- [ ] **Step 3: Run the full Rust suite**

Run: `cargo test --workspace`
Expected: all tests pass (new repo/wiring tests + pre-existing). Note: `keychain::tests::set_key_round_trip` is known-flaky on Windows under parallelism — a failure *only* there is pre-existing and unrelated.

- [ ] **Step 4: Run the full frontend suite**

Run: `cd ui && npx vitest run`
Expected: all tests pass (unchanged — 53).

- [ ] **Step 5: Commit**

```bash
git add ui/src/api/bindings.ts
git commit -m "chore(bindings): regenerate for backend foundations commands"
```

---

## Self-Review

**Spec coverage:**
- Migrations V006–V011 → Task 1. ✓
- net_worth_snapshots repo + record/list → Task 2; auto-record on start → Task 10; commands → Task 11. ✓
- manual_assets repo → Task 3; CRUD commands → Task 11. ✓
- liabilities repo → Task 4; CRUD commands → Task 11. ✓
- rule_proposals repo + emission → Task 5; categorizer wiring → Task 9; list/accept/decline commands → Task 13. ✓
- agent_memory repo → Task 6; correction wiring → Task 8; commands → Task 12. ✓
- transaction flags (struct/SELECTs/set_flags) → Task 7; command → Task 14. ✓
- Decision A (memory dedupe by merchant_key) → Task 6 upsert + unique index in Task 1. ✓
- Decision B (threshold 3) → Task 5 `emit_from_corrections(_, 3)` call site in Task 9. ✓
- Decision C (accounts-only net worth) → Task 2 `record_today`. ✓
- Bindings regen + green bar → Task 15. ✓
- Transaction struct ripple risk → Task 7 Steps 2–3 (append-last keeps indices stable). ✓

**Placeholder scan:** No TBD/TODO/"handle edge cases" — every code step contains full code. ✓

**Type consistency:**
- `record_snapshot`/`record_today`/`list_history` (Task 2) reused verbatim in Tasks 10/11. ✓
- `emit_from_corrections(conn, i64)` (Task 5) called with `3` in Task 9. ✓
- `upsert_correction(conn, &str, &str)` (Task 6) called in Task 8. ✓
- `set_flags(conn, &str, bool, bool) -> Transaction` (Task 7) called in Task 14. ✓
- `rule_proposals::get/list/set_status/insert` signatures (Task 5) match call sites in Tasks 9/13. ✓
- Model field names (`camelCase` via serde) consistent between model defs and repo `map_row`/literals. ✓

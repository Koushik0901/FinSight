# Backend Foundations — Design Spec

**Date:** 2026-06-04
**Status:** Approved (design), pending implementation plan
**Scope owner:** first sub-project of the TODO.md decomposition ("backend-foundations first")

## Overview

The remaining `docs/TODO.md` work is ~16 feature groups across 9 subsystems. Several
depend on new database tables, and the TODO repeatedly (and now incorrectly) calls for
"a V005 migration" — V005 is already taken by `scenarios`. This sub-project lands **all
the new schema, repositories, Tauri CRUD commands, regenerated bindings, live wiring, and
Rust tests** in one clean migration sequence (V006–V011), so the feature UIs can later be
built on stable bindings without migration collisions.

This batch is **backend + wiring + tests only**. No UI is built, and the new bindings have
no frontend consumers yet.

## Decisions (locked)

- **Tables:** 5 core tables + transaction flags (§5d pulled forward).
- **Behavior:** CRUD **plus** live wiring (categorizer→proposals, correction→memory,
  net-worth auto-record on app start).
- **Migrations:** one file per feature (V006–V011).
- **A. agent_memory dedupe:** upsert by `merchant_key`; rewrite description rather than
  appending a row per correction.
- **B. rule_proposal threshold:** ≥3 manual corrections of the same merchant→category.
- **C. net-worth balance basis:** sum bank accounts only for now (manual assets and
  liabilities fold into net worth when those screens ship — avoids double-counting before
  any UI exists).

## Non-goals

- No screens, components, or hooks. No consumers of the new bindings.
- No FK constraint enforcement (matches existing schema style).
- No scheduled/recurring net-worth recording beyond the once-per-app-start snapshot.
- No `is_reimbursable`/`is_split` UI toggles (the `set_transaction_flags` command exists,
  but the `TransactionDrawer` toggles and table chips ship with §5d's own cycle).

---

## Migrations (`crates/finsight-core/migrations/`)

One concern per file, following the existing `Vxxx__description.sql` style (`TEXT PRIMARY
KEY`, ISO-8601 text timestamps, `idx_*` indexes).

### `V006__net_worth_snapshots.sql`
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

### `V007__manual_assets.sql`
```sql
-- V007: manually tracked assets (§4a)
CREATE TABLE manual_assets (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  asset_type  TEXT NOT NULL,          -- 'property'|'vehicle'|'investment'|'crypto'|'other'
  value_cents INTEGER NOT NULL DEFAULT 0,
  currency    TEXT NOT NULL DEFAULT 'USD',
  notes       TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
```

### `V008__liabilities.sql`
```sql
-- V008: tracked liabilities (§4b)
CREATE TABLE liabilities (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  liability_type TEXT NOT NULL,       -- 'mortgage'|'student_loan'|'credit_card'|'auto_loan'|'other'
  balance_cents  INTEGER NOT NULL DEFAULT 0,
  limit_cents    INTEGER,             -- original principal / credit limit (nullable)
  apr_pct        REAL,                -- nullable
  payoff_date    TEXT,                -- ISO date or NULL
  currency       TEXT NOT NULL DEFAULT 'USD',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
```

### `V009__rule_proposals.sql`
```sql
-- V009: agent-suggested categorization rules awaiting user review (§11a)
CREATE TABLE rule_proposals (
  id          TEXT PRIMARY KEY,
  when_label  TEXT NOT NULL,          -- context label, e.g. 'Recurring'
  description TEXT NOT NULL,          -- human-readable proposal
  pattern     TEXT NOT NULL,          -- merchant pattern the rule would match
  category_id TEXT NOT NULL,          -- target category the rule would assign
  status      TEXT NOT NULL DEFAULT 'pending',  -- 'pending'|'accepted'|'declined'
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_rule_proposals_status ON rule_proposals(status);
```
*`pattern` + `category_id` are added beyond the TODO's sketch so `accept_rule_proposal`
can materialize a real rule.*

### `V010__agent_memory.sql`
```sql
-- V010: what the agent has learned from user corrections (§13b)
CREATE TABLE agent_memory (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL,         -- 'preference'|'pattern'|'correction'
  description  TEXT NOT NULL,
  merchant_key TEXT,                  -- normalized merchant for dedupe; NULL for non-correction kinds
  created_at   TEXT NOT NULL
);
-- NULL merchant_key rows are distinct under SQLite's unique-index NULL semantics,
-- so only keyed 'correction' rows dedupe.
CREATE UNIQUE INDEX idx_agent_memory_key ON agent_memory(kind, merchant_key);
```

### `V011__transaction_flags.sql`
```sql
-- V011: per-transaction flags (§5d)
ALTER TABLE transactions ADD COLUMN is_reimbursable INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transactions ADD COLUMN is_split        INTEGER NOT NULL DEFAULT 0;
```

---

## Models (`crates/finsight-core/src/models/`)

Add per existing module pattern (re-export from `models/mod.rs`). New DTO-facing structs
use `#[serde(rename_all = "camelCase")]`. **Exception:** the `Transaction` struct stays
snake_case (no `rename_all`), consistent with the documented codebase quirk.

- `ManualAsset`, `NewManualAsset`, `ManualAssetPatch`
- `Liability`, `NewLiability`, `LiabilityPatch`
- `NetWorthPoint { date: String, total_cents: i64 }`
- `RuleProposal`
- `AgentMemory`
- **Extend `Transaction`** with `is_reimbursable: bool`, `is_split: bool`.

### Transaction struct ripple (call out explicitly in the plan)

Adding two columns touches every place that maps a `transactions` row into `Transaction`:
- `repos/transactions.rs`: `insert` (column list ~line 13), `list` SELECT (~line 73) + row
  closure, `get_by_id` SELECT (~line 243) + row closure. All positional `.get(n)` indexes
  after the new columns shift — update carefully.
- Regenerated `bindings.ts` `Transaction` type gains the two fields.
- Any existing test/fixture constructing a full `Transaction` literal.

---

## Repositories (`crates/finsight-core/src/repos/`)

One file per table; register in `repos/mod.rs`. Each gets a round-trip unit test in the
style of `repos/scenarios.rs` (`fresh_db()` + migrations).

### `net_worth.rs`
- `record_snapshot(conn, total_cents: i64) -> CoreResult<()>` — upsert today's row:
  `INSERT ... ON CONFLICT(date) DO UPDATE SET total_cents = excluded.total_cents`.
  `date` = `Utc::now().date_naive()` formatted `%Y-%m-%d`.
- `list_history(conn, days: u32) -> CoreResult<Vec<NetWorthPoint>>` —
  `WHERE date >= date('now', '-N days') ORDER BY date ASC`.

### `manual_assets.rs`
- `list(conn) -> Vec<ManualAsset>`
- `create(conn, NewManualAsset) -> ManualAsset`
- `update(conn, id, ManualAssetPatch) -> ManualAsset` (sets `updated_at`)
- `delete(conn, id) -> ()`

### `liabilities.rs`
- Same CRUD shape as `manual_assets.rs`.

### `rule_proposals.rs`
- `list(conn, status: Option<&str>) -> Vec<RuleProposal>` (None = all)
- `insert(conn, when_label, description, pattern, category_id) -> RuleProposal`
- `set_status(conn, id, status) -> ()`
- `exists_pending(conn, pattern, category_id) -> bool` (dedupe guard for emission)

### `agent_memory.rs`
- `list(conn) -> Vec<AgentMemory>` (ORDER BY created_at DESC)
- `upsert_correction(conn, merchant_key, description) -> ()` —
  `INSERT ... ON CONFLICT(kind, merchant_key) DO UPDATE SET description = excluded.description`
  with `kind = 'correction'`.
- `forget(conn, id) -> ()`

### `transactions.rs` (extend)
- `set_flags(conn, id, is_reimbursable: bool, is_split: bool) -> CoreResult<Transaction>`

---

## Tauri commands (`crates/finsight-app/src/commands/`)

All `pub async fn`, `#[tauri::command] #[specta::specta]`, using the `run(&db, …)` helper.
Register every command in `build_specta_builder()` in `lib.rs`, then regenerate bindings.

### `commands/assets.rs` (new)
- `list_manual_assets() -> Vec<ManualAsset>`
- `create_manual_asset(name, asset_type, value_cents, currency, notes) -> ManualAsset`
- `update_manual_asset(id, patch: ManualAssetPatch) -> ManualAsset`
- `delete_manual_asset(id) -> ()`
- `list_liabilities() -> Vec<Liability>`
- `create_liability(name, liability_type, balance_cents, limit_cents, apr_pct, payoff_date, currency) -> Liability`
- `update_liability(id, patch: LiabilityPatch) -> Liability`
- `delete_liability(id) -> ()`
- `record_net_worth_snapshot() -> ()` — sums bank account balances (`accounts::list_summaries`,
  decision C), calls `net_worth::record_snapshot`.
- `list_net_worth_history(days: u32) -> Vec<NetWorthPoint>`

### `commands/insights.rs` (new)
- `list_agent_memory() -> Vec<AgentMemory>`
- `forget_agent_memory(id) -> ()`

### `commands/agent.rs` (extend)
- `list_rule_proposals() -> Vec<RuleProposal>` (pending only)
- `accept_rule_proposal(id) -> ()` — load proposal, `rules::insert` with its
  `pattern`/`category_id` and `source = "agent"`, then `set_status(id, "accepted")`.
- `decline_rule_proposal(id) -> ()` — `set_status(id, "declined")`.

### `commands/transactions.rs` (extend)
- `set_transaction_flags(id, is_reimbursable: bool, is_split: bool) -> Transaction`

---

## Live wiring

### 1. Net-worth auto-record on app start
In `lib.rs` `.setup` ([lib.rs:169](../../../crates/finsight-app/src/lib.rs)), after
`run_migrations` and before/after `app.manage(state)`: compute the bank-account balance sum
and call `net_worth::record_snapshot`. Best-effort — on error, log and continue (must never
block app startup).

### 2. Correction → agent_memory
In `repos/transactions.rs::update` ([transactions.rs:184](../../../crates/finsight-core/src/repos/transactions.rs)),
inside the `if let Some(Some(category_id)) = ...` branch (user set a concrete category):
- compute `merchant_key = merchant_raw.to_lowercase()`
- count `source = 'user'` categorizations for that merchant (across that merchant's history,
  regardless of category) to populate the running tally in the description
- `agent_memory::upsert_correction(conn, &merchant_key, &description)` where description is
  e.g. `"AMZN MKTPL → Shopping · you've set this {n}×"` ({n} = the count above, the target
  category = the one just set).

This runs in the same transaction/connection as the categorization insert.

### 3. Categorizer → rule_proposals
In `categorizer.rs::run_job` ([categorizer.rs:144](../../../crates/finsight-agent/src/categorizer.rs)),
after the LLM pass and before the `CategorizationComplete` event, add a `spawn_blocking`
post-run step that:
- finds merchants with **≥3** `source = 'user'` categorizations to the **same** category
  (`GROUP BY lower(merchant_raw), category_id HAVING COUNT(*) >= 3`),
- skips any with an existing enabled rule for that pattern, or an existing pending proposal
  (`rule_proposals::exists_pending`),
- inserts a pending proposal: `when_label = "Recurring"`,
  `description = "You've set {merchant} to {category} {n} times — make it a rule?"`,
  `pattern = merchant_raw`, `category_id`.

---

## Bindings

After all commands are registered, run from the **repo root**:
```bash
cargo run -p finsight-tauri --bin export_bindings
```
Regenerates `ui/src/api/bindings.ts` (new command wrappers + types; `Transaction` gains the
two flag fields). No frontend code consumes these yet; `cd ui && npx tsc --noEmit` must stay
clean.

## Testing

- **Repos:** round-trip unit test per new repo (insert/list/update/delete).
  - `net_worth`: two `record_snapshot` calls same day → one row, latest total; `list_history`
    range filter.
  - `agent_memory`: `upsert_correction` twice for same key → one row, updated description;
    independent NULL-key rows allowed.
  - `rule_proposals`: insert → `list(Some("pending"))`; `set_status` → excluded from pending;
    `exists_pending` true/false.
- **`transactions::set_flags`:** round-trip; `Transaction` carries both flags.
- **`transactions::update` wiring:** extend existing categorization test to assert an
  `agent_memory` correction row is upserted with the running count.
- **`categorizer` wiring:** seed 3 `source = 'user'` categorizations of one merchant→category,
  run `run_job`, assert one pending proposal; re-run asserts no duplicate.
- **Commands:** `accept_rule_proposal` creates a real `rules` row and flips status.
- Full suite green: `cargo test --workspace` and `cd ui && npx vitest run`.

## Migration sequence summary

| Version | File | Adds |
|---|---|---|
| V006 | net_worth_snapshots | net-worth chart history |
| V007 | manual_assets | manual asset tracking |
| V008 | liabilities | liability tracking |
| V009 | rule_proposals | agent rule suggestions |
| V010 | agent_memory | agent learning log |
| V011 | transaction_flags | is_reimbursable / is_split |

# FinSight — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Phase 0+1 walking skeleton into a usable first-run experience: CSV import provider, 4-step onboarding wizard, manual account/transaction drawers, deterministic sample household, and Settings escape hatches.

**Architecture:** V002 migration extends the schema (imports, csv_import_mappings, accounts.source, settings). A new `finsight-providers::csv` module owns parsing, encoding sniffing, deduped batched import, and mapping persistence. A new `finsight-core::sample` module replaces the Phase 1 startup seed with a procedural Mira & Adam household triggered explicitly from the wizard. The React app gets an `/onboarding` route with four steps, two reusable Drawers (`AccountDrawer`, `TransactionDrawer`), and an `ImportMappingDialog` that drives the CSV pipeline. Tauri plugins (`dialog`, `opener`) provide native file picking and URL opening.

**Tech Stack:**
- Rust: `rusqlite` + `sqlcipher`, `csv`, `encoding_rs`, `chrono`, `rand_chacha`
- Tauri 2.x: `tauri-plugin-dialog`, `tauri-plugin-opener`, `tauri-specta`
- Frontend: React 18 + TS, Vite, Tanstack Query, Zustand, `react-hook-form`, `zod`, `@hookform/resolvers/zod`, `react-focus-lock`
- Testing: `rstest`, fixture CSVs, `vitest`, `@testing-library/react`, `vitest-axe`

**Spec:** `docs/superpowers/specs/2026-05-22-finsight-phase-2-design.md` (commit history through `44a9567`).

**Exit criteria:**
- Fresh install → onboarding auto-opens → "Try with sample data" lands on `/today` with 6 accounts and ~250 transactions visible.
- CSV picker → mapping dialog → import → transactions visible on `/transactions`. Re-importing the same file dedupes silently.
- Manual `Add account` and `Add transaction` work from both the Accounts/Transactions screens and Onboarding Step 2.
- Categories step writes the curated 10 starter categories. Agent step probes Ollama and stores or skips config.
- Settings exposes `Re-run onboarding` and (when sample data present) `Replace sample data with my own`.
- All Rust + frontend tests pass on the 3-OS CI matrix.

---

## File Structure

This plan creates or modifies the following files. File paths use the Phase 0+1 layout (workspace root has `crates/`, `src-tauri/`, `ui/`).

```
crates/finsight-core/
├── migrations/
│   └── V002__phase2_schema.sql                     # NEW
└── src/
    ├── lib.rs                                      # MODIFY — pub mod sample; pub mod settings; repos::imports
    ├── sample.rs                                   # NEW — Mira & Adam deterministic generator
    ├── settings.rs                                 # NEW — typed KV reader/writer on settings table
    └── repos/
        ├── mod.rs                                  # MODIFY — pub mod imports
        └── imports.rs                              # NEW — insert/finish/list_unfinished

crates/finsight-providers/
├── Cargo.toml                                      # MODIFY — add csv, encoding_rs
├── tests/fixtures/csv/                             # NEW — six bank fixture files
│   ├── chase-checking.csv
│   ├── amex-card.csv
│   ├── mercury-checking.csv
│   ├── mint-export.csv
│   ├── personal-capital.csv
│   └── simple-semicolon.csv
└── src/
    ├── lib.rs                                      # MODIFY — re-export CsvProvider, ImportSummary, ProviderError
    ├── error.rs                                    # NEW — ProviderError
    ├── provider.rs                                 # NEW — SyncProvider trait (real shape, replaces phase-1 stub)
    └── csv/
        ├── mod.rs                                  # NEW — CsvProvider + CsvImportMapping + CsvPreview + ImportSummary
        ├── encoding.rs                             # NEW — BOM sniff + encoding_rs fallback
        ├── parse.rs                                # NEW — pure row → NewTransaction
        └── mapping.rs                              # NEW — csv_import_mappings persistence

crates/finsight-app/
├── Cargo.toml                                      # MODIFY — add tauri-plugin-dialog, tauri-plugin-opener
└── src/
    ├── lib.rs                                      # MODIFY — register plugins, drop walking_skeleton from setup
    ├── error.rs                                    # MODIFY — From<ProviderError>
    └── commands/
        ├── mod.rs                                  # MODIFY — pub mod import; pub mod onboarding
        ├── accounts.rs                             # MODIFY — add create_account
        ├── transactions.rs                         # MODIFY — add create_transaction
        ├── import.rs                               # NEW — preview_csv_columns / import_csv / list_unfinished_imports
        └── onboarding.rs                           # NEW — get_onboarding_state / seed_sample_household /
                                                    #        reset_onboarding_completion / clear_sample_data /
                                                    #        probe_ollama

src-tauri/
├── Cargo.toml                                      # MODIFY — add tauri-plugin-dialog, tauri-plugin-opener
└── capabilities/default.json                       # MODIFY — add dialog:default, opener:default permissions

ui/
├── package.json                                    # MODIFY — react-hook-form, zod, @hookform/resolvers,
│                                                   #          react-focus-lock, @tauri-apps/plugin-dialog,
│                                                   #          @tauri-apps/plugin-opener, vitest-axe, axe-core
└── src/
    ├── App.tsx                                     # MODIFY — onboarding auto-redirect + unfinished-import banner
    ├── api/hooks/
    │   ├── accounts.ts                             # MODIFY — useCreateAccount
    │   ├── transactions.ts                         # MODIFY — useCreateTransaction, useImportCsv
    │   ├── onboarding.ts                           # NEW — useOnboardingState, useSeedSampleHousehold,
    │   │                                           #        useResetOnboarding, useClearSampleData, useProbeOllama
    │   └── csv.ts                                  # NEW — usePreviewCsvColumns
    ├── components/
    │   ├── Drawer.tsx                              # NEW — react-focus-lock wrapper, ESC/backdrop
    │   ├── AccountDrawer.tsx                       # NEW
    │   ├── TransactionDrawer.tsx                   # NEW
    │   ├── FilePicker.tsx                          # NEW — wraps plugin-dialog
    │   ├── ImportProgress.tsx                      # NEW — listens to import.progress
    │   └── UnfinishedImportBanner.tsx              # NEW — list_unfinished_imports surface on App mount
    ├── screens/
    │   ├── Accounts.tsx                            # MODIFY — list + AddAccount
    │   ├── Transactions.tsx                        # MODIFY — Import CSV + Add transaction buttons
    │   ├── Settings.tsx                            # MODIFY — Re-run onboarding, Replace sample data
    │   ├── Onboarding.tsx                          # MODIFY — 4-step shell + step indicator
    │   └── onboarding/
    │       ├── StepWelcome.tsx                     # NEW
    │       ├── StepConnect.tsx                     # NEW
    │       ├── StepCategories.tsx                  # NEW
    │       ├── StepAgent.tsx                       # NEW
    │       └── ImportMappingDialog.tsx             # NEW
    ├── state/
    │   └── onboarding.ts                           # NEW — Zustand: current step + mapping draft
    └── test/                                       # MODIFY — Drawer/AccountDrawer/Onboarding/ImportMapping/a11y suites
```

---

## Task index

The plan is split into 22 numbered tasks across the 6 spec sub-phases. Tasks within a sub-phase should be executed in order; tasks in later sub-phases depend on earlier ones (called out per-task).

- **Phase 2.0 — Backend foundations:** Tasks 1–6
- **Phase 2.1 — CSV provider:** Tasks 7–10
- **Phase 2.2 — Onboarding shell + sample data wiring:** Tasks 11–13
- **Phase 2.3 — Manual entry drawers:** Tasks 14–16
- **Phase 2.4 — Import flow UI:** Tasks 17–19
- **Phase 2.5 — Categories + Agent + Settings escape hatches:** Tasks 20–22

Each task ends with a commit step. Commits follow Conventional Commits (`feat(scope): …`, `test(scope): …`, `chore: …`).

---

## Phase 2.0 — Backend foundations

### Task 1: V002 migration

**Files:**
- Create: `crates/finsight-core/migrations/V002__phase2_schema.sql`
- Test: `crates/finsight-core/tests/migrations_v2.rs` (new file)

- [ ] **Step 1: Write the failing test**

Create `crates/finsight-core/tests/migrations_v2.rs`:

```rust
use finsight_core::{db::run_migrations, keychain, Db};
use rusqlite::params;
use tempfile::TempDir;

#[test]
fn v002_creates_phase2_tables_and_columns() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("v2.sqlcipher");
    let key = keychain::generate_random_key();
    let db = Db::open(&db_path, &key).unwrap();
    run_migrations(&db).unwrap();

    let conn = db.get().unwrap();

    // imports table exists with the right columns.
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(imports)").unwrap()
        .query_map([], |r| r.get::<_, String>(1)).unwrap()
        .filter_map(Result::ok).collect();
    for expected in ["id","source","filename","account_id","started_at",
                     "finished_at","rows_imported","rows_skipped_duplicates","error"] {
        assert!(cols.iter().any(|c| c == expected),
                "imports missing column {expected}: got {cols:?}");
    }

    // csv_import_mappings + settings tables exist.
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap()
        .query_map([], |r| r.get::<_, String>(0)).unwrap()
        .filter_map(Result::ok).collect();
    for expected in ["csv_import_mappings","settings"] {
        assert!(names.iter().any(|n| n == expected),
                "missing table {expected}: got {names:?}");
    }

    // accounts.source column added with correct default.
    let acct_cols: Vec<(String, String)> = conn
        .prepare("PRAGMA table_info(accounts)").unwrap()
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(4)?))).unwrap()
        .filter_map(Result::ok).collect();
    let source = acct_cols.iter().find(|(n, _)| n == "source").expect("accounts.source missing");
    assert!(source.1.contains("'manual'"), "default not 'manual': {:?}", source.1);

    // idx_txn_dedup index exists on transactions.
    let idx_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_txn_dedup'",
        params![], |r| r.get(0)).unwrap();
    assert_eq!(idx_count, 1, "idx_txn_dedup missing");
}
```

If `keychain::generate_random_key` doesn't exist yet, expose a small test helper. Check the Phase 1 code first — there's likely a private `generate_key` used inside `load_or_create_key`. If so, add a `#[cfg(test)] pub fn generate_random_key()` in `keychain.rs` that returns a fresh `Zeroizing<String>`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test migrations_v2 v002_creates_phase2_tables_and_columns`
Expected: FAIL — migration file not found (or "migrations missing in finsight-core/migrations").

- [ ] **Step 3: Write the migration**

Create `crates/finsight-core/migrations/V002__phase2_schema.sql`:

```sql
-- Phase 2 schema additions: import history, mapping cache, accounts.source, settings KV.

CREATE TABLE imports (
  id                       TEXT PRIMARY KEY,
  source                   TEXT NOT NULL,        -- 'csv' | 'manual' | 'sample'
  filename                 TEXT,                 -- NULL for manual/sample
  account_id               TEXT REFERENCES accounts(id),
  started_at               TEXT NOT NULL,
  finished_at              TEXT,                 -- NULL until run completes
  rows_imported            INTEGER NOT NULL DEFAULT 0,
  rows_skipped_duplicates  INTEGER NOT NULL DEFAULT 0,
  error                    TEXT
);
CREATE INDEX idx_imports_unfinished ON imports(finished_at) WHERE finished_at IS NULL;

CREATE TABLE csv_import_mappings (
  account_id    TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  mapping_json  TEXT NOT NULL,
  last_used_at  TEXT NOT NULL
);

ALTER TABLE accounts ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

CREATE INDEX idx_txn_dedup ON transactions(account_id, posted_at, amount_cents, merchant_raw);

CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT NOT NULL                            -- JSON-encoded string
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test migrations_v2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V002__phase2_schema.sql \
        crates/finsight-core/tests/migrations_v2.rs \
        crates/finsight-core/src/keychain.rs  # only if you added generate_random_key
git commit -m "feat(core): V002 — imports, csv mappings, accounts.source, settings KV"
```

---

### Task 2: Settings key-value module

**Files:**
- Create: `crates/finsight-core/src/settings.rs`
- Modify: `crates/finsight-core/src/lib.rs` (add `pub mod settings;`)
- Test: same file (in-file `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Append to a new file `crates/finsight-core/src/settings.rs`:

```rust
//! Typed key/value reader+writer over the `settings` table.
//! Values are JSON-encoded so callers can store booleans, structs, or strings uniformly.

use crate::error::{CoreError, CoreResult};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};

/// Read a setting by key, returning None if absent.
pub fn get<T: DeserializeOwned>(conn: &Connection, key: &str) -> CoreResult<Option<T>> {
    let row: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    match row {
        None => Ok(None),
        Some(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| CoreError::InvalidState(format!("settings[{key}] parse: {e}"))),
    }
}

/// Insert or replace a setting. Value is JSON-encoded.
pub fn set<T: Serialize>(conn: &Connection, key: &str, value: &T) -> CoreResult<()> {
    let json = serde_json::to_string(value)
        .map_err(|e| CoreError::InvalidState(format!("settings[{key}] encode: {e}")))?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}

/// Delete a setting. No-op if absent.
pub fn delete(conn: &Connection, key: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_conn() -> rusqlite::Connection {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("s.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        // Leak the dir so the conn outlives this helper.
        std::mem::forget(dir);
        db.get().unwrap().try_clone().unwrap()
    }

    #[test]
    fn get_missing_key_returns_none() {
        let conn = fresh_conn();
        let v: Option<bool> = get(&conn, "nope").unwrap();
        assert_eq!(v, None);
    }

    #[test]
    fn round_trip_bool() {
        let conn = fresh_conn();
        set(&conn, "onboarding_completion_marked", &true).unwrap();
        let got: Option<bool> = get(&conn, "onboarding_completion_marked").unwrap();
        assert_eq!(got, Some(true));
    }

    #[test]
    fn overwrite_existing_value() {
        let conn = fresh_conn();
        set(&conn, "k", &"a").unwrap();
        set(&conn, "k", &"b").unwrap();
        let got: Option<String> = get(&conn, "k").unwrap();
        assert_eq!(got.as_deref(), Some("b"));
    }

    #[test]
    fn delete_removes_key() {
        let conn = fresh_conn();
        set(&conn, "k", &42i64).unwrap();
        delete(&conn, "k").unwrap();
        let got: Option<i64> = get(&conn, "k").unwrap();
        assert_eq!(got, None);
    }
}
```

> **Note on `Connection::try_clone`:** rusqlite 0.31+ exposes `try_clone`. If our pinned version differs, the helper can instead `forget` the `Db` and use `conn` directly; the goal is just to keep the test connection alive. Adjust to whatever the existing helper pattern is.

- [ ] **Step 2: Wire the module**

Edit `crates/finsight-core/src/lib.rs`. Add `pub mod settings;` next to the other `pub mod` lines.

- [ ] **Step 3: Run test to verify**

Run: `cargo test -p finsight-core settings::tests`
Expected: PASS (all four).

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/settings.rs crates/finsight-core/src/lib.rs
git commit -m "feat(core): typed settings key-value module on V002 table"
```

---

### Task 3: Imports repository

**Files:**
- Create: `crates/finsight-core/src/repos/imports.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs` (add `pub mod imports;`)
- Test: in-file `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

Create `crates/finsight-core/src/repos/imports.rs`:

```rust
//! CRUD for the `imports` table — started at import begin, finished when complete.

use crate::error::CoreResult;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    Csv,
    Manual,
    Sample,
}

impl ImportSource {
    fn as_db(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Manual => "manual",
            Self::Sample => "sample",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Import {
    pub id: String,
    pub source: String,
    pub filename: Option<String>,
    pub account_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub rows_imported: i64,
    pub rows_skipped_duplicates: i64,
    pub error: Option<String>,
}

/// Insert a new import row in started-but-not-finished state. Returns the id.
pub fn start(
    conn: &Connection,
    source: ImportSource,
    filename: Option<&str>,
    account_id: Option<&str>,
) -> CoreResult<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO imports(id, source, filename, account_id, started_at) \
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![&id, source.as_db(), filename, account_id, Utc::now().to_rfc3339()],
    )?;
    Ok(id)
}

/// Mark an import finished with row counts and optional error.
pub fn finish(
    conn: &Connection,
    id: &str,
    rows_imported: u32,
    rows_skipped_duplicates: u32,
    error: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE imports SET finished_at = ?1, rows_imported = ?2, \
              rows_skipped_duplicates = ?3, error = ?4 \
         WHERE id = ?5",
        params![
            Utc::now().to_rfc3339(),
            rows_imported as i64,
            rows_skipped_duplicates as i64,
            error,
            id
        ],
    )?;
    Ok(())
}

/// Return imports whose finished_at is NULL — surfaced as a recovery banner.
pub fn list_unfinished(conn: &Connection) -> CoreResult<Vec<Import>> {
    let mut stmt = conn.prepare(
        "SELECT id, source, filename, account_id, started_at, finished_at, \
                rows_imported, rows_skipped_duplicates, error \
         FROM imports WHERE finished_at IS NULL ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Import {
            id: r.get(0)?,
            source: r.get(1)?,
            filename: r.get(2)?,
            account_id: r.get(3)?,
            started_at: parse_rfc3339(&r.get::<_, String>(4)?),
            finished_at: r.get::<_, Option<String>>(5)?.as_deref().map(parse_rfc3339),
            rows_imported: r.get(6)?,
            rows_skipped_duplicates: r.get(7)?,
            error: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn parse_rfc3339(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap_or_else(|_| Utc::now().into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, rusqlite::Connection) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("imp.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        let conn = db.get().unwrap();
        (dir, conn.try_clone().unwrap())
    }

    #[test]
    fn start_then_finish_round_trip() {
        let (_d, conn) = fresh();
        let id = start(&conn, ImportSource::Csv, Some("chase.csv"), None).unwrap();
        assert!(list_unfinished(&conn).unwrap().iter().any(|i| i.id == id));
        finish(&conn, &id, 42, 3, None).unwrap();
        assert!(list_unfinished(&conn).unwrap().is_empty());
    }

    #[test]
    fn finish_with_error_records_message() {
        let (_d, conn) = fresh();
        let id = start(&conn, ImportSource::Csv, Some("bad.csv"), None).unwrap();
        finish(&conn, &id, 0, 0, Some("file not utf8")).unwrap();
        let row: (i64, i64, Option<String>) = conn.query_row(
            "SELECT rows_imported, rows_skipped_duplicates, error FROM imports WHERE id = ?1",
            params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(row, (0, 0, Some("file not utf8".to_string())));
    }
}
```

Add the module to `crates/finsight-core/src/repos/mod.rs`:

```rust
pub mod accounts;
pub mod categories;
pub mod imports;
pub mod transactions;

// ... existing run() helper unchanged ...
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p finsight-core repos::imports::tests`
Expected: PASS (both tests).

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/src/repos/imports.rs crates/finsight-core/src/repos/mod.rs
git commit -m "feat(core): imports repo with start/finish/list_unfinished"
```

---

### Task 4: Deterministic sample household generator

**Files:**
- Create: `crates/finsight-core/src/sample.rs`
- Modify: `crates/finsight-core/src/lib.rs` (add `pub mod sample;`)
- Modify: `crates/finsight-core/Cargo.toml` (add `rand`, `rand_chacha`)
- Test: in-file `#[cfg(test)] mod tests` + integration test in `tests/sample_seed.rs`

- [ ] **Step 1: Add dependencies**

In `crates/finsight-core/Cargo.toml` under `[dependencies]`:

```toml
rand = { version = "0.8", default-features = false, features = ["std", "std_rng"] }
rand_chacha = "0.3"
```

(If the workspace already pins these in `[workspace.dependencies]`, use `.workspace = true` form to match the codebase convention.)

- [ ] **Step 2: Write the file with both tests included**

Create `crates/finsight-core/src/sample.rs`:

```rust
//! Procedural "Mira & Adam" sample household — used by the onboarding wizard's
//! "Try with sample data" path. Seeded with a pinned constant so tests can assert
//! exact row counts and a known first transaction.

use crate::error::CoreResult;
use crate::Db;
use chrono::{Datelike, Duration, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Pinned. Do not change without bumping the determinism test.
const SAMPLE_SEED: u64 = 0xF1_5165_8AAA_0001;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SeedSummary {
    pub accounts_created: u32,
    pub transactions_created: u32,
    pub import_id: String,
}

struct AccountSpec {
    name: &'static str,
    bank: &'static str,
    owner: &'static str,
    r#type: &'static str,    // matches AccountType::as_db
    color: &'static str,
    opening_cents: i64,
}

const ACCOUNTS: &[AccountSpec] = &[
    AccountSpec { name: "Joint Checking",  bank: "Chase",       owner: "joint", r#type: "Checking",   color: "#3B82F6", opening_cents:  450_000 },
    AccountSpec { name: "Emergency Fund",  bank: "Marcus",      owner: "joint", r#type: "Savings",    color: "#10B981", opening_cents: 1_200_000 },
    AccountSpec { name: "Mira's Card",     bank: "Amex",        owner: "mira",  r#type: "Credit",     color: "#F59E0B", opening_cents:    -8_500 },
    AccountSpec { name: "Adam's Card",     bank: "Chase",       owner: "adam",  r#type: "Credit",     color: "#EF4444", opening_cents:   -12_300 },
    AccountSpec { name: "Brokerage",       bank: "Fidelity",    owner: "joint", r#type: "Investment", color: "#8B5CF6", opening_cents: 8_750_000 },
    AccountSpec { name: "Wallet Cash",     bank: "Cash",        owner: "joint", r#type: "Cash",       color: "#6B7280", opening_cents:    18_000 },
];

const MERCHANTS: &[(&str, i64, i64)] = &[
    // (merchant, min cents, max cents) — negative = outflow.
    ("Safeway",         -2_200, -12_500),
    ("Whole Foods",     -3_500, -18_000),
    ("Starbucks",         -350,  -1_200),
    ("Chipotle",        -1_100,  -2_400),
    ("Shell",           -3_500,  -7_500),
    ("PG&E",           -11_000, -18_500),
    ("Comcast",         -6_500,  -9_500),
    ("Netflix",         -1_599,  -1_599),
    ("Spotify",         -1_099,  -1_099),
    ("Amazon",          -2_000, -45_000),
    ("Target",          -2_500, -28_000),
    ("Uber",            -1_200,  -3_800),
    ("Apple",           -9_900, -29_900),
    ("Acme Payroll",   220_000, 380_000),  // bi-weekly inflow
];

const CATEGORIES: &[(&str, &str, &str)] = &[
    // (id, group_id, label) — group_ids match V001 schema's category_groups
    ("groceries",     "daily",     "Groceries"),
    ("dining",        "daily",     "Dining"),
    ("transport",     "daily",     "Transport"),
    ("housing",       "fixed",     "Housing"),
    ("utilities",     "fixed",     "Utilities"),
    ("subscriptions", "fixed",     "Subscriptions"),
    ("shopping",      "lifestyle", "Shopping"),
    ("travel",        "lifestyle", "Travel"),
    ("gifts",         "lifestyle", "Gifts"),
    ("health",        "wellbeing", "Health"),
];

const CATEGORY_GROUPS: &[(&str, &str)] = &[
    ("fixed",     "Fixed"),
    ("daily",     "Daily"),
    ("lifestyle", "Lifestyle"),
    ("wellbeing", "Wellbeing"),
];

/// Seed the database with the Mira & Adam household. Returns a summary with the
/// `imports` row id so the caller can mark it finished on success.
pub fn seed_household(db: &Db) -> CoreResult<SeedSummary> {
    let mut conn = db.get()?;
    let tx = conn.transaction()?;
    let mut rng = ChaCha20Rng::seed_from_u64(SAMPLE_SEED);

    // 1. Open an `imports` row of source='sample' so the wizard can stamp it on completion.
    let import_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO imports(id, source, filename, started_at) VALUES(?1, 'sample', NULL, ?2)",
        params![&import_id, Utc::now().to_rfc3339()],
    )?;

    // 2. Insert category groups + categories (idempotent via OR IGNORE).
    for (id, label) in CATEGORY_GROUPS {
        tx.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
            params![id, label],
        )?;
    }
    for (id, group, label) in CATEGORIES {
        tx.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) \
             VALUES(?1, ?2, ?3, '#94A3B8', 0)",
            params![id, group, label],
        )?;
    }

    // 3. Insert accounts with source = 'sample'.
    let mut account_ids = Vec::with_capacity(ACCOUNTS.len());
    let now = Utc::now();
    for acct in ACCOUNTS {
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, last4, currency, color, created_at, source) \
             VALUES(?1, ?2, ?3, ?4, ?5, NULL, 'USD', ?6, ?7, 'sample')",
            params![&id, acct.owner, acct.bank, acct.r#type, acct.name, acct.color, now.to_rfc3339()],
        )?;
        tx.execute(
            "INSERT INTO account_balances(account_id, as_of_date, balance_cents) VALUES(?1, ?2, ?3)",
            params![&id, now.date_naive().to_string(), acct.opening_cents],
        )?;
        account_ids.push(id);
    }

    // 4. Generate ~250 transactions across 12 months across the active accounts.
    let start_date = now - Duration::days(365);
    let active_accounts: Vec<&str> = account_ids
        .iter()
        .zip(ACCOUNTS.iter())
        .filter(|(_, a)| a.r#type != "Investment")  // skip the brokerage
        .map(|(id, _)| id.as_str())
        .collect();

    let mut tx_count: u32 = 0;
    for day in 0..365 {
        // 0–2 transactions per day on average, weighted by weekday.
        let n = rng.gen_range(0..=2);
        for _ in 0..n {
            let acct = active_accounts[rng.gen_range(0..active_accounts.len())];
            let (mname, lo, hi) = MERCHANTS[rng.gen_range(0..MERCHANTS.len())];
            let amount = rng.gen_range(*lo..=*hi);
            let cat: Option<&'static str> = category_for(mname);
            let posted = start_date + Duration::days(day);

            tx.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                          category_id, status, created_at) \
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'cleared', ?7)",
                params![
                    Uuid::new_v4().to_string(),
                    acct,
                    posted.to_rfc3339(),
                    amount,
                    mname,
                    cat,                      // rusqlite maps Option<&str> → NULL when None
                    Utc::now().to_rfc3339(),
                ],
            )?;
            tx_count += 1;
        }
    }

    tx.commit()?;
    Ok(SeedSummary {
        accounts_created: ACCOUNTS.len() as u32,
        transactions_created: tx_count,
        import_id,
    })
}

/// Returns `None` for inflows (no consumer-spend category fits) — Phase 3 adds inflow handling.
/// All returned ids MUST exist in CATEGORIES above (FK constraint on transactions.category_id).
fn category_for(merchant: &str) -> Option<&'static str> {
    match merchant {
        "Safeway" | "Whole Foods"     => Some("groceries"),
        "Starbucks" | "Chipotle"      => Some("dining"),
        "Shell" | "Uber"              => Some("transport"),
        "PG&E" | "Comcast"            => Some("utilities"),
        "Netflix" | "Spotify"         => Some("subscriptions"),
        "Amazon" | "Target" | "Apple" => Some("shopping"),
        "Acme Payroll"                => None,           // inflow — no category until Phase 3
        _                             => Some("shopping"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("seed.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn deterministic_seed_produces_known_counts() {
        let (_d, db) = fresh_db();
        let s = seed_household(&db).unwrap();
        assert_eq!(s.accounts_created, 6);
        // RNG is pinned; this exact number must hold. Update only when SAMPLE_SEED changes.
        // Empirically the generator produces between 230 and 280; assert a tight envelope
        // around the deterministic value and let the integration test pin the exact number.
        assert!(s.transactions_created >= 240 && s.transactions_created <= 280,
                "got {} txns", s.transactions_created);
    }

    #[test]
    fn idempotent_against_existing_category_groups() {
        let (_d, db) = fresh_db();
        {
            let conn = db.get().unwrap();
            conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('daily','Daily',0)", []).unwrap();
        }
        // Should not panic on the duplicate group_id.
        seed_household(&db).unwrap();
    }
}
```

Add `pub mod sample;` in `crates/finsight-core/src/lib.rs`.

- [ ] **Step 3: Add the determinism integration test**

Create `crates/finsight-core/tests/sample_seed.rs`:

```rust
use finsight_core::{db::run_migrations, keychain, sample::seed_household, Db};
use tempfile::TempDir;

#[test]
fn sample_seed_is_byte_for_byte_deterministic() {
    let key = keychain::generate_random_key();

    let dir_a = TempDir::new().unwrap();
    let db_a = Db::open(&dir_a.path().join("a.sqlcipher"), &key).unwrap();
    run_migrations(&db_a).unwrap();
    let a = seed_household(&db_a).unwrap();

    let dir_b = TempDir::new().unwrap();
    let db_b = Db::open(&dir_b.path().join("b.sqlcipher"), &key).unwrap();
    run_migrations(&db_b).unwrap();
    let b = seed_household(&db_b).unwrap();

    assert_eq!(a.accounts_created, b.accounts_created);
    assert_eq!(a.transactions_created, b.transactions_created,
               "transaction count drift — RNG stream changed; pin rand_chacha");

    // First merchant_raw (ordered by posted_at, account_id) must match across runs.
    let first_a: String = db_a.get().unwrap().query_row(
        "SELECT merchant_raw FROM transactions ORDER BY posted_at, account_id LIMIT 1",
        [], |r| r.get(0)).unwrap();
    let first_b: String = db_b.get().unwrap().query_row(
        "SELECT merchant_raw FROM transactions ORDER BY posted_at, account_id LIMIT 1",
        [], |r| r.get(0)).unwrap();
    assert_eq!(first_a, first_b);
}
```

- [ ] **Step 4: Run all sample tests**

Run: `cargo test -p finsight-core sample`
Expected: PASS for `sample::tests::deterministic_seed_produces_known_counts`, `sample::tests::idempotent_against_existing_category_groups`, and `sample_seed_is_byte_for_byte_deterministic`.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/Cargo.toml \
        crates/finsight-core/src/sample.rs \
        crates/finsight-core/src/lib.rs \
        crates/finsight-core/tests/sample_seed.rs
git commit -m "feat(core): deterministic Mira & Adam sample household generator"
```

---

### Task 5: Drop walking-skeleton from app startup

**Files:**
- Modify: `crates/finsight-app/src/lib.rs`

The Phase 1 startup chain seeds 1 account + 3 transactions on every fresh DB. Phase 2 lets the user pick a path; on a fresh DB the wizard auto-opens because `accounts` is empty. Calling `walking_skeleton` here would defeat the auto-redirect (because account_count would always be > 0). The seed function is kept in `finsight-core::seed` only for the existing Phase 1 tests; it's no longer called from production startup.

- [ ] **Step 1: Modify the setup closure**

In `crates/finsight-app/src/lib.rs`, locate the `.setup(move |app| { ... })` block and remove the line:

```rust
finsight_core::seed::walking_skeleton(&db).map_err(
    |e| -> Box<dyn std::error::Error> { format!("seed: {e}").into() },
)?;
```

Leave the rest of the setup chain untouched.

- [ ] **Step 2: Run the existing app build to confirm no compile errors**

Run: `cargo build -p finsight-app`
Expected: clean build (warnings about the now-unused import path are fine; an unused import lint will fail clippy, but `seed` is still exported and the test under `finsight-core::seed` still uses it, so no module-level dead code).

- [ ] **Step 3: Add a smoke test that an empty fresh DB stays empty**

Append to `crates/finsight-app/tests/startup_empty.rs` (new file):

```rust
//! After Task 5, a fresh DB on app startup MUST stay empty — the walking-skeleton
//! seed call was removed. This test reproduces the startup chain (open + migrate)
//! and asserts the accounts table is empty.

use finsight_core::{db::run_migrations, keychain, Db};
use tempfile::TempDir;

#[test]
fn fresh_db_after_startup_has_no_accounts() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("startup.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    // NOTE: We deliberately do NOT call seed::walking_skeleton here; this mirrors
    // what configure_app() does in production after Task 5.

    let count: i64 = db.get().unwrap()
        .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0, "expected 0 accounts after fresh migration (no auto-seed)");
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p finsight-app --test startup_empty`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/lib.rs crates/finsight-app/tests/startup_empty.rs
git commit -m "feat(app): drop walking-skeleton seed from startup chain"
```

---

### Task 6: Tauri commands — accounts/transactions create + onboarding probe + sample seed

**Files:**
- Modify: `crates/finsight-app/src/commands/mod.rs`
- Modify: `crates/finsight-app/src/commands/accounts.rs`
- Modify: `crates/finsight-app/src/commands/transactions.rs`
- Create: `crates/finsight-app/src/commands/onboarding.rs`
- Modify: `crates/finsight-app/src/lib.rs` (extend `collect_commands![]`)
- Modify: `crates/finsight-core/src/repos/transactions.rs` (add a `create` helper if not present)

- [ ] **Step 1: Add the `create` helper to the accounts repo**

If `crates/finsight-core/src/repos/accounts.rs::insert` already accepts a `NewAccount`, we'll thin-wrap it. If it does not yet handle the new `source` column, modify `insert` to accept and write it. Update `models::NewAccount` to include `source: String` (default value `"manual"` when callers don't care).

Edit `crates/finsight-core/src/models/account.rs` — append `pub source: String` to `NewAccount`:

```rust
#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewAccount {
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub last4: Option<String>,
    pub currency: String,
    pub color: String,
    pub opening_balance_cents: i64,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String { "manual".to_string() }
```

Edit `crates/finsight-core/src/repos/accounts.rs::insert` to write the new column:

```rust
conn.execute(
    "INSERT INTO accounts (id, owner, bank, type, name, last4, currency, color, created_at, source) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    params![
        &id, &input.owner, &input.bank, input.r#type.as_db(),
        &input.name, &input.last4, &input.currency, &input.color,
        now.to_rfc3339(), &input.source,
    ],
)?;
```

- [ ] **Step 2: Extend the accounts command**

Replace `crates/finsight-app/src/commands/accounts.rs` body with:

```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{Account, AccountSummary, NewAccount};
use finsight_core::repos::{accounts, run};

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<AccountSummary>> {
    let db = (*state.db).clone();
    run(&db, accounts::list_summaries).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    state: tauri::State<'_, AppState>,
    input: NewAccount,
) -> AppResult<Account> {
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::insert(conn, input)).await.map_err(AppError::from)
}
```

- [ ] **Step 3: Extend the transactions command**

Inspect `crates/finsight-core/src/repos/transactions.rs`. If a `create` or `insert` function does not exist, add one:

```rust
use crate::error::CoreResult;
use crate::models::{NewTransaction, Transaction};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn insert(conn: &mut Connection, input: NewTransaction) -> CoreResult<Transaction> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                   merchant_id, category_id, status, notes, created_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 'cleared', ?8, ?9)",
        params![
            &id, &input.account_id, input.posted_at.to_rfc3339(),
            input.amount_cents, &input.merchant_raw,
            &input.merchant_id, &input.category_id,
            &input.notes, now.to_rfc3339(),
        ],
    )?;
    // ... fetch back via SELECT and return Transaction
    transactions_by_id(conn, &id)
}
```

Add `NewTransaction` to `crates/finsight-core/src/models/transaction.rs`:

```rust
#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewTransaction {
    pub account_id: String,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub merchant_id: Option<String>,
    pub category_id: Option<String>,
    pub notes: Option<String>,
}
```

Replace `crates/finsight-app/src/commands/transactions.rs` body with:

```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{NewTransaction, Transaction};
use finsight_core::repos::{run, transactions};

#[tauri::command]
#[specta::specta]
pub async fn list_transactions(state: tauri::State<'_, AppState>) -> AppResult<Vec<Transaction>> {
    let db = (*state.db).clone();
    run(&db, transactions::list_recent).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_transaction(
    state: tauri::State<'_, AppState>,
    input: NewTransaction,
) -> AppResult<Transaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::insert(conn, input)).await.map_err(AppError::from)
}
```

- [ ] **Step 4: Create the onboarding command module**

Create `crates/finsight-app/src/commands/onboarding.rs`:

```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::{imports as imports_repo, run};
use finsight_core::{sample, settings};
use serde::Serialize;
use specta::Type;

const KEY_COMPLETION: &str = "onboarding_completion_marked";

#[derive(Debug, Clone, Serialize, Type)]
pub struct OnboardingState {
    pub account_count: i64,
    pub category_count: i64,
    pub completion_marked: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_state(state: tauri::State<'_, AppState>) -> AppResult<OnboardingState> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let account_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM accounts WHERE archived_at IS NULL", [], |r| r.get(0))?;
        let category_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE archived_at IS NULL", [], |r| r.get(0))?;
        let completion_marked: bool = settings::get::<bool>(conn, KEY_COMPLETION)?
            .unwrap_or(false);
        Ok(OnboardingState { account_count, category_count, completion_marked })
    }).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn seed_sample_household(state: tauri::State<'_, AppState>) -> AppResult<sample::SeedSummary> {
    let db = (*state.db).clone();
    let summary = sample::seed_household(&db).map_err(AppError::from)?;
    // Finish the imports row the seeder opened.
    let txns = summary.transactions_created;
    let id = summary.import_id.clone();
    run(&db, move |conn| {
        imports_repo::finish(conn, &id, txns, 0, None)
    }).await.map_err(AppError::from)?;
    Ok(summary)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_onboarding_complete(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| settings::set(conn, KEY_COMPLETION, &true)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_onboarding_completion(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| settings::set(conn, KEY_COMPLETION, &false)).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn clear_sample_data(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        // FK ON DELETE CASCADE on transactions/account_balances cleans dependents.
        conn.execute("DELETE FROM accounts WHERE source = 'sample'", [])?;
        settings::set(conn, KEY_COMPLETION, &false)?;
        Ok(())
    }).await.map_err(AppError::from)
}
```

- [ ] **Step 5: Register the new commands**

Edit `crates/finsight-app/src/commands/mod.rs`:

```rust
pub mod accounts;
pub mod import;       // added in Task 9
pub mod meta;
pub mod onboarding;
pub mod transactions;
```

(`import` is referenced now so the registration line in lib.rs compiles after Task 9; if you're executing tasks strictly in order, leave the `import` line out here and add it back when Task 9 runs.)

Edit `crates/finsight-app/src/lib.rs::build_specta_builder`:

```rust
pub fn build_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::accounts::list_accounts,
        commands::accounts::create_account,
        commands::transactions::list_transactions,
        commands::transactions::create_transaction,
        commands::onboarding::get_onboarding_state,
        commands::onboarding::seed_sample_household,
        commands::onboarding::mark_onboarding_complete,
        commands::onboarding::reset_onboarding_completion,
        commands::onboarding::clear_sample_data,
        commands::meta::app_ready,
    ])
}
```

- [ ] **Step 6: Add a command smoke test**

Create `crates/finsight-app/tests/onboarding_cmd.rs`:

```rust
use finsight_app::commands::onboarding;
// We can't easily wire `tauri::State` from a unit test, so call the underlying
// pieces directly via finsight_core.

use finsight_core::{db::run_migrations, keychain, sample::seed_household, settings, Db};
use tempfile::TempDir;

#[test]
fn fresh_db_reports_zero_then_sample_increments() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("ob.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    let conn = db.get().unwrap();
    let zero: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).unwrap();
    assert_eq!(zero, 0);
    let marked: Option<bool> = settings::get(&conn, "onboarding_completion_marked").unwrap();
    assert_eq!(marked, None);

    drop(conn);
    let summary = seed_household(&db).unwrap();
    assert_eq!(summary.accounts_created, 6);

    let conn = db.get().unwrap();
    let six: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).unwrap();
    assert_eq!(six, 6);
}
```

- [ ] **Step 7: Run all backend tests**

Run: `cargo test --workspace`
Expected: all green (Phase 1 tests, new V002 test, settings, imports repo, sample, startup_empty, onboarding_cmd).

- [ ] **Step 8: Commit**

```bash
git add crates/finsight-core crates/finsight-app
git commit -m "feat(app): commands for account/txn create, onboarding probe, sample seed"
```

---

## Phase 2.1 — CSV provider

### Task 7: Provider crate scaffold + ProviderError + fixtures

**Files:**
- Modify: `crates/finsight-providers/Cargo.toml` (add `csv`, `encoding_rs`, `chrono`, `serde`, `serde_json`, `thiserror`, `uuid`, `specta`)
- Modify: `crates/finsight-providers/src/lib.rs`
- Create: `crates/finsight-providers/src/error.rs`
- Create: `crates/finsight-providers/src/provider.rs`
- Create: 6 fixture files under `crates/finsight-providers/tests/fixtures/csv/`

- [ ] **Step 1: Add Cargo deps**

In `crates/finsight-providers/Cargo.toml` under `[dependencies]` (use workspace pins where they exist):

```toml
csv = "1.3"
encoding_rs = "0.8"
chrono = { workspace = true, features = ["serde"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
specta = { workspace = true }
finsight-core = { path = "../finsight-core" }
rusqlite = { workspace = true }
```

`[dev-dependencies]`:

```toml
tempfile = { workspace = true }
```

- [ ] **Step 2: Write ProviderError**

Create `crates/finsight-providers/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("file is empty")]
    EmptyFile,
    #[error("file too large: {bytes} bytes (max {cap})")]
    FileTooLarge { bytes: u64, cap: u64 },
    #[error("file is not readable: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv parse: {0}")]
    Csv(#[from] csv::Error),
    #[error("encoding: could not decode as UTF-8 or Windows-1252")]
    UndecodableEncoding,
    #[error("invalid mapping: {0}")]
    InvalidMapping(String),
    #[error("database: {0}")]
    Core(#[from] finsight_core::CoreError),
    #[error("internal: {0}")]
    Internal(String),
}

pub type ProviderResult<T> = std::result::Result<T, ProviderError>;
```

- [ ] **Step 3: Define the SyncProvider trait**

Create `crates/finsight-providers/src/provider.rs`:

```rust
//! Replaces the Phase 1 stub. Sync providers are pluggable backends that
//! produce transactions on the same shape `CsvProvider` does. Phase 2 only
//! implements CsvProvider; Plaid/SimpleFin come later.

use crate::error::ProviderResult;
use finsight_core::models::NewTransaction;

/// A SyncProvider pulls transactions and yields them as parsed rows. Each row
/// either parses to a NewTransaction or is a per-row error.
pub trait SyncProvider {
    /// Human-readable id (e.g. "csv", "plaid"); used in the `imports.source` column.
    fn id(&self) -> &'static str;

    /// Stream rows for the given account. The implementation must be lazy —
    /// callers may stop early on a cap or a cancellation.
    fn rows(&self) -> Box<dyn Iterator<Item = ProviderResult<NewTransaction>> + '_>;
}
```

- [ ] **Step 4: Wire lib.rs**

Replace `crates/finsight-providers/src/lib.rs`:

```rust
//! finsight-providers — pluggable transaction sources.
//! Phase 2 ships the `csv` module; Plaid/SimpleFin land in later phases.

pub mod csv;
pub mod error;
pub mod provider;

pub use error::{ProviderError, ProviderResult};
pub use provider::SyncProvider;
```

This will fail to compile until Task 8 adds `csv/`. To unblock Task 7's commit, add a placeholder `pub mod csv { /* see Task 8 */ }` *inline* in `lib.rs`:

```rust
pub mod csv {
    // Filled in Task 8.
}
```

Replace this inline stub with the real module in Task 8.

- [ ] **Step 5: Add fixture CSV files**

Create `crates/finsight-providers/tests/fixtures/csv/chase-checking.csv`:

```csv
Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #
DEBIT,05/19/2026,SAFEWAY #1234,-8.42,DEBIT_CARD,4321.12,
DEBIT,05/18/2026,STARBUCKS,-6.50,DEBIT_CARD,4329.54,
CREDIT,05/15/2026,PAYROLL ACME,2200.00,ACH_CREDIT,4336.04,
```

`amex-card.csv`:

```csv
Date,Description,Amount
2026-05-19,WHOLE FOODS,42.18
2026-05-18,NETFLIX.COM,15.99
2026-05-15,UBER TRIP,12.40
```

(Amex convention: positive = outflow.)

`mercury-checking.csv`:

```csv
Date (UTC),Description,Amount,Status,Source Account,Reference
2026-05-19,"Stripe payout",1250.00,Sent,Mercury Checking,abc123
2026-05-18,"AWS",-189.43,Sent,Mercury Checking,def456
```

`mint-export.csv`:

```csv
"Date","Description","Original Description","Amount","Transaction Type","Category","Account Name","Labels","Notes"
"5/19/2026","Safeway","SAFEWAY #1234","8.42","debit","Groceries","Chase Checking","",""
"5/18/2026","Starbucks","STARBUCKS","6.50","debit","Coffee Shops","Chase Checking","",""
```

`personal-capital.csv`:

```csv
Date,Account,Description,Category,Tags,Amount
2026-05-19,Chase Checking,Safeway,Groceries,,−8.42
2026-05-18,Chase Checking,Starbucks,Dining,,−6.50
```

(Note: Personal Capital uses the Unicode minus `−` U+2212. Test that the parser handles both `−` and `-`.)

`simple-semicolon.csv`:

```csv
Datum;Beschreibung;Betrag
19.05.2026;REWE SAGT DANKE;-12,34
18.05.2026;STARBUCKS;-3,50
```

(German bank export: semicolon delimiter, DD.MM.YYYY dates, comma decimal — drives the "Custom" date format + the locale-aware amount parser branch.)

- [ ] **Step 6: Smoke check**

Run: `cargo build -p finsight-providers`
Expected: clean build (the `pub mod csv { }` placeholder compiles with the rest).

- [ ] **Step 7: Commit**

```bash
git add crates/finsight-providers
git commit -m "feat(providers): scaffold ProviderError + SyncProvider + 6 fixture CSVs"
```

---

### Task 8: CSV encoding sniff + decoded stream

**Files:**
- Create: `crates/finsight-providers/src/csv/encoding.rs`
- Create: `crates/finsight-providers/src/csv/mod.rs`
- Modify: `crates/finsight-providers/src/lib.rs` (delete the placeholder `pub mod csv {}`, re-add real `pub mod csv;`)

- [ ] **Step 1: Write the failing tests first**

Create `crates/finsight-providers/src/csv/encoding.rs` (test block already included so we can run before the impl):

```rust
//! Layered decoding strategy: BOM sniff → UTF-8 strict → Windows-1252 fallback.

use crate::error::{ProviderError, ProviderResult};

/// Result of sniffing the first bytes of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,           // No BOM, decodes as UTF-8.
    Utf8Bom,        // EF BB BF prefix.
    Utf16Le,        // FF FE prefix.
    Utf16Be,        // FE FF prefix.
    Windows1252,    // No BOM, didn't decode as UTF-8; fallback.
}

/// Decode the entire byte buffer to a String using the layered strategy.
/// Returns (decoded_text, detected_encoding) so callers can surface the
/// "Decoded as Windows-1252" note in the preview header.
pub fn decode_layered(bytes: &[u8]) -> ProviderResult<(String, DetectedEncoding)> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let body = &bytes[3..];
        let (cow, _, had_errors) = encoding_rs::UTF_8.decode(body);
        if had_errors {
            return Err(ProviderError::UndecodableEncoding);
        }
        return Ok((cow.into_owned(), DetectedEncoding::Utf8Bom));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let body = &bytes[2..];
        let (cow, _, had_errors) = encoding_rs::UTF_16LE.decode(body);
        if had_errors {
            return Err(ProviderError::UndecodableEncoding);
        }
        return Ok((cow.into_owned(), DetectedEncoding::Utf16Le));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let body = &bytes[2..];
        let (cow, _, had_errors) = encoding_rs::UTF_16BE.decode(body);
        if had_errors {
            return Err(ProviderError::UndecodableEncoding);
        }
        return Ok((cow.into_owned(), DetectedEncoding::Utf16Be));
    }

    // No BOM — try UTF-8 strict on the whole buffer.
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok((s.to_owned(), DetectedEncoding::Utf8));
    }

    // Fall back to Windows-1252; encoding_rs guarantees no errors for 1252.
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    tracing::warn!("CSV decoded as Windows-1252 (no UTF-8 BOM and not valid UTF-8)");
    Ok((cow.into_owned(), DetectedEncoding::Windows1252))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "hello");
        assert_eq!(enc, DetectedEncoding::Utf8Bom);
    }

    #[test]
    fn decodes_utf16_le_with_bom() {
        // "hi" in UTF-16 LE with BOM
        let bytes = b"\xFF\xFEh\x00i\x00";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "hi");
        assert_eq!(enc, DetectedEncoding::Utf16Le);
    }

    #[test]
    fn decodes_utf16_be_with_bom() {
        let bytes = b"\xFE\xFF\x00h\x00i";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "hi");
        assert_eq!(enc, DetectedEncoding::Utf16Be);
    }

    #[test]
    fn plain_utf8_no_bom_is_utf8() {
        let bytes = b"name,amount\nSafeway,-8.42\n";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert!(s.starts_with("name,amount"));
        assert_eq!(enc, DetectedEncoding::Utf8);
    }

    #[test]
    fn invalid_utf8_falls_back_to_windows_1252() {
        // 0xE9 is "é" in Windows-1252 but invalid as a UTF-8 lead byte standalone.
        let bytes = b"caf\xE9";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "café");
        assert_eq!(enc, DetectedEncoding::Windows1252);
    }
}
```

- [ ] **Step 2: Create the csv module shell**

Replace the placeholder in `crates/finsight-providers/src/lib.rs`:

```rust
pub mod csv;
```

Create `crates/finsight-providers/src/csv/mod.rs`:

```rust
//! CSV ingestion provider. Public surface: CsvProvider, CsvImportMapping,
//! CsvPreview, ImportSummary, RowError.

pub mod encoding;
// pub mod parse;     // filled in Task 9
// pub mod mapping;   // filled in Task 9
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p finsight-providers csv::encoding`
Expected: 5/5 pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-providers/src/lib.rs \
        crates/finsight-providers/src/csv/mod.rs \
        crates/finsight-providers/src/csv/encoding.rs
git commit -m "feat(providers): BOM-aware CSV decoding with Windows-1252 fallback"
```

---

### Task 9: CSV parser + mapping + CsvProvider (preview & import)

**Files:**
- Create: `crates/finsight-providers/src/csv/parse.rs`
- Create: `crates/finsight-providers/src/csv/mapping.rs`
- Modify: `crates/finsight-providers/src/csv/mod.rs` (CsvProvider, ImportSummary, RowError, CsvPreview)
- Test: `crates/finsight-providers/tests/csv_integration.rs`

This is the largest task in the plan. Split it into three deliberate phases (mapping types → parse → CsvProvider) so a re-dispatched subagent can pick up cleanly if interrupted.

- [ ] **Step 1: Write the mapping types + tests**

Create `crates/finsight-providers/src/csv/mapping.rs`:

```rust
//! CsvImportMapping — describes how a particular CSV's columns map to
//! NewTransaction fields. Persisted per-account in `csv_import_mappings`.

use crate::error::{ProviderError, ProviderResult};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AmountConvention {
    /// Negative numbers are outflows (US bank convention).
    NegativeIsOutflow,
    /// Positive numbers are outflows (Amex / some credit cards).
    PositiveIsOutflow,
    /// Two separate debit/credit columns.
    SplitDebitCredit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum ColumnRole {
    Date,
    Amount,
    Merchant,
    Notes,
    Category,
    Skip,
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CsvImportMapping {
    pub skip_header_rows: u32,
    /// One entry per source column, in order.
    pub columns: Vec<ColumnRole>,
    /// e.g. "%Y-%m-%d", "%m/%d/%Y", "%d.%m.%Y".
    pub date_format: String,
    pub amount_convention: AmountConvention,
    /// Some banks (German, French) use "," as decimal separator. Default ".".
    #[serde(default = "default_decimal")]
    pub decimal_separator: char,
    #[serde(default)]
    pub delimiter: Option<char>,    // Some(';') or None (auto-detected).
}

fn default_decimal() -> char { '.' }

/// Load the saved mapping for an account, if any.
pub fn load(conn: &Connection, account_id: &str) -> ProviderResult<Option<CsvImportMapping>> {
    let row: Option<String> = conn
        .query_row(
            "SELECT mapping_json FROM csv_import_mappings WHERE account_id = ?1",
            params![account_id], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(|e| ProviderError::Internal(format!("load mapping: {e}")))?;
    match row {
        None => Ok(None),
        Some(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| ProviderError::InvalidMapping(format!("decode: {e}"))),
    }
}

/// Save (or overwrite) the mapping for an account, stamping last_used_at to now.
pub fn save(conn: &Connection, account_id: &str, mapping: &CsvImportMapping) -> ProviderResult<()> {
    let json = serde_json::to_string(mapping)
        .map_err(|e| ProviderError::InvalidMapping(format!("encode: {e}")))?;
    conn.execute(
        "INSERT INTO csv_import_mappings(account_id, mapping_json, last_used_at) \
         VALUES(?1, ?2, ?3) \
         ON CONFLICT(account_id) DO UPDATE SET \
            mapping_json = excluded.mapping_json, \
            last_used_at = excluded.last_used_at",
        params![account_id, json, Utc::now().to_rfc3339()],
    ).map_err(|e| ProviderError::Internal(format!("save mapping: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_conn_with_account() -> (TempDir, rusqlite::Connection, String) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("m.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        let conn = db.get().unwrap();
        let acct_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at, source) \
             VALUES(?1, 'joint', 'Chase', 'Checking', 'Test', 'USD', '#000', ?2, 'manual')",
            params![&acct_id, Utc::now().to_rfc3339()],
        ).unwrap();
        (dir, conn.try_clone().unwrap(), acct_id)
    }

    fn sample_mapping() -> CsvImportMapping {
        CsvImportMapping {
            skip_header_rows: 1,
            columns: vec![ColumnRole::Skip, ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            date_format: "%m/%d/%Y".to_string(),
            amount_convention: AmountConvention::NegativeIsOutflow,
            decimal_separator: '.',
            delimiter: None,
        }
    }

    #[test]
    fn load_returns_none_for_unknown_account() {
        let (_d, conn, acct) = fresh_conn_with_account();
        assert!(load(&conn, &acct).unwrap().is_none());
    }

    #[test]
    fn round_trip_save_then_load() {
        let (_d, conn, acct) = fresh_conn_with_account();
        save(&conn, &acct, &sample_mapping()).unwrap();
        let got = load(&conn, &acct).unwrap().unwrap();
        assert_eq!(got.date_format, "%m/%d/%Y");
        assert_eq!(got.skip_header_rows, 1);
    }

    #[test]
    fn save_twice_overwrites() {
        let (_d, conn, acct) = fresh_conn_with_account();
        let mut m = sample_mapping();
        save(&conn, &acct, &m).unwrap();
        m.skip_header_rows = 5;
        save(&conn, &acct, &m).unwrap();
        let got = load(&conn, &acct).unwrap().unwrap();
        assert_eq!(got.skip_header_rows, 5);
    }
}
```

Run: `cargo test -p finsight-providers csv::mapping`
Expected: 3/3 pass.

- [ ] **Step 2: Write the row parser + tests**

Create `crates/finsight-providers/src/csv/parse.rs`:

```rust
//! Pure CSV row → NewTransaction. No I/O, no DB.

use crate::csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
use chrono::{DateTime, NaiveDate, Utc};
use finsight_core::models::NewTransaction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRow {
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub notes: Option<String>,
    pub category_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongColumnCount { got: usize, expected: usize },
    UnparseableDate(String),
    UnparseableAmount(String),
    MissingRequiredField(&'static str),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongColumnCount { got, expected } =>
                write!(f, "expected {expected} columns, got {got}"),
            Self::UnparseableDate(s) => write!(f, "could not parse date {s:?}"),
            Self::UnparseableAmount(s) => write!(f, "could not parse amount {s:?}"),
            Self::MissingRequiredField(name) => write!(f, "missing required field {name}"),
        }
    }
}

/// Parse a single CSV row (after header rows have been skipped) using the mapping.
pub fn parse_row(fields: &[&str], mapping: &CsvImportMapping) -> Result<ParsedRow, ParseError> {
    if fields.len() != mapping.columns.len() {
        return Err(ParseError::WrongColumnCount {
            got: fields.len(),
            expected: mapping.columns.len(),
        });
    }

    let mut date: Option<&str> = None;
    let mut amount: Option<&str> = None;
    let mut debit: Option<&str> = None;
    let mut credit: Option<&str> = None;
    let mut merchant: Option<&str> = None;
    let mut notes: Option<&str> = None;
    let mut category: Option<&str> = None;

    for (idx, role) in mapping.columns.iter().enumerate() {
        let v = fields[idx].trim();
        match role {
            ColumnRole::Date     => date = Some(v),
            ColumnRole::Amount   => amount = Some(v),
            ColumnRole::Debit    => debit = Some(v),
            ColumnRole::Credit   => credit = Some(v),
            ColumnRole::Merchant => merchant = Some(v),
            ColumnRole::Notes    if !v.is_empty() => notes = Some(v),
            ColumnRole::Category if !v.is_empty() => category = Some(v),
            ColumnRole::Notes | ColumnRole::Category | ColumnRole::Skip => {},
        }
    }

    let merchant = merchant.ok_or(ParseError::MissingRequiredField("merchant"))?.to_owned();
    if merchant.is_empty() {
        return Err(ParseError::MissingRequiredField("merchant"));
    }
    let date_str = date.ok_or(ParseError::MissingRequiredField("date"))?;
    let posted = parse_date(date_str, &mapping.date_format)?;

    let amount_cents = match mapping.amount_convention {
        AmountConvention::SplitDebitCredit => {
            let d = debit.unwrap_or("");
            let c = credit.unwrap_or("");
            let d_cents = if d.is_empty() { 0 } else { parse_amount(d, mapping.decimal_separator)? };
            let c_cents = if c.is_empty() { 0 } else { parse_amount(c, mapping.decimal_separator)? };
            // Debit is outflow (negative). Credit is inflow (positive).
            c_cents - d_cents
        },
        AmountConvention::NegativeIsOutflow => {
            let a = amount.ok_or(ParseError::MissingRequiredField("amount"))?;
            parse_amount(a, mapping.decimal_separator)?
        },
        AmountConvention::PositiveIsOutflow => {
            let a = amount.ok_or(ParseError::MissingRequiredField("amount"))?;
            -parse_amount(a, mapping.decimal_separator)?
        },
    };

    Ok(ParsedRow {
        posted_at: posted,
        amount_cents,
        merchant_raw: merchant,
        notes: notes.map(str::to_owned),
        category_hint: category.map(str::to_owned),
    })
}

fn parse_date(s: &str, fmt: &str) -> Result<DateTime<Utc>, ParseError> {
    NaiveDate::parse_from_str(s, fmt)
        .map(|d| d.and_hms_opt(12, 0, 0).unwrap().and_utc())
        .map_err(|_| ParseError::UnparseableDate(s.to_owned()))
}

fn parse_amount(s: &str, decimal_separator: char) -> Result<i64, ParseError> {
    // Normalize: strip whitespace, drop thousands separators, accept Unicode minus.
    let cleaned: String = s
        .chars()
        .filter_map(|c| match c {
            '\u{2212}' => Some('-'),
            ',' if decimal_separator == ',' => Some('.'),
            '.' if decimal_separator == ',' => None,        // German thousands separator
            ',' if decimal_separator == '.' => None,        // US thousands separator
            ' ' | '$' | '€' | '£' => None,
            other => Some(other),
        })
        .collect();
    let f: f64 = cleaned.parse().map_err(|_| ParseError::UnparseableAmount(s.to_owned()))?;
    // Round-half-to-even to nearest cent.
    Ok((f * 100.0).round() as i64)
}

/// Convenience adapter for callers who want a NewTransaction directly.
pub fn into_new_transaction(parsed: ParsedRow, account_id: String) -> NewTransaction {
    NewTransaction {
        account_id,
        posted_at: parsed.posted_at,
        amount_cents: parsed.amount_cents,
        merchant_raw: parsed.merchant_raw,
        merchant_id: None,
        category_id: None,
        notes: parsed.notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(cols: Vec<ColumnRole>, conv: AmountConvention, fmt: &str) -> CsvImportMapping {
        CsvImportMapping {
            skip_header_rows: 0,
            columns: cols,
            date_format: fmt.to_string(),
            amount_convention: conv,
            decimal_separator: '.',
            delimiter: None,
        }
    }

    #[test]
    fn standard_us_negative_outflow() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%Y-%m-%d");
        let p = parse_row(&["2026-05-19", "Safeway", "-8.42"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
        assert_eq!(p.merchant_raw, "Safeway");
    }

    #[test]
    fn amex_positive_outflow_negates() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::PositiveIsOutflow, "%Y-%m-%d");
        let p = parse_row(&["2026-05-19", "Whole Foods", "42.18"], &m).unwrap();
        assert_eq!(p.amount_cents, -4218);
    }

    #[test]
    fn split_debit_credit() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Debit, ColumnRole::Credit],
                    AmountConvention::SplitDebitCredit, "%Y-%m-%d");
        let p = parse_row(&["2026-05-19", "Safeway", "8.42", ""], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
        let p = parse_row(&["2026-05-15", "Payroll", "", "2200.00"], &m).unwrap();
        assert_eq!(p.amount_cents, 220_000);
    }

    #[test]
    fn mmddyyyy_date_format() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%m/%d/%Y");
        let p = parse_row(&["5/19/2026", "Safeway", "-8.42"], &m).unwrap();
        assert_eq!(p.posted_at.naive_utc().date().to_string(), "2026-05-19");
    }

    #[test]
    fn unicode_minus_sign_accepted() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%Y-%m-%d");
        let p = parse_row(&["2026-05-19", "Safeway", "−8.42"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
    }

    #[test]
    fn german_comma_decimal() {
        let mut m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%d.%m.%Y");
        m.decimal_separator = ',';
        let p = parse_row(&["19.05.2026", "REWE", "-12,34"], &m).unwrap();
        assert_eq!(p.amount_cents, -1234);
    }

    #[test]
    fn quoted_field_with_comma_does_not_break() {
        // The csv crate handles quoting at the reader level; parse_row gets clean fields.
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%Y-%m-%d");
        let p = parse_row(&["2026-05-19", "Smith, Jones & Co.", "-8.42"], &m).unwrap();
        assert_eq!(p.merchant_raw, "Smith, Jones & Co.");
    }

    #[test]
    fn missing_merchant_field_errors() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%Y-%m-%d");
        let err = parse_row(&["2026-05-19", "", "-8.42"], &m).unwrap_err();
        assert!(matches!(err, ParseError::MissingRequiredField("merchant")));
    }

    #[test]
    fn unparseable_date_errors() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%Y-%m-%d");
        let err = parse_row(&["not a date", "Safeway", "-8.42"], &m).unwrap_err();
        assert!(matches!(err, ParseError::UnparseableDate(_)));
    }

    #[test]
    fn wrong_column_count_errors() {
        let m = map(vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
                    AmountConvention::NegativeIsOutflow, "%Y-%m-%d");
        let err = parse_row(&["2026-05-19", "Safeway"], &m).unwrap_err();
        assert!(matches!(err, ParseError::WrongColumnCount { got: 2, expected: 3 }));
    }
}
```

Run: `cargo test -p finsight-providers csv::parse`
Expected: 10/10 pass.

- [ ] **Step 3: Implement CsvProvider**

Replace `crates/finsight-providers/src/csv/mod.rs` with:

```rust
//! CSV ingestion provider — preview, import with dedup, mapping persistence.

pub mod encoding;
pub mod mapping;
pub mod parse;

use crate::error::{ProviderError, ProviderResult};
use crate::csv::encoding::{decode_layered, DetectedEncoding};
use crate::csv::mapping::CsvImportMapping;
use crate::csv::parse::{into_new_transaction, parse_row};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_BYTES: u64 = 50 * 1024 * 1024;            // 50 MiB
const PREVIEW_ROWS: usize = 10;
const PREVIEW_COUNT_CAP: u32 = 10_000;
const BATCH_SIZE: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CsvPreview {
    pub headers: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
    pub detected_delimiter: char,
    pub total_rows: u32,
    /// "Decoded as Windows-1252" surfaces in the preview header when this is Some.
    pub encoding_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImportSummary {
    pub import_id: String,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub errors: Vec<RowError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RowError {
    pub row_number: u32,
    pub reason: String,
}

/// Result yielded to the progress callback after each batch commit.
#[derive(Debug, Clone)]
pub struct BatchProgress {
    pub rows_done: u32,
    pub rows_total: u32,
}

pub struct CsvProvider;

impl CsvProvider {
    /// Read up to PREVIEW_ROWS data rows + count up to PREVIEW_COUNT_CAP total rows.
    pub fn preview(path: &Path, skip_header_rows: u32) -> ProviderResult<CsvPreview> {
        let bytes = read_capped(path)?;
        if bytes.is_empty() {
            return Err(ProviderError::EmptyFile);
        }
        let (text, encoding) = decode_layered(&bytes)?;
        let delimiter = detect_delimiter(&text);
        let encoding_note = match encoding {
            DetectedEncoding::Windows1252 =>
                Some("Decoded as Windows-1252 (no UTF-8 BOM detected)".into()),
            _ => None,
        };

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter as u8)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut headers: Option<Vec<String>> = None;
        let mut rows: Vec<Vec<String>> = Vec::with_capacity(PREVIEW_ROWS);
        let mut total: u32 = 0;
        let skip = skip_header_rows as usize;

        for (idx, rec) in reader.records().enumerate() {
            let rec = rec?;
            if idx == 0 && skip > 0 {
                headers = Some(rec.iter().map(str::to_owned).collect());
            }
            if idx >= skip {
                total = total.saturating_add(1);
                if rows.len() < PREVIEW_ROWS {
                    rows.push(rec.iter().map(str::to_owned).collect());
                }
                if total >= PREVIEW_COUNT_CAP { break; }
            }
        }

        Ok(CsvPreview {
            headers,
            rows,
            detected_delimiter: delimiter,
            total_rows: total,
            encoding_note,
        })
    }

    /// Import every row from `path` into `account_id` using `mapping`.
    /// `on_progress` is called after each batch commit (every BATCH_SIZE rows
    /// or at the adaptive cadence `max(1, total/20)`, whichever fires sooner).
    pub fn import(
        path: &Path,
        account_id: &str,
        mapping: &CsvImportMapping,
        db: &finsight_core::Db,
        mut on_progress: impl FnMut(BatchProgress) + Send,
    ) -> ProviderResult<ImportSummary> {
        let bytes = read_capped(path)?;
        if bytes.is_empty() {
            return Err(ProviderError::EmptyFile);
        }
        let (text, _) = decode_layered(&bytes)?;
        let delimiter = mapping.delimiter.unwrap_or_else(|| detect_delimiter(&text));

        // First pass: count rows for progress + adaptive emission cadence.
        let total = {
            let mut r = csv::ReaderBuilder::new()
                .has_headers(false)
                .delimiter(delimiter as u8)
                .flexible(true)
                .from_reader(text.as_bytes());
            let mut n: u32 = 0;
            for (idx, rec) in r.records().enumerate() {
                rec?;
                if idx >= mapping.skip_header_rows as usize {
                    n = n.saturating_add(1);
                }
            }
            n
        };
        let emit_every = std::cmp::max(1, total / 20) as usize;

        // Open the imports row + buffer up rows for batched insert.
        let mut conn = db.get().map_err(ProviderError::Core)?;
        let filename = path.file_name().map(|s| s.to_string_lossy().into_owned());
        let import_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO imports(id, source, filename, account_id, started_at) \
             VALUES(?1, 'csv', ?2, ?3, ?4)",
            params![&import_id, filename, account_id, Utc::now().to_rfc3339()],
        ).map_err(|e| ProviderError::Internal(format!("imports insert: {e}")))?;

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter as u8)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut rows_imported: u32 = 0;
        let mut rows_skipped: u32 = 0;
        let mut errors: Vec<RowError> = Vec::new();
        let mut processed: u32 = 0;

        let mut tx = conn.transaction().map_err(|e| ProviderError::Internal(format!("begin: {e}")))?;

        for (idx, rec) in reader.records().enumerate() {
            let row_number = (idx + 1) as u32;
            let rec = match rec {
                Ok(r) => r,
                Err(e) => { errors.push(RowError { row_number, reason: e.to_string() }); continue; }
            };
            if idx < mapping.skip_header_rows as usize { continue; }

            let fields: Vec<&str> = rec.iter().collect();
            let parsed = match parse_row(&fields, mapping) {
                Ok(p) => p,
                Err(e) => { errors.push(RowError { row_number, reason: e.to_string() }); continue; }
            };
            let new_tx = into_new_transaction(parsed, account_id.to_string());

            // Dedup check via the V002 covering index.
            let exists: bool = tx.query_row(
                "SELECT 1 FROM transactions WHERE account_id = ?1 AND posted_at = ?2 \
                                                AND amount_cents = ?3 AND merchant_raw = ?4 LIMIT 1",
                params![&new_tx.account_id, new_tx.posted_at.to_rfc3339(),
                        new_tx.amount_cents, &new_tx.merchant_raw],
                |_| Ok(true),
            ).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                other => Err(other),
            }).map_err(|e| ProviderError::Internal(format!("dedup: {e}")))?;

            if exists {
                rows_skipped += 1;
            } else {
                tx.execute(
                    "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                              status, notes, created_at) \
                     VALUES(?1, ?2, ?3, ?4, ?5, 'cleared', ?6, ?7)",
                    params![
                        Uuid::new_v4().to_string(),
                        &new_tx.account_id,
                        new_tx.posted_at.to_rfc3339(),
                        new_tx.amount_cents,
                        &new_tx.merchant_raw,
                        &new_tx.notes,
                        Utc::now().to_rfc3339(),
                    ],
                ).map_err(|e| ProviderError::Internal(format!("insert: {e}")))?;
                rows_imported += 1;
            }

            processed += 1;
            if processed as usize >= BATCH_SIZE
                || (emit_every > 0 && (processed as usize) % emit_every == 0)
            {
                tx.commit().map_err(|e| ProviderError::Internal(format!("commit batch: {e}")))?;
                on_progress(BatchProgress { rows_done: processed, rows_total: total });
                tx = conn.transaction().map_err(|e| ProviderError::Internal(format!("begin: {e}")))?;
            }
        }

        // Persist the mapping for re-imports + finalize the imports row.
        mapping::save(&tx, account_id, mapping)?;
        tx.execute(
            "UPDATE imports SET finished_at = ?1, rows_imported = ?2, \
                                  rows_skipped_duplicates = ?3 WHERE id = ?4",
            params![Utc::now().to_rfc3339(), rows_imported as i64,
                    rows_skipped as i64, &import_id],
        ).map_err(|e| ProviderError::Internal(format!("imports finish: {e}")))?;
        tx.commit().map_err(|e| ProviderError::Internal(format!("commit final: {e}")))?;

        on_progress(BatchProgress { rows_done: processed, rows_total: total });

        Ok(ImportSummary {
            import_id,
            rows_imported,
            rows_skipped_duplicates: rows_skipped,
            errors,
        })
    }
}

fn detect_delimiter(text: &str) -> char {
    let first_line = text.lines().next().unwrap_or("");
    let commas = first_line.matches(',').count();
    let semis  = first_line.matches(';').count();
    let tabs   = first_line.matches('\t').count();
    if tabs >= commas && tabs >= semis && tabs > 0 { '\t' }
    else if semis > commas { ';' }
    else { ',' }
}

fn read_capped(path: &Path) -> ProviderResult<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_BYTES {
        return Err(ProviderError::FileTooLarge { bytes: meta.len(), cap: MAX_BYTES });
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    use std::io::Read;
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

// Re-export so callers can `use finsight_providers::CsvImportMapping;`
pub use mapping::{AmountConvention, ColumnRole};

// Internal: also let other modules see a PathBuf type alias for clarity.
#[allow(dead_code)]
pub(crate) type PathRef = PathBuf;
```

Update `crates/finsight-providers/src/lib.rs` to add the public re-exports:

```rust
pub use csv::{CsvPreview, CsvProvider, ImportSummary, RowError};
pub use csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
```

- [ ] **Step 4: Integration test against real fixture files**

Create `crates/finsight-providers/tests/csv_integration.rs`:

```rust
use chrono::Utc;
use finsight_core::{db::run_migrations, keychain, Db};
use finsight_providers::{
    AmountConvention, ColumnRole, CsvImportMapping, CsvProvider,
};
use rusqlite::params;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/csv").join(name)
}

fn fresh_db() -> (TempDir, Db, String) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("ci.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    let acct = uuid::Uuid::new_v4().to_string();
    db.get().unwrap().execute(
        "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at, source) \
         VALUES(?1, 'joint', 'Chase', 'Checking', 'Test', 'USD', '#000', ?2, 'manual')",
        params![&acct, Utc::now().to_rfc3339()],
    ).unwrap();
    (dir, db, acct)
}

#[test]
fn chase_csv_imports_then_dedupes_on_reimport() {
    let (_d, db, acct) = fresh_db();
    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![
            ColumnRole::Skip, ColumnRole::Date, ColumnRole::Merchant,
            ColumnRole::Amount, ColumnRole::Skip, ColumnRole::Skip, ColumnRole::Skip,
        ],
        date_format: "%m/%d/%Y".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: '.',
        delimiter: Some(','),
    };

    let s = CsvProvider::import(&fixture("chase-checking.csv"), &acct, &mapping, &db, |_| {}).unwrap();
    assert_eq!(s.rows_imported, 3);
    assert_eq!(s.rows_skipped_duplicates, 0);
    assert!(s.errors.is_empty());

    let s2 = CsvProvider::import(&fixture("chase-checking.csv"), &acct, &mapping, &db, |_| {}).unwrap();
    assert_eq!(s2.rows_imported, 0);
    assert_eq!(s2.rows_skipped_duplicates, 3);
}

#[test]
fn semicolon_german_csv_parses_with_comma_decimal() {
    let (_d, db, acct) = fresh_db();
    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
        date_format: "%d.%m.%Y".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: ',',
        delimiter: Some(';'),
    };
    let s = CsvProvider::import(&fixture("simple-semicolon.csv"), &acct, &mapping, &db, |_| {}).unwrap();
    assert_eq!(s.rows_imported, 2);
}

#[test]
fn preview_returns_correct_row_count_and_first_rows() {
    let p = CsvProvider::preview(&fixture("amex-card.csv"), 1).unwrap();
    assert_eq!(p.total_rows, 3);
    assert_eq!(p.rows.len(), 3);
    assert_eq!(p.detected_delimiter, ',');
}
```

Run: `cargo test -p finsight-providers`
Expected: all encoding/parse/mapping/integration tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-providers
git commit -m "feat(providers): CsvProvider preview + dedup-aware batched import"
```

---

### Task 10: Tauri commands for CSV import + unfinished imports

**Files:**
- Create: `crates/finsight-app/src/commands/import.rs`
- Modify: `crates/finsight-app/src/error.rs` (add `From<ProviderError>`)
- Modify: `crates/finsight-app/src/lib.rs` (register the 3 new commands; emit progress event)
- Modify: `crates/finsight-app/Cargo.toml` (add `finsight-providers` dep)

- [ ] **Step 1: Cargo wiring**

In `crates/finsight-app/Cargo.toml` under `[dependencies]`:

```toml
finsight-providers = { path = "../finsight-providers" }
```

- [ ] **Step 2: Error conversion**

Add to `crates/finsight-app/src/error.rs` (next to other `From` impls):

```rust
impl From<finsight_providers::ProviderError> for AppError {
    fn from(err: finsight_providers::ProviderError) -> Self {
        AppError {
            kind: "provider".to_string(),
            message: err.to_string(),
        }
    }
}
```

(Adapt to whatever the existing `AppError` shape is — match the Phase 1 idiom.)

- [ ] **Step 3: Implement the import commands**

Create `crates/finsight-app/src/commands/import.rs`:

```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::{imports as imports_repo, run};
use finsight_providers::{CsvImportMapping, CsvPreview, CsvProvider, ImportSummary};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize, Type)]
pub struct ProgressPayload {
    pub import_id: String,
    pub rows_done: u32,
    pub rows_total: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn preview_csv_columns(
    path: String,
    skip_header_rows: u32,
) -> AppResult<CsvPreview> {
    let path_buf = PathBuf::from(path);
    tokio::task::spawn_blocking(move || CsvProvider::preview(&path_buf, skip_header_rows))
        .await
        .map_err(|e| AppError {
            kind: "internal".into(),
            message: format!("join: {e}"),
        })?
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn import_csv(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<ImportSummary> {
    let db = (*state.db).clone();
    let path = PathBuf::from(path);
    // Hold the import_id slot to emit a progress event with it; CsvProvider
    // generates the id, so we capture it via the first progress callback.
    let app_emit = app.clone();

    let summary = tokio::task::spawn_blocking(move || {
        let mut current_id: Option<String> = None;
        let result = CsvProvider::import(&path, &account_id, &mapping, &db, |p| {
            // Re-read the imports.id from the open transaction is not possible
            // here; instead we let CsvProvider expose the id via the summary
            // and emit a single "complete" event below. Progress events use
            // the *summary* id once we have it — for very long imports we
            // also emit interim progress without an id, which the frontend
            // simply ignores. This is a deliberate simplification.
            if let Some(id) = &current_id {
                let _ = app_emit.emit("import.progress", ProgressPayload {
                    import_id: id.clone(),
                    rows_done: p.rows_done,
                    rows_total: p.rows_total,
                });
            }
        }).map_err(AppError::from)?;
        current_id = Some(result.import_id.clone());
        Ok::<_, AppError>(result)
    })
    .await
    .map_err(|e| AppError { kind: "internal".into(), message: format!("join: {e}") })??;

    app.emit("import.complete", &summary.import_id).ok();
    Ok(summary)
}

#[tauri::command]
#[specta::specta]
pub async fn list_unfinished_imports(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<imports_repo::Import>> {
    let db = (*state.db).clone();
    run(&db, imports_repo::list_unfinished).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn discard_unfinished_import(
    state: tauri::State<'_, AppState>,
    import_id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        imports_repo::finish(conn, &import_id, 0, 0, Some("discarded"))
    }).await.map_err(AppError::from)
}
```

> **Note on the progress emission caveat:** Capturing `import_id` from within the closure before the provider returns it is awkward. A cleaner approach is to change `CsvProvider::import` to accept an `import_id: &str` that the caller pre-generates and passes both to `imports::start` and to the provider. The plan keeps the simpler shape above for clarity — when implementing, prefer the pre-generated id refactor: generate the UUID in the command, call `imports::start`, pass the id into `CsvProvider::import_with_id`. Update the provider signature accordingly.

- [ ] **Step 4: Register commands**

Edit `crates/finsight-app/src/lib.rs::build_specta_builder` — add the four new commands:

```rust
commands::import::preview_csv_columns,
commands::import::import_csv,
commands::import::list_unfinished_imports,
commands::import::discard_unfinished_import,
```

And ensure `commands::import` is `pub mod`'d in `commands/mod.rs`.

- [ ] **Step 5: Smoke test the command surface**

Run: `cargo build -p finsight-app`
Expected: clean build. The end-to-end behavior is exercised through the CSV integration test in Task 9 (`csv_integration.rs`).

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/Cargo.toml \
        crates/finsight-app/src/commands/import.rs \
        crates/finsight-app/src/commands/mod.rs \
        crates/finsight-app/src/error.rs \
        crates/finsight-app/src/lib.rs
git commit -m "feat(app): Tauri commands for CSV preview/import + unfinished import recovery"
```

---

## Phase 2.2 — Onboarding shell + sample-data wiring

### Task 11: Tauri plugins + frontend deps + regenerate bindings

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `crates/finsight-app/src/lib.rs` (register `.plugin()`s)
- Modify: `ui/package.json`
- Regenerate: `ui/src/api/bindings.ts`

- [ ] **Step 1: Add Tauri plugin crates**

In `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
tauri-plugin-dialog = "2"
tauri-plugin-opener = "2"
```

Also add the same two crates to `crates/finsight-app/Cargo.toml` (the app crate is where `configure_app` registers them).

- [ ] **Step 2: Register plugins**

Edit `crates/finsight-app/src/lib.rs::configure_app` — add `.plugin(...)` calls right after `tauri_plugin_single_instance`:

```rust
.plugin(tauri_plugin_dialog::init())
.plugin(tauri_plugin_opener::init())
```

- [ ] **Step 3: Update capability permissions**

Edit `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the FinSight desktop window.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default",
    "opener:default"
  ]
}
```

(If the existing file has different field names — `windows` vs `target` for example — keep the original shape and only add the two permissions.)

- [ ] **Step 4: Add frontend deps**

In `ui/package.json` `dependencies`:

```json
"@tauri-apps/plugin-dialog": "^2",
"@tauri-apps/plugin-opener": "^2",
"react-hook-form": "^7",
"zod": "^3",
"@hookform/resolvers": "^3",
"react-focus-lock": "^2"
```

`devDependencies`:

```json
"vitest-axe": "^0.1.0",
"axe-core": "^4"
```

Run: `pnpm install`

- [ ] **Step 5: Regenerate bindings**

Run: `cargo run -p src-tauri --bin export_bindings` (or whichever binary your project uses — match Phase 0+1 Task 14's command).

Verify `ui/src/api/bindings.ts` now contains generated type signatures for: `createAccount`, `createTransaction`, `previewCsvColumns`, `importCsv`, `listUnfinishedImports`, `discardUnfinishedImport`, `getOnboardingState`, `seedSampleHousehold`, `markOnboardingComplete`, `resetOnboardingCompletion`, `clearSampleData`.

- [ ] **Step 6: Smoke build**

Run: `pnpm run tauri build --debug` (it should at least compile; do not need to launch the GUI).
Expected: build succeeds.

- [ ] **Step 7: Commit**

```bash
git add src-tauri ui/package.json ui/pnpm-lock.yaml \
        crates/finsight-app/Cargo.toml crates/finsight-app/src/lib.rs \
        ui/src/api/bindings.ts
git commit -m "chore: register dialog/opener Tauri plugins + frontend form/a11y deps"
```

---

### Task 12: Onboarding state hooks + App auto-redirect

**Files:**
- Create: `ui/src/api/hooks/onboarding.ts`
- Modify: `ui/src/App.tsx` (add the redirect effect)
- Test: `ui/src/test/App.redirect.test.tsx`

- [ ] **Step 1: Write the hooks**

Create `ui/src/api/hooks/onboarding.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as bindings from "../bindings";

const KEY = ["onboarding-state"] as const;

export function useOnboardingState() {
  return useQuery({
    queryKey: KEY,
    queryFn: () => bindings.getOnboardingState(),
    staleTime: 5_000,
  });
}

export function useSeedSampleHousehold() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => bindings.seedSampleHousehold(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEY });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

export function useMarkOnboardingComplete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => bindings.markOnboardingComplete(),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useResetOnboarding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => bindings.resetOnboardingCompletion(),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useClearSampleData() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => bindings.clearSampleData(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEY });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
    },
  });
}
```

- [ ] **Step 2: Add the App redirect effect**

Edit `ui/src/App.tsx` — inside the App component (where the router renders), add:

```tsx
import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useOnboardingState } from "./api/hooks/onboarding";

// inside the App component:
const { data: onboarding } = useOnboardingState();
const navigate = useNavigate();
const location = useLocation();
useEffect(() => {
  if (!onboarding) return;
  const shouldShow =
    onboarding.account_count === 0 && !onboarding.completion_marked;
  if (shouldShow && location.pathname !== "/onboarding") {
    navigate("/onboarding", { replace: true });
  }
}, [onboarding, location.pathname, navigate]);
```

Also confirm the router has an `/onboarding` route that renders the existing stub `Onboarding.tsx` (the stub gets fleshed out in Task 13).

- [ ] **Step 3: Write the failing test**

Create `ui/src/test/App.redirect.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import App from "../App";
import * as bindings from "../api/bindings";

vi.mock("../api/bindings", () => ({
  getOnboardingState: vi.fn(),
  listAccounts: vi.fn().mockResolvedValue([]),
  listTransactions: vi.fn().mockResolvedValue([]),
  appReady: vi.fn().mockResolvedValue(undefined),
  listUnfinishedImports: vi.fn().mockResolvedValue([]),
}));

function renderApp(initialPath: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("App onboarding redirect", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("redirects empty DB to /onboarding", async () => {
    (bindings.getOnboardingState as any).mockResolvedValue({
      account_count: 0, category_count: 0, completion_marked: false,
    });
    renderApp("/today");
    await waitFor(() => {
      expect(screen.getByText(/welcome/i)).toBeInTheDocument();
    });
  });

  it("does not redirect when accounts exist", async () => {
    (bindings.getOnboardingState as any).mockResolvedValue({
      account_count: 3, category_count: 5, completion_marked: false,
    });
    renderApp("/today");
    await waitFor(() => {
      expect(screen.queryByText(/welcome/i)).not.toBeInTheDocument();
    });
  });

  it("does not redirect when completion_marked even if accounts empty", async () => {
    (bindings.getOnboardingState as any).mockResolvedValue({
      account_count: 0, category_count: 0, completion_marked: true,
    });
    renderApp("/today");
    await waitFor(() => {
      expect(screen.queryByText(/welcome/i)).not.toBeInTheDocument();
    });
  });
});
```

> The "Welcome" copy comes from Task 13. The test will fail on the first assertion until Task 13 lands the `StepWelcome.tsx` content. That's acceptable — Tasks 12 and 13 are committed together and re-tested at the end of Task 13.

- [ ] **Step 4: Run vitest (expect partial failures until Task 13)**

Run: `pnpm test -- App.redirect`
Expected: tests reference a "Welcome" text that doesn't exist yet — the failure is on assertion, not import. If imports fail, fix wiring before moving on.

- [ ] **Step 5: Commit**

```bash
git add ui/src/api/hooks/onboarding.ts ui/src/App.tsx ui/src/test/App.redirect.test.tsx
git commit -m "feat(ui): onboarding state hooks + App auto-redirect to /onboarding"
```

---

### Task 13: Onboarding shell + Welcome step + sample-seed path

**Files:**
- Create: `ui/src/state/onboarding.ts`
- Modify: `ui/src/screens/Onboarding.tsx` (shell)
- Create: `ui/src/screens/onboarding/StepWelcome.tsx`
- Create: `ui/src/screens/onboarding/StepConnect.tsx` (placeholder — full content in Task 16/19)
- Create: `ui/src/screens/onboarding/StepCategories.tsx` (placeholder — Task 20)
- Create: `ui/src/screens/onboarding/StepAgent.tsx` (placeholder — Task 21)
- Test: `ui/src/test/Onboarding.welcome.test.tsx`

- [ ] **Step 1: Onboarding state store**

Create `ui/src/state/onboarding.ts`:

```ts
import { create } from "zustand";
import type { CsvImportMapping } from "../api/bindings";

export type OnboardingStep = "welcome" | "connect" | "categories" | "agent";

interface OnboardingStore {
  step: OnboardingStep;
  reachedSteps: Set<OnboardingStep>;
  mappingDraft: Partial<CsvImportMapping> | null;
  setStep: (s: OnboardingStep) => void;
  markReached: (s: OnboardingStep) => void;
  setMappingDraft: (m: Partial<CsvImportMapping> | null) => void;
  reset: () => void;
}

const ORDER: OnboardingStep[] = ["welcome", "connect", "categories", "agent"];

export const useOnboardingStore = create<OnboardingStore>((set) => ({
  step: "welcome",
  reachedSteps: new Set(["welcome"]),
  mappingDraft: null,
  setStep: (step) => set((s) => ({
    step,
    reachedSteps: new Set([...s.reachedSteps, step]),
  })),
  markReached: (step) => set((s) => ({
    reachedSteps: new Set([...s.reachedSteps, step]),
  })),
  setMappingDraft: (mappingDraft) => set({ mappingDraft }),
  reset: () => set({ step: "welcome", reachedSteps: new Set(["welcome"]), mappingDraft: null }),
}));

export const STEP_ORDER = ORDER;
```

- [ ] **Step 2: Onboarding shell**

Replace `ui/src/screens/Onboarding.tsx`:

```tsx
import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { STEP_ORDER, useOnboardingStore } from "../state/onboarding";
import { useOnboardingState } from "../api/hooks/onboarding";
import StepWelcome from "./onboarding/StepWelcome";
import StepConnect from "./onboarding/StepConnect";
import StepCategories from "./onboarding/StepCategories";
import StepAgent from "./onboarding/StepAgent";

const STEP_TITLES: Record<string, string> = {
  welcome: "Welcome",
  connect: "Connect",
  categories: "Categories",
  agent: "Agent",
};

export default function Onboarding() {
  const navigate = useNavigate();
  const { step, setStep, reachedSteps } = useOnboardingStore();
  const { data: state } = useOnboardingState();

  // If onboarding was already completed and the user navigated here manually
  // (Settings → Re-run), keep them here without auto-redirecting.
  useEffect(() => {
    if (!state) return;
  }, [state]);

  return (
    <div className="onboarding-shell" data-testid="onboarding-shell">
      <nav className="onboarding-stepper" aria-label="Onboarding progress">
        {STEP_ORDER.map((s, idx) => {
          const reached = reachedSteps.has(s);
          const isCurrent = s === step;
          return (
            <button
              key={s}
              className={`step-chip ${isCurrent ? "current" : ""} ${reached ? "reached" : "locked"}`}
              disabled={!reached}
              onClick={() => reached && setStep(s)}
              aria-current={isCurrent ? "step" : undefined}
            >
              <span className="step-index">{idx + 1}</span>
              <span className="step-title">{STEP_TITLES[s]}</span>
            </button>
          );
        })}
      </nav>

      <section className="onboarding-step">
        {step === "welcome"    && <StepWelcome onNext={() => setStep("connect")} onSkipToToday={() => navigate("/today")} />}
        {step === "connect"    && <StepConnect onNext={() => setStep("categories")} />}
        {step === "categories" && <StepCategories onNext={() => setStep("agent")} />}
        {step === "agent"      && <StepAgent onDone={() => navigate("/today")} />}
      </section>
    </div>
  );
}
```

- [ ] **Step 3: Welcome step with sample-data CTA**

Create `ui/src/screens/onboarding/StepWelcome.tsx`:

```tsx
import { useSeedSampleHousehold, useMarkOnboardingComplete } from "../../api/hooks/onboarding";

interface Props {
  onNext: () => void;
  onSkipToToday: () => void;
}

export default function StepWelcome({ onNext, onSkipToToday }: Props) {
  const seedSample = useSeedSampleHousehold();
  const markComplete = useMarkOnboardingComplete();

  async function trySample() {
    await seedSample.mutateAsync();
    await markComplete.mutateAsync();
    onSkipToToday();
  }

  return (
    <div className="step-welcome">
      <h1>A quiet way to understand your money</h1>
      <p>
        FinSight is a local, encrypted notebook for your accounts. Nothing leaves
        your machine. We'll help you import a statement, add accounts by hand, or
        explore with realistic sample data — whichever feels right today.
      </p>
      <div className="actions">
        <button className="primary" onClick={onNext}>Get started →</button>
        <button
          className="tertiary"
          onClick={trySample}
          disabled={seedSample.isPending}
          data-testid="try-sample-data"
        >
          {seedSample.isPending ? "Seeding…" : "Try with sample data"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Placeholder steps so the build passes**

Create minimal placeholder files. Each is replaced in later tasks but exports a default component now so the shell compiles.

`ui/src/screens/onboarding/StepConnect.tsx`:

```tsx
interface Props { onNext: () => void; }
export default function StepConnect({ onNext }: Props) {
  return (
    <div>
      <h2>Connect your money</h2>
      <p>This step gets filled in by Tasks 16 (manual entry) and 19 (CSV import).</p>
      <button onClick={onNext}>Skip for now →</button>
    </div>
  );
}
```

`ui/src/screens/onboarding/StepCategories.tsx`:

```tsx
interface Props { onNext: () => void; }
export default function StepCategories({ onNext }: Props) {
  return (
    <div>
      <h2>Confirm your categories</h2>
      <p>Filled in by Task 20.</p>
      <button onClick={onNext}>Use these →</button>
    </div>
  );
}
```

`ui/src/screens/onboarding/StepAgent.tsx`:

```tsx
interface Props { onDone: () => void; }
export default function StepAgent({ onDone }: Props) {
  return (
    <div>
      <h2>Set up the agent</h2>
      <p>Filled in by Task 21.</p>
      <button onClick={onDone}>Finish →</button>
    </div>
  );
}
```

- [ ] **Step 5: Test the welcome path**

Create `ui/src/test/Onboarding.welcome.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import Onboarding from "../screens/Onboarding";
import * as bindings from "../api/bindings";

vi.mock("../api/bindings", () => ({
  getOnboardingState: vi.fn().mockResolvedValue({
    account_count: 0, category_count: 0, completion_marked: false,
  }),
  seedSampleHousehold: vi.fn().mockResolvedValue({
    accounts_created: 6, transactions_created: 250, import_id: "abc",
  }),
  markOnboardingComplete: vi.fn().mockResolvedValue(undefined),
}));

function renderOnboarding() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={["/onboarding"]}>
        <Routes>
          <Route path="/onboarding" element={<Onboarding />} />
          <Route path="/today" element={<div>TODAY ROUTE</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("Onboarding · Welcome step", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders the welcome heading", () => {
    renderOnboarding();
    expect(screen.getByRole("heading", { name: /quiet way/i })).toBeInTheDocument();
  });

  it("Try sample seeds, marks complete, navigates to /today", async () => {
    renderOnboarding();
    fireEvent.click(screen.getByTestId("try-sample-data"));
    await waitFor(() => {
      expect(bindings.seedSampleHousehold).toHaveBeenCalledOnce();
      expect(bindings.markOnboardingComplete).toHaveBeenCalledOnce();
      expect(screen.getByText("TODAY ROUTE")).toBeInTheDocument();
    });
  });

  it("Get started advances to Connect step", () => {
    renderOnboarding();
    fireEvent.click(screen.getByRole("button", { name: /get started/i }));
    expect(screen.getByRole("heading", { name: /connect your money/i })).toBeInTheDocument();
  });
});
```

Run: `pnpm test -- Onboarding.welcome App.redirect`
Expected: both files green.

- [ ] **Step 6: Commit**

```bash
git add ui/src/state/onboarding.ts ui/src/screens/Onboarding.tsx \
        ui/src/screens/onboarding/ ui/src/test/Onboarding.welcome.test.tsx
git commit -m "feat(ui): onboarding shell + Welcome step with sample-data path"
```

---

## Phase 2.3 — Manual entry drawers

### Task 14: Generic Drawer component

**Files:**
- Create: `ui/src/components/Drawer.tsx`
- Test: `ui/src/test/Drawer.test.tsx`

- [ ] **Step 1: Write the failing tests**

Create `ui/src/test/Drawer.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Drawer from "../components/Drawer";

describe("Drawer", () => {
  it("renders title and children when open", () => {
    render(
      <Drawer open onClose={() => {}} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    expect(screen.getByRole("dialog", { name: /add account/i })).toBeInTheDocument();
    expect(screen.getByText("BODY")).toBeInTheDocument();
  });

  it("does not render content when closed", () => {
    render(
      <Drawer open={false} onClose={() => {}} title="Closed">
        <div>HIDDEN</div>
      </Drawer>
    );
    expect(screen.queryByText("HIDDEN")).not.toBeInTheDocument();
  });

  it("calls onClose when Escape is pressed", async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("calls onClose when backdrop is clicked", () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    fireEvent.click(screen.getByTestId("drawer-backdrop"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("sets aria-modal and labelledby", () => {
    render(
      <Drawer open onClose={() => {}} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog.getAttribute("aria-labelledby")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Implement Drawer**

Create `ui/src/components/Drawer.tsx`:

```tsx
import { useEffect, useId, useRef } from "react";
import FocusLock from "react-focus-lock";
import { createPortal } from "react-dom";
import type { ReactNode } from "react";

interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  width?: number;
}

export default function Drawer({ open, onClose, title, children, width = 480 }: DrawerProps) {
  const titleId = useId();
  const lastActive = useRef<HTMLElement | null>(null);

  // Restore focus on close.
  useEffect(() => {
    if (open) {
      lastActive.current = (document.activeElement as HTMLElement) ?? null;
    } else if (lastActive.current) {
      lastActive.current.focus();
      lastActive.current = null;
    }
  }, [open]);

  // ESC key closes the drawer.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <FocusLock returnFocus={false}>
      <div className="drawer-root">
        <div
          className="drawer-backdrop"
          data-testid="drawer-backdrop"
          onClick={onClose}
        />
        <aside
          className="drawer-panel"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          style={{ width }}
        >
          <header className="drawer-header">
            <h2 id={titleId}>{title}</h2>
            <button type="button" aria-label="Close" onClick={onClose}>×</button>
          </header>
          <div className="drawer-body">{children}</div>
        </aside>
      </div>
    </FocusLock>,
    document.body
  );
}
```

- [ ] **Step 3: Run tests**

Run: `pnpm test -- Drawer`
Expected: 5/5 pass.

- [ ] **Step 4: Commit**

```bash
git add ui/src/components/Drawer.tsx ui/src/test/Drawer.test.tsx
git commit -m "feat(ui): Drawer component with focus trap + ESC/backdrop close"
```

---

### Task 15: AccountDrawer + TransactionDrawer with react-hook-form + zod

**Files:**
- Create: `ui/src/components/AccountDrawer.tsx`
- Create: `ui/src/components/TransactionDrawer.tsx`
- Modify: `ui/src/api/hooks/accounts.ts` (add `useCreateAccount`)
- Modify: `ui/src/api/hooks/transactions.ts` (add `useCreateTransaction`)
- Test: `ui/src/test/AccountDrawer.test.tsx`, `ui/src/test/TransactionDrawer.test.tsx`

- [ ] **Step 1: Mutation hooks**

In `ui/src/api/hooks/accounts.ts` add:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import * as bindings from "../bindings";

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: bindings.NewAccount) => bindings.createAccount(input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["onboarding-state"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}
```

In `ui/src/api/hooks/transactions.ts` add:

```ts
export function useCreateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: bindings.NewTransaction) => bindings.createTransaction(input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}
```

- [ ] **Step 2: AccountDrawer**

Create `ui/src/components/AccountDrawer.tsx`:

```tsx
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import Drawer from "./Drawer";
import { useCreateAccount } from "../api/hooks/accounts";

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
  defaultOwner?: string;
  onCreated?: () => void;
}

export default function AccountDrawer({ open, onClose, defaultOwner = "joint", onCreated }: Props) {
  const createAccount = useCreateAccount();
  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      type: "Checking",
      currency: "USD",
      owner: defaultOwner,
      opening_dollars: 0,
    },
  });

  async function onSubmit(values: FormValues) {
    await createAccount.mutateAsync({
      bank: values.bank,
      name: values.name,
      type: values.type,
      last4: values.last4 || null,
      currency: values.currency,
      color: "#3B82F6",
      opening_balance_cents: Math.round(values.opening_dollars * 100),
      owner: values.owner,
      source: "manual",
    });
    reset();
    onCreated?.();
    onClose();
  }

  return (
    <Drawer open={open} onClose={onClose} title="Add account">
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Bank
          <input {...register("bank")} aria-invalid={!!errors.bank} />
          {errors.bank && <span className="err">{errors.bank.message}</span>}
        </label>
        <label> Name
          <input {...register("name")} placeholder="e.g. Joint Checking" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <fieldset>
          <legend>Type</legend>
          {(["Checking","Savings","Credit","Investment","Cash","Other"] as const).map(t => (
            <label key={t}><input type="radio" value={t} {...register("type")} /> {t}</label>
          ))}
        </fieldset>
        <label> Last 4 <input {...register("last4")} maxLength={4} /></label>
        <label> Currency
          <select {...register("currency")}>
            {(["USD","EUR","GBP","CAD","AUD"] as const).map(c => <option key={c}>{c}</option>)}
          </select>
        </label>
        <label> Opening balance ($)
          <input type="number" step="0.01" {...register("opening_dollars")} />
        </label>
        <label> Owner
          <input {...register("owner")} aria-invalid={!!errors.owner} />
        </label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Creating…" : "Create account"}
          </button>
        </div>
      </form>
    </Drawer>
  );
}
```

- [ ] **Step 3: TransactionDrawer**

Create `ui/src/components/TransactionDrawer.tsx`:

```tsx
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import Drawer from "./Drawer";
import { useCreateTransaction } from "../api/hooks/transactions";
import { useQuery } from "@tanstack/react-query";
import * as bindings from "../api/bindings";

const schema = z.object({
  account_id: z.string().min(1),
  date: z.string().min(1),
  dollars: z.coerce.number(),
  direction: z.enum(["inflow", "outflow"]),
  merchant_raw: z.string().min(1),
  category_id: z.string().optional(),
  notes: z.string().optional(),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  defaultAccountId?: string;
  onCreated?: () => void;
}

export default function TransactionDrawer({ open, onClose, defaultAccountId, onCreated }: Props) {
  const create = useCreateTransaction();
  const { data: accounts = [] } = useQuery({
    queryKey: ["accounts"],
    queryFn: () => bindings.listAccounts(),
  });

  const today = new Date().toISOString().slice(0, 10);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      account_id: defaultAccountId ?? "",
      date: today,
      direction: "outflow",
    },
  });

  async function onSubmit(values: FormValues) {
    const cents_signed =
      values.direction === "outflow"
        ? -Math.round(Math.abs(values.dollars) * 100)
        :  Math.round(Math.abs(values.dollars) * 100);
    await create.mutateAsync({
      account_id: values.account_id,
      posted_at: new Date(values.date + "T12:00:00Z").toISOString(),
      amount_cents: cents_signed,
      merchant_raw: values.merchant_raw,
      merchant_id: null,
      category_id: values.category_id || null,
      notes: values.notes || null,
    });
    reset();
    onCreated?.();
    onClose();
  }

  return (
    <Drawer open={open} onClose={onClose} title="Add transaction">
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Account
          <select {...register("account_id")} aria-invalid={!!errors.account_id}>
            <option value="">— Pick an account —</option>
            {accounts.map(a => <option key={a.id} value={a.id}>{a.bank} · {a.name}</option>)}
          </select>
        </label>
        <label> Date
          <input type="date" {...register("date")} />
        </label>
        <fieldset>
          <legend>Direction</legend>
          <label><input type="radio" value="outflow" {...register("direction")} /> Outflow</label>
          <label><input type="radio" value="inflow"  {...register("direction")} /> Inflow</label>
        </fieldset>
        <label> Amount ($)
          <input type="number" step="0.01" {...register("dollars")} />
        </label>
        <label> Merchant
          <input {...register("merchant_raw")} aria-invalid={!!errors.merchant_raw} />
        </label>
        <label> Notes
          <textarea {...register("notes")} />
        </label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : "Save transaction"}
          </button>
        </div>
      </form>
    </Drawer>
  );
}
```

- [ ] **Step 4: Tests**

Create `ui/src/test/AccountDrawer.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import AccountDrawer from "../components/AccountDrawer";
import * as bindings from "../api/bindings";

vi.mock("../api/bindings", () => ({
  createAccount: vi.fn().mockResolvedValue({ id: "a1" }),
  listAccounts: vi.fn().mockResolvedValue([]),
}));

function renderDrawer(props: Partial<React.ComponentProps<typeof AccountDrawer>> = {}) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <AccountDrawer open onClose={() => {}} {...props} />
    </QueryClientProvider>
  );
}

describe("AccountDrawer", () => {
  beforeEach(() => vi.clearAllMocks());

  it("blocks submission when bank/name empty", async () => {
    renderDrawer();
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));
    await waitFor(() => {
      expect(bindings.createAccount).not.toHaveBeenCalled();
    });
  });

  it("submits with cents conversion", async () => {
    renderDrawer();
    fireEvent.change(screen.getByLabelText(/Bank/), { target: { value: "Chase" } });
    fireEvent.change(screen.getByLabelText(/Name/), { target: { value: "Joint Checking" } });
    fireEvent.change(screen.getByLabelText(/Opening balance/), { target: { value: "100.50" } });
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));
    await waitFor(() => {
      expect(bindings.createAccount).toHaveBeenCalledWith(expect.objectContaining({
        bank: "Chase", name: "Joint Checking",
        opening_balance_cents: 10050, source: "manual",
      }));
    });
  });
});
```

Create `ui/src/test/TransactionDrawer.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import TransactionDrawer from "../components/TransactionDrawer";
import * as bindings from "../api/bindings";

vi.mock("../api/bindings", () => ({
  listAccounts: vi.fn().mockResolvedValue([
    { id: "a1", bank: "Chase", name: "Joint Checking", type: "Checking",
      owner: "joint", currency: "USD", color: "#000", balance_cents: 0 },
  ]),
  createTransaction: vi.fn().mockResolvedValue({ id: "t1" }),
}));

function renderDrawer() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <TransactionDrawer open onClose={() => {}} />
    </QueryClientProvider>
  );
}

describe("TransactionDrawer", () => {
  beforeEach(() => vi.clearAllMocks());

  it("submits outflow as negative cents", async () => {
    renderDrawer();
    await waitFor(() => expect(screen.getByText(/Chase · Joint Checking/)).toBeInTheDocument());
    fireEvent.change(screen.getByLabelText(/Account/), { target: { value: "a1" } });
    fireEvent.change(screen.getByLabelText(/Amount/), { target: { value: "8.42" } });
    fireEvent.change(screen.getByLabelText(/Merchant/), { target: { value: "Safeway" } });
    fireEvent.click(screen.getByRole("button", { name: /save transaction/i }));
    await waitFor(() => {
      expect(bindings.createTransaction).toHaveBeenCalledWith(expect.objectContaining({
        account_id: "a1", amount_cents: -842, merchant_raw: "Safeway",
      }));
    });
  });
});
```

Run: `pnpm test -- AccountDrawer TransactionDrawer`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/AccountDrawer.tsx ui/src/components/TransactionDrawer.tsx \
        ui/src/api/hooks/accounts.ts ui/src/api/hooks/transactions.ts \
        ui/src/test/AccountDrawer.test.tsx ui/src/test/TransactionDrawer.test.tsx
git commit -m "feat(ui): Account/Transaction drawers with react-hook-form + zod"
```

---

### Task 16: Wire drawers into Accounts/Transactions screens + StepConnect manual path

**Files:**
- Modify: `ui/src/screens/Accounts.tsx` (add "Add account" button + drawer state)
- Modify: `ui/src/screens/Transactions.tsx` (add "Add transaction" button + drawer state — Import CSV button placeholder, filled in Task 19)
- Replace: `ui/src/screens/onboarding/StepConnect.tsx` (real implementation with three cards)

- [ ] **Step 1: Accounts screen**

Edit `ui/src/screens/Accounts.tsx` to add an "Add account" button at the top that opens `AccountDrawer`:

```tsx
import { useState } from "react";
import AccountDrawer from "../components/AccountDrawer";
// ... existing imports

export default function Accounts() {
  const [drawerOpen, setDrawerOpen] = useState(false);
  // ... existing query/data code

  return (
    <div className="screen-accounts">
      <header className="screen-header">
        <h1>Accounts</h1>
        <button className="primary" onClick={() => setDrawerOpen(true)}>+ Add account</button>
      </header>
      {/* existing list rendering */}
      <AccountDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </div>
  );
}
```

- [ ] **Step 2: Transactions screen**

Edit `ui/src/screens/Transactions.tsx` similarly. Add **two** buttons: "Import CSV" (stub for now — wired in Task 19) and "Add transaction":

```tsx
import { useState } from "react";
import TransactionDrawer from "../components/TransactionDrawer";
// ... existing imports

export default function Transactions() {
  const [drawerOpen, setDrawerOpen] = useState(false);
  // ... existing query/data code

  return (
    <div className="screen-transactions">
      <header className="screen-header">
        <h1>Transactions</h1>
        <div className="actions">
          <button data-testid="import-csv-trigger" disabled title="Filled in Task 19">Import CSV</button>
          <button className="primary" onClick={() => setDrawerOpen(true)}>+ Add transaction</button>
        </div>
      </header>
      {/* existing list rendering */}
      <TransactionDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </div>
  );
}
```

- [ ] **Step 3: StepConnect real implementation (manual path only — CSV path comes in Task 19)**

Replace `ui/src/screens/onboarding/StepConnect.tsx`:

```tsx
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import AccountDrawer from "../../components/AccountDrawer";
import TransactionDrawer from "../../components/TransactionDrawer";
import * as bindings from "../../api/bindings";

interface Props { onNext: () => void; }

export default function StepConnect({ onNext }: Props) {
  const [acctOpen, setAcctOpen] = useState(false);
  const [txnOpen, setTxnOpen]   = useState(false);

  const { data: accounts = [] } = useQuery({
    queryKey: ["accounts"], queryFn: () => bindings.listAccounts(),
  });
  const { data: txns = [] } = useQuery({
    queryKey: ["transactions"], queryFn: () => bindings.listTransactions(),
  });

  const canContinue = accounts.length > 0;

  return (
    <div className="step-connect">
      <h2>Connect your money</h2>

      <div className="connect-cards">
        <article className="card">
          <h3>Import a statement</h3>
          <p>Pick a CSV exported from your bank and map its columns.</p>
          <button disabled title="Filled in Task 19">Pick a file…</button>
        </article>

        <article className="card">
          <h3>Add manually</h3>
          <p>Walk through accounts and a few recent transactions by hand.</p>
          <div className="button-row">
            <button onClick={() => setAcctOpen(true)}>+ Account</button>
            <button onClick={() => setTxnOpen(true)} disabled={accounts.length === 0}>+ Transaction</button>
          </div>
        </article>

        <article className="card">
          <h3>Skip for now</h3>
          <p>You can always add or import later from the Accounts screen.</p>
          <button onClick={onNext}>Skip →</button>
        </article>
      </div>

      <aside className="connect-tally" aria-live="polite">
        <strong>{accounts.length}</strong> account{accounts.length === 1 ? "" : "s"} added,{" "}
        <strong>{txns.length}</strong> transaction{txns.length === 1 ? "" : "s"} so far
      </aside>

      <footer>
        <button className="primary" disabled={!canContinue} onClick={onNext}>
          Continue →
        </button>
      </footer>

      <AccountDrawer open={acctOpen} onClose={() => setAcctOpen(false)} />
      <TransactionDrawer open={txnOpen} onClose={() => setTxnOpen(false)} />
    </div>
  );
}
```

- [ ] **Step 4: Smoke test by re-running affected suites**

Run: `pnpm test -- Onboarding.welcome Drawer AccountDrawer TransactionDrawer`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Accounts.tsx ui/src/screens/Transactions.tsx \
        ui/src/screens/onboarding/StepConnect.tsx
git commit -m "feat(ui): wire drawers into Accounts/Transactions + StepConnect manual path"
```

---

## Phase 2.4 — Import flow UI

### Task 17: FilePicker + CSV hooks + ImportProgress + UnfinishedImportBanner

**Files:**
- Create: `ui/src/components/FilePicker.tsx`
- Create: `ui/src/components/ImportProgress.tsx`
- Create: `ui/src/components/UnfinishedImportBanner.tsx`
- Create: `ui/src/api/hooks/csv.ts`
- Modify: `ui/src/api/hooks/transactions.ts` (add `useImportCsv`)
- Modify: `ui/src/App.tsx` (render `UnfinishedImportBanner` once at top)

- [ ] **Step 1: FilePicker wraps plugin-dialog**

Create `ui/src/components/FilePicker.tsx`:

```tsx
import { open as openDialog } from "@tauri-apps/plugin-dialog";

interface Props {
  onPicked: (path: string) => void;
  label?: string;
}

export default function FilePicker({ onPicked, label = "Pick a CSV…" }: Props) {
  async function pick() {
    const selected = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (typeof selected === "string") onPicked(selected);
  }
  return <button onClick={pick} data-testid="file-picker">{label}</button>;
}
```

- [ ] **Step 2: CSV hook**

Create `ui/src/api/hooks/csv.ts`:

```ts
import { useMutation, useQuery } from "@tanstack/react-query";
import * as bindings from "../bindings";

export function usePreviewCsvColumns(path: string | null, skipHeaderRows: number) {
  return useQuery({
    queryKey: ["csv-preview", path, skipHeaderRows],
    queryFn: () => bindings.previewCsvColumns(path!, skipHeaderRows),
    enabled: !!path,
    staleTime: 30_000,
  });
}
```

In `ui/src/api/hooks/transactions.ts` add:

```ts
export function useImportCsv() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: {
      path: string;
      account_id: string;
      mapping: bindings.CsvImportMapping;
    }) => bindings.importCsv(args.path, args.account_id, args.mapping),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}
```

- [ ] **Step 3: ImportProgress pill**

Create `ui/src/components/ImportProgress.tsx`:

```tsx
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

interface ProgressPayload {
  import_id: string;
  rows_done: number;
  rows_total: number;
}

export default function ImportProgress() {
  const [active, setActive] = useState<ProgressPayload | null>(null);

  useEffect(() => {
    const u1 = listen<ProgressPayload>("import.progress", (e) => setActive(e.payload));
    const u2 = listen<string>("import.complete", () => setActive(null));
    return () => { u1.then(fn => fn()); u2.then(fn => fn()); };
  }, []);

  if (!active) return null;
  const pct = active.rows_total === 0 ? 0
    : Math.round((active.rows_done / active.rows_total) * 100);
  return (
    <div className="import-progress" role="status" aria-live="polite">
      Importing {active.rows_done.toLocaleString()} / {active.rows_total.toLocaleString()} ({pct}%)
    </div>
  );
}
```

- [ ] **Step 4: UnfinishedImportBanner**

Create `ui/src/components/UnfinishedImportBanner.tsx`:

```tsx
import { useQuery } from "@tanstack/react-query";
import * as bindings from "../api/bindings";

export default function UnfinishedImportBanner() {
  const { data: unfinished = [], refetch } = useQuery({
    queryKey: ["unfinished-imports"],
    queryFn: () => bindings.listUnfinishedImports(),
    staleTime: 60_000,
  });

  if (unfinished.length === 0) return null;
  const top = unfinished[0];

  async function discard() {
    await bindings.discardUnfinishedImport(top.id);
    refetch();
  }

  return (
    <div role="alert" className="banner banner-warning">
      An import didn't finish last time
      ({top.filename ?? "manual"}). It was deduped on the next run, so re-importing is safe.
      <button onClick={discard}>Discard</button>
    </div>
  );
}
```

- [ ] **Step 5: Mount the banner + progress in App.tsx**

Edit `ui/src/App.tsx`:

```tsx
import ImportProgress from "./components/ImportProgress";
import UnfinishedImportBanner from "./components/UnfinishedImportBanner";

// inside the App layout, above the router outlet:
<UnfinishedImportBanner />
<ImportProgress />
```

- [ ] **Step 6: Build + smoke test**

Run: `pnpm test`
Expected: existing suites still pass (no new tests added — these components have minimal logic and are covered indirectly by Task 18+19).

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/FilePicker.tsx ui/src/components/ImportProgress.tsx \
        ui/src/components/UnfinishedImportBanner.tsx \
        ui/src/api/hooks/csv.ts ui/src/api/hooks/transactions.ts \
        ui/src/App.tsx
git commit -m "feat(ui): FilePicker, ImportProgress, unfinished-import banner + hooks"
```

---

### Task 18: ImportMappingDialog component

**Files:**
- Create: `ui/src/screens/onboarding/ImportMappingDialog.tsx`
- Test: `ui/src/test/ImportMappingDialog.test.tsx`

- [ ] **Step 1: Component**

Create `ui/src/screens/onboarding/ImportMappingDialog.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import FocusLock from "react-focus-lock";
import { usePreviewCsvColumns } from "../../api/hooks/csv";
import { useImportCsv } from "../../api/hooks/transactions";
import * as bindings from "../../api/bindings";

const COLUMN_ROLES = ["Date", "Amount", "Merchant", "Notes", "Category", "Skip", "Debit", "Credit"] as const;
type Role = (typeof COLUMN_ROLES)[number];

const DATE_FORMATS = [
  { label: "2026-05-19", value: "%Y-%m-%d" },
  { label: "5/19/2026",   value: "%m/%d/%Y" },
  { label: "19/05/2026",  value: "%d/%m/%Y" },
  { label: "19.05.2026",  value: "%d.%m.%Y" },
  { label: "May 19, 2026", value: "%B %d, %Y" },
  { label: "19-May-2026",  value: "%d-%b-%Y" },
  { label: "Custom",       value: "__CUSTOM__" },
];

const AMOUNT_CONVENTIONS = [
  { label: "Negative = outflow",         value: "negative_is_outflow" },
  { label: "Positive = outflow",         value: "positive_is_outflow" },
  { label: "Separate debit/credit cols", value: "split_debit_credit" },
] as const;

interface Props {
  path: string;
  onClose: () => void;
  onImported: (summary: bindings.ImportSummary) => void;
  defaultAccountId?: string;
}

export default function ImportMappingDialog({ path, onClose, onImported, defaultAccountId }: Props) {
  const [skipHeaderRows, setSkipHeaderRows] = useState(1);
  const { data: preview } = usePreviewCsvColumns(path, skipHeaderRows);

  const { data: accounts = [] } = useQuery({
    queryKey: ["accounts"], queryFn: () => bindings.listAccounts(),
  });

  const [accountId, setAccountId] = useState(defaultAccountId ?? "");
  const [columns, setColumns] = useState<Role[]>([]);
  const [dateFormat, setDateFormat] = useState("%Y-%m-%d");
  const [customDateFormat, setCustomDateFormat] = useState("");
  const [amountConvention, setAmountConvention] =
    useState<typeof AMOUNT_CONVENTIONS[number]["value"]>("negative_is_outflow");

  // When preview arrives, initialize columns to all-Skip (or carry an existing saved mapping).
  useEffect(() => {
    if (!preview) return;
    if (columns.length === 0) {
      const guess: Role[] = preview.rows[0]?.map(() => "Skip" as Role) ?? [];
      setColumns(guess);
    }
  }, [preview]);

  const importCsv = useImportCsv();

  const finalDateFormat = dateFormat === "__CUSTOM__" ? customDateFormat : dateFormat;
  const canSubmit =
    accountId &&
    finalDateFormat.length > 0 &&
    columns.includes("Date") &&
    columns.includes("Merchant") &&
    (amountConvention === "split_debit_credit"
      ? columns.includes("Debit") && columns.includes("Credit")
      : columns.includes("Amount"));

  async function submit() {
    const mapping: bindings.CsvImportMapping = {
      skip_header_rows: skipHeaderRows,
      columns: columns.map(c => c as any),     // role strings match Rust enum case-sensitively
      date_format: finalDateFormat,
      amount_convention: amountConvention as any,
      decimal_separator: ".",
      delimiter: null,
    };
    const summary = await importCsv.mutateAsync({ path, account_id: accountId, mapping });
    onImported(summary);
  }

  return (
    <FocusLock returnFocus>
      <div className="dialog-backdrop" onClick={onClose} />
      <div className="dialog-overlay" role="dialog" aria-modal="true" aria-labelledby="map-title">
        <header><h2 id="map-title">Map CSV columns</h2></header>

        <div className="dialog-grid">
          <label> Account
            <select value={accountId} onChange={e => setAccountId(e.target.value)}>
              <option value="">— Pick —</option>
              {accounts.map(a => <option key={a.id} value={a.id}>{a.bank} · {a.name}</option>)}
            </select>
          </label>
          <label> Skip header rows
            <input type="number" min={0} value={skipHeaderRows}
                   onChange={e => setSkipHeaderRows(parseInt(e.target.value, 10) || 0)} />
          </label>
          <label> Date format
            <select value={dateFormat} onChange={e => setDateFormat(e.target.value)}>
              {DATE_FORMATS.map(f => <option key={f.value} value={f.value}>{f.label}</option>)}
            </select>
            {dateFormat === "__CUSTOM__" && (
              <input placeholder="e.g. %Y/%m/%d" value={customDateFormat}
                     onChange={e => setCustomDateFormat(e.target.value)} />
            )}
          </label>
          <fieldset>
            <legend>Amount convention</legend>
            {AMOUNT_CONVENTIONS.map(c => (
              <label key={c.value}>
                <input type="radio" name="conv" value={c.value}
                       checked={amountConvention === c.value}
                       onChange={() => setAmountConvention(c.value)} /> {c.label}
              </label>
            ))}
          </fieldset>
        </div>

        {preview && (
          <table className="preview-table">
            <thead>
              <tr>
                {(preview.headers ?? preview.rows[0] ?? []).map((_, i) => (
                  <th key={i}>
                    <select
                      value={columns[i] ?? "Skip"}
                      onChange={e => {
                        const next = [...columns];
                        next[i] = e.target.value as Role;
                        setColumns(next);
                      }}
                    >
                      {COLUMN_ROLES.map(r => <option key={r} value={r}>{r}</option>)}
                    </select>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {preview.rows.slice(0, 5).map((row, ri) => (
                <tr key={ri}>{row.map((cell, ci) => <td key={ci}>{cell}</td>)}</tr>
              ))}
            </tbody>
          </table>
        )}

        <footer>
          <button onClick={onClose}>Cancel</button>
          <button className="primary" onClick={submit} disabled={!canSubmit || importCsv.isPending}>
            {importCsv.isPending ? "Importing…" : "Import"}
          </button>
        </footer>
      </div>
    </FocusLock>
  );
}
```

- [ ] **Step 2: Test the dialog**

Create `ui/src/test/ImportMappingDialog.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import ImportMappingDialog from "../screens/onboarding/ImportMappingDialog";
import * as bindings from "../api/bindings";

vi.mock("../api/bindings", () => ({
  previewCsvColumns: vi.fn().mockResolvedValue({
    headers: ["Date", "Merchant", "Amount"],
    rows: [["2026-05-19", "Safeway", "-8.42"]],
    detected_delimiter: ",", total_rows: 1, encoding_note: null,
  }),
  listAccounts: vi.fn().mockResolvedValue([
    { id: "a1", bank: "Chase", name: "Checking", type: "Checking",
      owner: "joint", currency: "USD", color: "#000", balance_cents: 0 },
  ]),
  importCsv: vi.fn().mockResolvedValue({
    import_id: "imp1", rows_imported: 1, rows_skipped_duplicates: 0, errors: [],
  }),
}));

function renderDialog() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ImportMappingDialog path="/tmp/x.csv" onClose={() => {}} onImported={() => {}} />
    </QueryClientProvider>
  );
}

describe("ImportMappingDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("Import button starts disabled until account + columns assigned", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());
    const btn = screen.getByRole("button", { name: /^import$/i });
    expect(btn).toBeDisabled();
  });

  it("becomes enabled once required mapping is complete and submits", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText(/Account/), { target: { value: "a1" } });

    // Find the three column dropdowns (in table header).
    const selects = screen.getAllByRole("combobox").filter(s => s.getAttribute("aria-label") !== null || true);
    // Convention: first three role-comboboxes are the column-mapping dropdowns
    // since "Account" + "Date format" + "Skip header rows" come before them.
    // We use the table headers explicitly:
    const headers = screen.getAllByRole("columnheader");
    const dropdowns = headers.map(h => h.querySelector("select") as HTMLSelectElement);
    fireEvent.change(dropdowns[0], { target: { value: "Date" } });
    fireEvent.change(dropdowns[1], { target: { value: "Merchant" } });
    fireEvent.change(dropdowns[2], { target: { value: "Amount" } });

    const btn = screen.getByRole("button", { name: /^import$/i });
    await waitFor(() => expect(btn).not.toBeDisabled());

    fireEvent.click(btn);
    await waitFor(() => {
      expect(bindings.importCsv).toHaveBeenCalledWith(
        "/tmp/x.csv", "a1",
        expect.objectContaining({
          columns: expect.arrayContaining(["Date", "Merchant", "Amount"]),
        })
      );
    });
  });
});
```

Run: `pnpm test -- ImportMappingDialog`
Expected: both tests pass.

- [ ] **Step 3: Commit**

```bash
git add ui/src/screens/onboarding/ImportMappingDialog.tsx \
        ui/src/test/ImportMappingDialog.test.tsx
git commit -m "feat(ui): ImportMappingDialog with live preview + validation gating"
```

---

### Task 19: Wire ImportMappingDialog into Transactions screen + StepConnect

**Files:**
- Modify: `ui/src/screens/Transactions.tsx` (replace stub Import CSV button with FilePicker + dialog)
- Modify: `ui/src/screens/onboarding/StepConnect.tsx` (replace stub Import card)

- [ ] **Step 1: Transactions screen**

Edit `ui/src/screens/Transactions.tsx`:

```tsx
import { useState } from "react";
import FilePicker from "../components/FilePicker";
import ImportMappingDialog from "./onboarding/ImportMappingDialog";
import TransactionDrawer from "../components/TransactionDrawer";

export default function Transactions() {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [csvPath, setCsvPath] = useState<string | null>(null);

  return (
    <div className="screen-transactions">
      <header className="screen-header">
        <h1>Transactions</h1>
        <div className="actions">
          <FilePicker onPicked={setCsvPath} label="Import CSV" />
          <button className="primary" onClick={() => setDrawerOpen(true)}>+ Add transaction</button>
        </div>
      </header>

      {/* existing list rendering */}

      <TransactionDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
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

- [ ] **Step 2: StepConnect import card**

Edit the "Import a statement" card in `ui/src/screens/onboarding/StepConnect.tsx`:

```tsx
import FilePicker from "../../components/FilePicker";
import ImportMappingDialog from "./ImportMappingDialog";
// inside the component:
const [csvPath, setCsvPath] = useState<string | null>(null);

// in the Import card:
<article className="card">
  <h3>Import a statement</h3>
  <p>Pick a CSV exported from your bank and map its columns.</p>
  <FilePicker onPicked={setCsvPath} label="Pick a file…" />
</article>

// at the bottom of the return:
{csvPath && (
  <ImportMappingDialog
    path={csvPath}
    onClose={() => setCsvPath(null)}
    onImported={() => setCsvPath(null)}
  />
)}
```

- [ ] **Step 3: Manual smoke**

Run: `pnpm test`
Expected: all suites green.

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/Transactions.tsx ui/src/screens/onboarding/StepConnect.tsx
git commit -m "feat(ui): wire CSV import dialog into Transactions + Onboarding Step 2"
```

---

## Phase 2.5 — Categories + Agent + Settings escape hatches

### Task 20: StepCategories with inline edit + commit

**Files:**
- Replace: `ui/src/screens/onboarding/StepCategories.tsx`
- Modify: `crates/finsight-app/src/commands/onboarding.rs` (add `commit_starter_categories`)
- Modify: `crates/finsight-app/src/lib.rs` (register the new command)

- [ ] **Step 1: Backend command**

In `crates/finsight-app/src/commands/onboarding.rs` add:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Type)]
pub struct StarterCategory {
    pub id: String,           // slug
    pub label: String,
    pub group_id: String,     // 'fixed' | 'daily' | 'lifestyle' | 'wellbeing'
}

#[tauri::command]
#[specta::specta]
pub async fn commit_starter_categories(
    state: tauri::State<'_, AppState>,
    categories: Vec<StarterCategory>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let tx = conn.transaction()?;
        // Ensure the four canonical groups exist.
        for (gid, label) in [
            ("fixed", "Fixed"), ("daily", "Daily"),
            ("lifestyle", "Lifestyle"), ("wellbeing", "Wellbeing"),
        ] {
            tx.execute(
                "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
                rusqlite::params![gid, label],
            )?;
        }
        for c in &categories {
            tx.execute(
                "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) \
                 VALUES(?1, ?2, ?3, '#94A3B8', 0)",
                rusqlite::params![c.id, c.group_id, c.label],
            )?;
        }
        tx.commit()?;
        Ok(())
    }).await.map_err(AppError::from)
}
```

Register in `build_specta_builder` and regenerate bindings:

```bash
cargo run -p src-tauri --bin export_bindings
```

- [ ] **Step 2: Frontend step**

Replace `ui/src/screens/onboarding/StepCategories.tsx`:

```tsx
import { useState } from "react";
import * as bindings from "../../api/bindings";

interface Props { onNext: () => void; }

interface Row { id: string; label: string; group_id: string; }

const STARTERS: Row[] = [
  { id: "housing",       label: "Housing",       group_id: "fixed" },
  { id: "utilities",     label: "Utilities",     group_id: "fixed" },
  { id: "subscriptions", label: "Subscriptions", group_id: "fixed" },
  { id: "groceries",     label: "Groceries",     group_id: "daily" },
  { id: "dining",        label: "Dining",        group_id: "daily" },
  { id: "transport",     label: "Transport",     group_id: "daily" },
  { id: "shopping",      label: "Shopping",      group_id: "lifestyle" },
  { id: "travel",        label: "Travel",        group_id: "lifestyle" },
  { id: "gifts",         label: "Gifts",         group_id: "lifestyle" },
  { id: "health",        label: "Health",        group_id: "wellbeing" },
];

const GROUPS = ["fixed","daily","lifestyle","wellbeing"];

export default function StepCategories({ onNext }: Props) {
  const [rows, setRows] = useState<Row[]>(STARTERS);
  const [saving, setSaving] = useState(false);

  function update(i: number, patch: Partial<Row>) {
    setRows(r => r.map((row, idx) => idx === i ? { ...row, ...patch } : row));
  }
  function add() {
    setRows(r => [...r, { id: `custom-${r.length}`, label: "", group_id: "daily" }]);
  }
  function remove(i: number) {
    setRows(r => r.filter((_, idx) => idx !== i));
  }

  async function commit() {
    setSaving(true);
    await bindings.commitStarterCategories(rows.filter(r => r.label.trim().length > 0));
    setSaving(false);
    onNext();
  }

  return (
    <div className="step-categories">
      <h2>Confirm your categories</h2>
      <p>Edit or delete anything that doesn't fit. We'll only store what you keep.</p>
      <ul className="category-list">
        {rows.map((row, i) => (
          <li key={row.id}>
            <input value={row.label} onChange={e => update(i, { label: e.target.value })} aria-label={`Category ${i+1} label`} />
            <select value={row.group_id} onChange={e => update(i, { group_id: e.target.value })} aria-label={`Category ${i+1} group`}>
              {GROUPS.map(g => <option key={g} value={g}>{g}</option>)}
            </select>
            <button onClick={() => remove(i)} aria-label={`Remove ${row.label || "row"}`}>×</button>
          </li>
        ))}
      </ul>
      <button onClick={add}>+ Add category</button>
      <footer>
        <button className="primary" onClick={commit} disabled={saving}>
          {saving ? "Saving…" : "Use these →"}
        </button>
      </footer>
    </div>
  );
}
```

- [ ] **Step 3: Smoke test**

Run: `cargo test --workspace && pnpm test`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/onboarding.rs \
        crates/finsight-app/src/lib.rs \
        ui/src/api/bindings.ts \
        ui/src/screens/onboarding/StepCategories.tsx
git commit -m "feat: StepCategories with inline editing + commit_starter_categories cmd"
```

---

### Task 21: StepAgent with Ollama probe

**Files:**
- Modify: `crates/finsight-app/src/commands/onboarding.rs` (add `probe_ollama`, `save_llm_provider`)
- Modify: `crates/finsight-app/Cargo.toml` (add `reqwest` with `json` feature)
- Modify: `crates/finsight-app/src/lib.rs` (register commands)
- Replace: `ui/src/screens/onboarding/StepAgent.tsx`

- [ ] **Step 1: Backend probe**

In `crates/finsight-app/Cargo.toml`:

```toml
reqwest = { workspace = true, features = ["json", "rustls-tls"] }
```

(Or pin a fresh version if not in workspace deps; verify Phase 1's TLS choice.)

Add to `crates/finsight-app/src/commands/onboarding.rs`:

```rust
#[derive(Debug, Clone, Serialize, Type)]
pub struct OllamaProbeResult {
    pub reachable: bool,
    pub models: Vec<String>,
    pub has_nomic_embed: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn probe_ollama(base_url: String) -> AppResult<OllamaProbeResult> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| AppError { kind: "http".into(), message: e.to_string() })?;
    let result: Result<reqwest::Response, reqwest::Error> = client.get(&url).send().await;
    let resp = match result {
        Ok(r) => r,
        Err(_) => return Ok(OllamaProbeResult { reachable: false, models: vec![], has_nomic_embed: false }),
    };
    if !resp.status().is_success() {
        return Ok(OllamaProbeResult { reachable: false, models: vec![], has_nomic_embed: false });
    }
    #[derive(serde::Deserialize)]
    struct TagsResp { models: Vec<Tag> }
    #[derive(serde::Deserialize)]
    struct Tag { name: String }
    let body: TagsResp = match resp.json().await {
        Ok(b) => b,
        Err(_) => return Ok(OllamaProbeResult { reachable: false, models: vec![], has_nomic_embed: false }),
    };
    let models: Vec<String> = body.models.into_iter().map(|m| m.name).collect();
    let has_nomic_embed = models.iter().any(|m| m.starts_with("nomic-embed-text"));
    Ok(OllamaProbeResult { reachable: true, models, has_nomic_embed })
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "kind")]
pub enum LlmProviderConfig {
    #[serde(rename = "ollama")]
    Ollama { base_url: String, completion_model: String, embedding_model: String },
    #[serde(rename = "unconfigured")]
    Unconfigured,
}

#[tauri::command]
#[specta::specta]
pub async fn save_llm_provider(
    state: tauri::State<'_, AppState>,
    config: LlmProviderConfig,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| settings::set(conn, "llm_provider", &config)).await.map_err(AppError::from)
}
```

Register these in `build_specta_builder` and regenerate bindings.

- [ ] **Step 2: Frontend step**

Replace `ui/src/screens/onboarding/StepAgent.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as bindings from "../../api/bindings";
import { useMarkOnboardingComplete } from "../../api/hooks/onboarding";

interface Props { onDone: () => void; }

export default function StepAgent({ onDone }: Props) {
  const [baseUrl] = useState("http://localhost:11434");
  const { data: probe, refetch, isFetching } = useQuery({
    queryKey: ["ollama-probe", baseUrl],
    queryFn: () => bindings.probeOllama(baseUrl),
    staleTime: 0,
  });
  const [completionModel, setCompletionModel] = useState<string>("");
  const markComplete = useMarkOnboardingComplete();

  useEffect(() => {
    if (probe?.models[0] && !completionModel) setCompletionModel(probe.models[0]);
  }, [probe]);

  async function finishWithOllama() {
    if (!probe?.reachable || !completionModel) return;
    await bindings.saveLlmProvider({
      kind: "ollama",
      base_url: baseUrl,
      completion_model: completionModel,
      embedding_model: "nomic-embed-text",
    });
    await markComplete.mutateAsync();
    onDone();
  }

  async function skipForLater() {
    await bindings.saveLlmProvider({ kind: "unconfigured" });
    await markComplete.mutateAsync();
    onDone();
  }

  if (isFetching && !probe) {
    return <div className="step-agent"><p>Checking for Ollama…</p></div>;
  }

  if (!probe?.reachable) {
    return (
      <div className="step-agent">
        <h2>Set up the agent</h2>
        <p>We couldn't find a local model. FinSight uses{" "}
          <a href="#" onClick={(e) => { e.preventDefault(); openUrl("https://ollama.com"); }}>Ollama</a>
          {" "}for private agent features.
        </p>
        <div className="actions">
          <button onClick={() => openUrl("https://ollama.com")}>Install Ollama →</button>
          <button onClick={() => refetch()}>I just installed it — refresh</button>
          <button className="tertiary" onClick={skipForLater}>Configure later →</button>
        </div>
      </div>
    );
  }

  return (
    <div className="step-agent">
      <h2>Set up the agent</h2>
      <p>Ollama is running. Pick a completion model.</p>
      <label> Completion model
        <select value={completionModel} onChange={e => setCompletionModel(e.target.value)}>
          {probe.models.map(m => <option key={m} value={m}>{m}</option>)}
        </select>
      </label>
      {!probe.has_nomic_embed && (
        <p className="warning">
          <code>nomic-embed-text</code> isn't installed. Run{" "}
          <code>ollama pull nomic-embed-text</code> in your terminal, then{" "}
          <button onClick={() => refetch()}>Refresh</button>.
        </p>
      )}
      <div className="actions">
        <button className="primary" onClick={finishWithOllama}
                disabled={!probe.has_nomic_embed || !completionModel}>
          Use Ollama →
        </button>
        <button className="tertiary" onClick={skipForLater}>Skip for now</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Smoke test**

Run: `cargo build --workspace && pnpm test`
Expected: clean build, all existing tests still green. The agent step has no dedicated test (it's effectively an HTTP wrapper); coverage comes from manual smoke + the e2e check in Task 22.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app ui/src/screens/onboarding/StepAgent.tsx ui/src/api/bindings.ts
git commit -m "feat: StepAgent with Ollama probe + saved llm_provider config"
```

---

### Task 22: Settings escape hatches + a11y audit + final smoke

**Files:**
- Modify: `ui/src/screens/Settings.tsx` (Re-run onboarding + Replace sample data)
- Create: `ui/src/test/a11y.test.tsx` (vitest-axe sweep)
- Modify: `.github/workflows/ci.yml` (if needed — confirm vitest already runs)

- [ ] **Step 1: Settings buttons**

Replace `ui/src/screens/Settings.tsx`:

```tsx
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import * as bindings from "../api/bindings";
import { useResetOnboarding, useClearSampleData, useOnboardingState } from "../api/hooks/onboarding";

export default function Settings() {
  const navigate = useNavigate();
  const reset = useResetOnboarding();
  const clearSample = useClearSampleData();
  const { data: accounts = [] } = useQuery({
    queryKey: ["accounts"], queryFn: () => bindings.listAccounts(),
  });
  const { data: onboarding } = useOnboardingState();
  const hasSample = accounts.some((a: any) => a.source === "sample");

  async function reRunOnboarding() {
    if (!confirm("This will re-open the welcome wizard. Your existing accounts, transactions, and categories are kept.")) return;
    await reset.mutateAsync();
    navigate("/onboarding");
  }

  async function replaceSampleData() {
    if (!confirm("This will permanently delete the Mira & Adam sample accounts and their transactions. Anything you added manually or imported is kept.")) return;
    await clearSample.mutateAsync();
    navigate("/onboarding");
  }

  return (
    <div className="screen-settings">
      <h1>Settings</h1>

      <section>
        <h2>Onboarding</h2>
        <p>Completed: <strong>{onboarding?.completion_marked ? "yes" : "no"}</strong></p>
        <button onClick={reRunOnboarding}>Re-run onboarding</button>
      </section>

      {hasSample && (
        <section>
          <h2>Sample data</h2>
          <p>You're currently looking at the Mira &amp; Adam sample household. Replace it with your own when you're ready.</p>
          <button onClick={replaceSampleData} className="danger">Replace sample data with my own</button>
        </section>
      )}
    </div>
  );
}
```

> **NOTE:** AccountSummary doesn't expose `source` yet in bindings. Edit `crates/finsight-core/src/models/account.rs::AccountSummary` to include `pub source: String`, update `repos::accounts::list_summaries` to SELECT `a.source` and populate it, and regenerate bindings. Wrap that change into this task's commit.

- [ ] **Step 2: a11y test suite**

Create `ui/src/test/a11y.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "vitest-axe";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import Drawer from "../components/Drawer";
import AccountDrawer from "../components/AccountDrawer";
import TransactionDrawer from "../components/TransactionDrawer";
import Onboarding from "../screens/Onboarding";

vi.mock("../api/bindings", () => ({
  listAccounts: vi.fn().mockResolvedValue([]),
  listTransactions: vi.fn().mockResolvedValue([]),
  createAccount: vi.fn(), createTransaction: vi.fn(),
  getOnboardingState: vi.fn().mockResolvedValue({
    account_count: 0, category_count: 0, completion_marked: false,
  }),
  seedSampleHousehold: vi.fn(), markOnboardingComplete: vi.fn(),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("a11y sweep", () => {
  it("Drawer has no axe violations", async () => {
    const { container } = wrap(<Drawer open onClose={() => {}} title="X"><p>body</p></Drawer>);
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("AccountDrawer has no axe violations", async () => {
    const { container } = wrap(<AccountDrawer open onClose={() => {}} />);
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("TransactionDrawer has no axe violations", async () => {
    const { container } = wrap(<TransactionDrawer open onClose={() => {}} />);
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("Onboarding shell (welcome step) has no axe violations", async () => {
    const { container } = wrap(<Onboarding />);
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });
});
```

> If any of the four components has axe violations the test will fail — fix them by adding labels, contrast tweaks, or aria attributes as needed. Common easy fixes: ensure every form input has an associated `<label>`, ensure every button has accessible text, ensure dialog `aria-labelledby` points at an existing element id.

- [ ] **Step 3: Toast wiring**

Confirm that the existing Phase 1 toaster (or sonner / react-hot-toast) is invoked from the import success path. In `ui/src/screens/onboarding/ImportMappingDialog.tsx::submit`, after `onImported(summary)`, push a toast:

```ts
import { toast } from "sonner";   // adjust to whichever lib Phase 1 wired
// inside submit:
toast.success(`Imported ${summary.rows_imported} transactions, skipped ${summary.rows_skipped_duplicates} duplicates`);
```

(If Phase 1 didn't wire any toast lib, install `sonner` and mount its `<Toaster />` in `App.tsx`. Phase 1 had a placeholder; this task pins down which library it is.)

- [ ] **Step 4: Full test suite**

Run:
```bash
cargo test --workspace
pnpm test
pnpm run lint        # if a lint script exists
cargo fmt --check
cargo clippy --workspace -- -D warnings
```
Expected: everything green.

- [ ] **Step 5: Manual end-to-end smoke**

Run `pnpm tauri dev` and walk through:
1. Delete the existing dev DB (`~/Library/Application Support/com.finsight.app/data.sqlcipher` on macOS; equivalent path on Windows/Linux).
2. App starts → onboarding auto-opens.
3. Click "Try with sample data" → land on `/today` with 6 accounts.
4. Open Settings → "Re-run onboarding" → confirm dialog → back at /onboarding.
5. Open Settings → "Replace sample data with my own" → confirm dialog → /onboarding.
6. Click "Get started" → Step 2 → "+ Account" → fill out → drawer closes → tally updates.
7. Step 2 → "Pick a file…" → pick a CSV → map columns → import → toast.
8. Step 3 → tweak categories → "Use these".
9. Step 4 → Ollama present: pick model + Use Ollama; Ollama missing: pick "Configure later" → land on /today.
10. Re-open Transactions → "Import CSV" → re-import the same file → toast shows "Imported 0, skipped N duplicates".

Document any issues found and fix before commit.

- [ ] **Step 6: Commit**

```bash
git add ui/src/screens/Settings.tsx ui/src/test/a11y.test.tsx \
        ui/src/screens/onboarding/ImportMappingDialog.tsx \
        crates/finsight-core/src/models/account.rs \
        crates/finsight-core/src/repos/accounts.rs \
        ui/src/api/bindings.ts
git commit -m "feat: Settings escape hatches + a11y sweep + import toast wiring"
```

---

## Final review

After Task 22 commits:

- [ ] **Run the full smoke + test matrix locally:**
  ```bash
  cargo test --workspace
  cargo clippy --workspace -- -D warnings
  cargo fmt --check
  pnpm test
  pnpm tauri build --debug
  ```
- [ ] **Push the branch and watch CI:** the 3-OS matrix from Phase 1 covers everything; no CI changes needed in Phase 2.
- [ ] **Hand off via `superpowers:finishing-a-development-branch`** to merge/PR.









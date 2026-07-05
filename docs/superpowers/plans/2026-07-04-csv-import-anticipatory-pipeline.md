# CSV Import Anticipatory Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make CSV import feel instant by computing parse + validate + reconcile as early as it's safe (before the user clicks Import), while keeping commit authoritative and dedup behavior byte-for-byte identical.

**Architecture:** Extract the parse+reconcile row-fold into a **read-only** function that produces an ordered `PreparedImport` (decisions + counts + a staleness signature). A new `prepare_csv_import` command surfaces a lightweight preview to the dialog. `CsvProvider::import` is refactored to reuse the same fold (so parity is guaranteed by shared code), eliminating the double file-read and the separate count-pass. A phase-attributed criterion bench captures baselines first and decides whether the post-commit cascade (D4) needs scoping.

**Tech Stack:** Rust (rusqlite, csv, criterion), Tauri + specta bindings, React + TanStack Query, vitest.

---

## Correctness invariants (must hold in every task)

- **The reconcile fold is sequential and order-dependent.** `matched_existing_ids` and `self_import_ids` accumulate across rows. Never parallelize it. Pre-generate the would-be-inserted UUID for each `Insert` decision so `self_import_ids` accumulates identically to the write path.
- **Read-only fold == current interleaved fold.** The current import reconciles against the open write txn but excludes `self_import_ids`; the only writes that affect later fuzzy matches are `Insert` rows, which are excluded. A read-only fold over the pre-import connection therefore produces identical decisions. This equivalence is the parity guarantee — the fold code is shared, not duplicated.
- **Prepare never mutates.** No `imports` row, no mapping save, no inserts during prepare.
- **Commit is authoritative.** Commit re-runs the fold against a fresh connection inside the write path; it does not trust a client-supplied plan. The `signature` is informational (staleness/invalidation), not a correctness gate in v1.

## File structure

| File | Responsibility | Change |
|---|---|---|
| `crates/finsight-providers/src/csv/prepare.rs` | The shared read-only fold → `PreparedImport`, `PreparedDecision`, signature. | Create |
| `crates/finsight-providers/src/csv/mod.rs` | Re-export prepare types; `import` refactored to reuse the fold + apply decisions. | Modify |
| `crates/finsight-providers/src/simplefin/matcher.rs:277` | `conn.prepare` → `conn.prepare_cached` in `find_fuzzy_candidates`. | Modify |
| `crates/finsight-providers/benches/import_phases.rs` | Phase-attributed criterion bench over `samples/`. | Create |
| `crates/finsight-providers/Cargo.toml` | Add `criterion` dev-dep + `[[bench]]`. | Modify |
| `crates/finsight-app/src/commands/import.rs` | `prepare_csv_import` command returning a lightweight preview. | Modify |
| `crates/finsight-app/src/lib.rs` | Register the new command. | Modify |
| `ui/src/api/bindings.ts` | Regenerated. | Generated |
| `ui/src/api/hooks/csv.ts` | `usePrepareImport` hook. | Modify |
| `ui/src/components/ImportMappingDialog.tsx` | Show the prepared outcome; invalidate on close. | Modify |
| `docs/superpowers/specs/2026-07-04-...-design.md` | Append baseline + after tables. | Modify |

---

### Task 1: Phase-attributed benchmark harness + baselines (D1 — decides D4)

**Files:**
- Modify: `crates/finsight-providers/Cargo.toml`
- Create: `crates/finsight-providers/benches/import_phases.rs`
- Modify: `docs/superpowers/specs/2026-07-04-csv-import-anticipatory-pipeline-design.md` (append numbers)

- [ ] **Step 1: Add criterion dev-dep + bench target**

In `crates/finsight-providers/Cargo.toml`, under `[dev-dependencies]` add:
```toml
criterion = { version = "0.5", features = ["html_reports"] }
```
At end of file add:
```toml
[[bench]]
name = "import_phases"
harness = false
```

- [ ] **Step 2: Write the bench**

Create `crates/finsight-providers/benches/import_phases.rs`:
```rust
//! Phase-attributed import baselines over real sample CSVs.
//! Mutating phases use iter_batched with a fresh seeded DB per iteration so
//! iteration 2+ does not silently measure the all-duplicates path.
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use finsight_core::Db;
use finsight_providers::csv::{CsvImportMapping, CsvProvider};
use finsight_providers::csv::mapping::{AmountConvention, ColumnRole};
use std::path::PathBuf;

fn repo_sample(name: &str) -> PathBuf {
    // benches run with CWD = crate dir; samples/ is at workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples").join(name)
}

fn fresh_db() -> (Db, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("b.sqlcipher"), &"cd".repeat(32)).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    (db, dir)
}

// Amex all-time is the largest sample (~2k rows). Adjust columns to the real header.
fn amex_mapping() -> CsvImportMapping {
    CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
        date_format: "%m/%d/%Y".into(),
        amount_convention: AmountConvention::PositiveIsOutflow,
        decimal_separator: '.',
        delimiter: None,
    }
}

fn bench_import_end_to_end(c: &mut Criterion) {
    let path = repo_sample("amex-all-time-statement.csv");
    let mapping = amex_mapping();
    c.bench_function("import_amex_full", |b| {
        b.iter_batched(
            || { let (db, dir) = fresh_db();
                 // create the account the import targets
                 let acct_id = seed_one_account(&db);
                 (db, dir, acct_id) },
            |(db, _dir, acct_id)| {
                let id = uuid::Uuid::new_v4().to_string();
                CsvProvider::import(&path, &acct_id, &id, &mapping, &db, |_| {}).unwrap();
            },
            BatchSize::LargeInput,
        )
    });
}

fn seed_one_account(db: &Db) -> String {
    use finsight_core::models::{AccountType, NewAccount};
    use finsight_core::repos::accounts;
    let mut conn = db.get().unwrap();
    accounts::insert(&mut conn, NewAccount {
        owner: "joint".into(), bank: "Amex".into(), r#type: AccountType::Credit,
        name: "Card".into(), last4: None, currency: "USD".into(), color: "#fff".into(),
        opening_balance_cents: 0, source: "manual".into(), liquidity_type: "liquid".into(),
        emergency_fund_eligible: false, goal_earmark: None, apy_pct: None,
        simplefin_account_id: None, nickname: None, connection_id: None,
        institution_id: None, external_account_id: None,
    }).unwrap()
}

criterion_group!(benches, bench_import_end_to_end);
criterion_main!(benches);
```
> Note: verify the exact `NewAccount` fields against `crates/finsight-core/src/models/account.rs` at execution time (the V039 WIP may have changed them) and fix the amex column mapping against the real header row. Add per-phase `bench_function`s (read+decode, parse-only via a public parse helper, reconcile-only against a pre-seeded DB, and one bench each for `apply_builtin_categorization`, `pair_transfers`, `recompute_anomalies`, `net_worth::backfill_history_from_transactions` over the seeded-then-imported DB) mirroring the end-to-end shape.

- [ ] **Step 3: Run the bench, capture numbers**

Run: `cargo bench -p finsight-providers --bench import_phases`
Expected: completes; prints per-phase medians. Record the median for each phase.

- [ ] **Step 4: Record baselines in the spec**

Append a "## Baselines (before)" table to the design doc with one row per phase (ms median, sample = amex ~2k rows), and a one-line verdict: **does the post-commit cascade dominate parse+reconcile?** This verdict decides whether Task 7 (D4) is required or deferred.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-providers/Cargo.toml crates/finsight-providers/benches/import_phases.rs docs/superpowers/specs/2026-07-04-csv-import-anticipatory-pipeline-design.md
git commit -m "bench(import): phase-attributed baselines over sample CSVs"
```

---

### Task 2: Cheap win — cache the fuzzy-candidate statement (D3)

**Files:**
- Modify: `crates/finsight-providers/src/simplefin/matcher.rs:277`
- Test: existing `cargo test -p finsight-providers` covers behavior parity.

- [ ] **Step 1: Switch to prepare_cached**

In `find_fuzzy_candidates`, change `conn.prepare(` to `conn.prepare_cached(`. The SQL and params are unchanged. `prepare_cached` returns a `CachedStatement`; the rest of the function body compiles unchanged.

- [ ] **Step 2: Run provider tests**

Run: `cargo test -p finsight-providers`
Expected: PASS (behavior identical; only statement caching changed).

- [ ] **Step 3: Re-run the reconcile-only bench**

Run: `cargo bench -p finsight-providers --bench import_phases -- reconcile`
Expected: reconcile-phase median improves or is unchanged. Note the delta.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-providers/src/simplefin/matcher.rs
git commit -m "perf(import): cache the per-row fuzzy-candidate statement (prepare_cached)"
```

---

### Task 3: Extract the shared read-only reconcile fold (D2 backend core)

**Files:**
- Create: `crates/finsight-providers/src/csv/prepare.rs`
- Modify: `crates/finsight-providers/src/csv/mod.rs`
- Test: `crates/finsight-providers/tests/prepare_parity.rs` (create)

- [ ] **Step 1: Write the failing parity test**

Create `crates/finsight-providers/tests/prepare_parity.rs`:
```rust
//! prepare() decisions must match import() outcomes exactly, on real samples.
use finsight_core::Db;
use finsight_providers::csv::{CsvProvider};
mod common; // small helper module: open_db(), seed_account() — copy the pattern
            // from crates/finsight-core/tests/repos_transactions.rs::open()

fn setup() -> (Db, tempfile::TempDir, String) { common::open_with_account() }

#[test]
fn prepare_counts_match_import_summary_on_amex() {
    let path = common::sample("amex-all-time-statement.csv");
    let mapping = common::amex_mapping();

    // Prepare against a fresh DB (read-only).
    let (db, _d, acct) = setup();
    let prepared = {
        let conn = db.get().unwrap();
        CsvProvider::prepare(&path, &acct, &mapping, &conn).unwrap()
    };

    // Import against an identically fresh DB.
    let (db2, _d2, acct2) = setup();
    let id = uuid::Uuid::new_v4().to_string();
    let summary = CsvProvider::import(&path, &acct2, &id, &mapping, &db2, |_| {}).unwrap();

    assert_eq!(prepared.rows_imported, summary.rows_imported);
    assert_eq!(prepared.rows_skipped_duplicates, summary.rows_skipped_duplicates);
    assert_eq!(prepared.rows_queued_for_review, summary.rows_queued_for_review);
    assert_eq!(prepared.errors.len(), summary.errors.len());
}
```

- [ ] **Step 2: Run it, verify it fails to compile**

Run: `cargo test -p finsight-providers --test prepare_parity`
Expected: FAIL — `CsvProvider::prepare` and `common` do not exist yet.

- [ ] **Step 3: Write `prepare.rs`**

Create `crates/finsight-providers/src/csv/prepare.rs`:
```rust
//! Read-only anticipatory fold: parse + reconcile a CSV into an ordered plan
//! WITHOUT any DB mutation. Shared with the write path so decisions are
//! identical by construction.
use crate::csv::encoding::decode_layered;
use crate::csv::mapping::CsvImportMapping;
use crate::csv::parse::{into_new_transaction, parse_row};
use crate::csv::{detect_delimiter, read_capped, RowError};
use crate::error::{ProviderError, ProviderResult};
use crate::simplefin::matcher::{reconcile_excluding_batch, PotentialMatch, ReconciliationDecision};
use finsight_core::models::NewTransaction;
use rusqlite::Connection;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum PreparedDecision {
    Insert { new_id: String, tx: NewTransaction },
    Duplicate { existing_id: String },
    Review { candidate: NewTransaction, matches: Vec<PotentialMatch>, confidence: i64, reason: String },
}

#[derive(Debug, Clone)]
pub struct PreparedRow {
    pub row_number: u32,
    pub decision: PreparedDecision,
}

#[derive(Debug, Clone)]
pub struct PreparedImport {
    pub signature: String,
    pub delimiter: char,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub rows_queued_for_review: u32,
    pub errors: Vec<RowError>,
    pub rows: Vec<PreparedRow>,
}

/// Per-account ledger fingerprint: cheap staleness signal for the prepared plan.
pub fn ledger_fingerprint(conn: &Connection, account_id: &str) -> ProviderResult<String> {
    let (count, max_created): (i64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*), MAX(created_at) FROM transactions WHERE account_id = ?1",
            [account_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| ProviderError::Internal(format!("fingerprint: {e}")))?;
    Ok(format!("{count}:{}", max_created.unwrap_or_default()))
}

impl crate::csv::CsvProvider {
    /// Read-only: parse + reconcile into an ordered plan. No writes.
    pub fn prepare(
        path: &std::path::Path,
        account_id: &str,
        mapping: &CsvImportMapping,
        conn: &Connection,
    ) -> ProviderResult<PreparedImport> {
        let bytes = read_capped(path)?;
        if bytes.is_empty() {
            return Err(ProviderError::EmptyFile);
        }
        let (text, _) = decode_layered(&bytes)?;
        let delimiter = mapping.delimiter.unwrap_or_else(|| detect_delimiter(&text));

        let meta = std::fs::metadata(path)?;
        let signature = format!(
            "{}|{}|{}|{}",
            account_id,
            mapping_signature(mapping),
            meta.len(),
            ledger_fingerprint(conn, account_id)?,
        );

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter as u8)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut out = PreparedImport {
            signature, delimiter,
            rows_imported: 0, rows_skipped_duplicates: 0, rows_queued_for_review: 0,
            errors: Vec::new(), rows: Vec::new(),
        };
        let mut matched_existing_ids: HashSet<String> = HashSet::new();
        let mut self_import_ids: HashSet<String> = HashSet::new();

        for (idx, rec) in reader.records().enumerate() {
            let row_number = (idx + 1) as u32;
            let rec = match rec {
                Ok(r) => r,
                Err(e) => { out.errors.push(RowError { row_number, reason: e.to_string() }); continue; }
            };
            if idx < mapping.skip_header_rows as usize { continue; }
            let fields: Vec<&str> = rec.iter().collect();
            let parsed = match parse_row(&fields, mapping) {
                Ok(p) => p,
                Err(e) => { out.errors.push(RowError { row_number, reason: e.to_string() }); continue; }
            };
            let new_tx = into_new_transaction(parsed, account_id.to_string());
            match reconcile_excluding_batch(
                conn, account_id, &new_tx, None, 7, &matched_existing_ids, &self_import_ids,
            )? {
                ReconciliationDecision::AutoMatch(existing) => {
                    matched_existing_ids.insert(existing.id.clone());
                    out.rows_skipped_duplicates += 1;
                    out.rows.push(PreparedRow { row_number, decision: PreparedDecision::Duplicate { existing_id: existing.id } });
                }
                ReconciliationDecision::NeedsReview { matches, confidence, reason } => {
                    out.rows_queued_for_review += 1;
                    out.rows.push(PreparedRow { row_number, decision: PreparedDecision::Review { candidate: new_tx, matches, confidence, reason } });
                }
                ReconciliationDecision::None => {
                    let new_id = Uuid::new_v4().to_string();
                    self_import_ids.insert(new_id.clone());
                    out.rows_imported += 1;
                    out.rows.push(PreparedRow { row_number, decision: PreparedDecision::Insert { new_id, tx: new_tx } });
                }
            }
        }
        Ok(out)
    }
}

fn mapping_signature(m: &CsvImportMapping) -> String {
    format!("{:?}|{}|{:?}|{}|{}|{:?}",
        m.columns, m.date_format, m.amount_convention, m.decimal_separator,
        m.skip_header_rows, m.delimiter)
}
```

- [ ] **Step 4: Make `detect_delimiter`, `read_capped`, `RowError` reachable from `prepare.rs`**

In `crates/finsight-providers/src/csv/mod.rs`: add `pub mod prepare;`, change `fn detect_delimiter` → `pub(crate) fn detect_delimiter`, `fn read_capped` → `pub(crate) fn read_capped`. Add `pub use prepare::{PreparedImport, PreparedDecision, PreparedRow};`. Ensure `PotentialMatch` is `pub` in matcher (it already is).

- [ ] **Step 5: Add the `common` test helper**

Create `crates/finsight-providers/tests/common/mod.rs` with `open_with_account()`, `sample(name)`, and `amex_mapping()` (copy the `open()` pattern from `crates/finsight-core/tests/repos_transactions.rs` and the mapping from the bench). Keep it tiny and DRY.

- [ ] **Step 6: Run the parity test**

Run: `cargo test -p finsight-providers --test prepare_parity`
Expected: PASS — prepare counts equal import summary on the amex sample.

- [ ] **Step 7: Run the whole provider suite (no regressions)**

Run: `cargo test -p finsight-providers`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/finsight-providers/src/csv/prepare.rs crates/finsight-providers/src/csv/mod.rs crates/finsight-providers/tests/prepare_parity.rs crates/finsight-providers/tests/common/mod.rs
git commit -m "feat(import): read-only prepare fold shared with the write path"
```

---

### Task 4: Refactor `import` to reuse the fold (eliminate double read + count pass)

**Files:**
- Modify: `crates/finsight-providers/src/csv/mod.rs` (`CsvProvider::import`)
- Test: existing `cargo test -p finsight-providers` (behavior parity) + parity test from Task 3.

- [ ] **Step 1: Rewrite `import` to fold-then-apply**

Replace the body of `CsvProvider::import` so it: (a) opens a connection, inserts the `imports` row (unchanged); (b) calls `Self::prepare(path, account_id, mapping, &conn)` to get the ordered `PreparedImport` (this reads pre-import state — correct, self-inserts are excluded); (c) sets `total = prepared.rows.len() + prepared.errors.len()` for progress; (d) opens a write txn and walks `prepared.rows` applying each decision — `Insert` → the existing INSERT with the **pre-generated `new_id`**; `Review` → `import_candidates::create(...)`; `Duplicate` → no write; committing every `BATCH_SIZE` and emitting progress exactly as today; (e) saves mapping, finalizes the `imports` row, recomputes balance (unchanged). Carry `prepared.errors` into the returned `ImportSummary`. The file is now read once (inside prepare); the standalone count pass is deleted.

> The apply loop must preserve the current commit cadence (`in_batch >= BATCH_SIZE || should_emit`) so progress events are unchanged.

- [ ] **Step 2: Run parity + full provider suite**

Run: `cargo test -p finsight-providers`
Expected: PASS — including `prepare_parity` and all existing import tests, unchanged.

- [ ] **Step 3: Re-run end-to-end bench**

Run: `cargo bench -p finsight-providers --bench import_phases -- import_amex_full`
Expected: end-to-end median improves (no double read, no count pass). Record delta.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-providers/src/csv/mod.rs
git commit -m "perf(import): commit reuses the prepare fold; drop double read + count pass"
```

---

### Task 5: `prepare_csv_import` command returning a lightweight preview (D2 surface)

**Files:**
- Modify: `crates/finsight-app/src/commands/import.rs`
- Modify: `crates/finsight-app/src/lib.rs`
- Test: `crates/finsight-app/tests/prepare_csv_cmd.rs` (create)

- [ ] **Step 1: Write the failing command test**

Create `crates/finsight-app/tests/prepare_csv_cmd.rs` that opens a test `Db`, seeds an account, and calls the underlying preview builder (factor the body into a `pub(crate) fn build_preview(db, path, account_id, mapping) -> AppResult<PreparedImportPreview>` so it's testable without a Tauri `AppHandle`). Assert the preview counts match a direct `CsvProvider::prepare` on the amex sample.

- [ ] **Step 2: Run it, verify failure**

Run: `cargo test -p finsight-app --test prepare_csv_cmd`
Expected: FAIL — `build_preview` / `PreparedImportPreview` undefined.

- [ ] **Step 3: Implement the preview type + command**

In `import.rs` add:
```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreparedImportPreview {
    pub signature: String,
    pub rows_total: u32,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub rows_queued_for_review: u32,
    pub errors: Vec<finsight_providers::csv::RowError>, // capped below
}

pub(crate) fn build_preview(
    db: &finsight_core::Db,
    path: &std::path::Path,
    account_id: &str,
    mapping: &CsvImportMapping,
) -> AppResult<PreparedImportPreview> {
    let conn = db.get().map_err(AppError::from)?;
    let p = finsight_providers::csv::CsvProvider::prepare(path, account_id, mapping, &conn)
        .map_err(AppError::from)?;
    let mut errors = p.errors;
    errors.truncate(50); // never ship an unbounded per-row payload over IPC
    Ok(PreparedImportPreview {
        signature: p.signature,
        rows_total: (p.rows.len() as u32) + errors.len() as u32,
        rows_imported: p.rows_imported,
        rows_skipped_duplicates: p.rows_skipped_duplicates,
        rows_queued_for_review: p.rows_queued_for_review,
        errors,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_csv_import(
    state: tauri::State<'_, AppState>,
    path: String,
    account_id: String,
    mapping: CsvImportMapping,
) -> AppResult<PreparedImportPreview> {
    let db = (*state.db).clone();
    let path = PathBuf::from(path);
    tokio::task::spawn_blocking(move || build_preview(&db, &path, &account_id, &mapping))
        .await
        .map_err(|e| AppError::new("internal", format!("join: {e}")))?
}
```
Ensure `RowError` is `Serialize + Type` (it already derives both). Add `use finsight_providers::CsvImportMapping;` if not already imported.

- [ ] **Step 4: Register the command**

In `crates/finsight-app/src/lib.rs` `build_specta_builder()`, add `commands::import::prepare_csv_import` to `collect_commands![...]`.

- [ ] **Step 5: Run the command test**

Run: `cargo test -p finsight-app --test prepare_csv_cmd`
Expected: PASS.

- [ ] **Step 6: Regenerate bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts` gains `prepareCsvImport` + `PreparedImportPreview`.

- [ ] **Step 7: Commit**

```bash
git add crates/finsight-app/src/commands/import.rs crates/finsight-app/src/lib.rs crates/finsight-app/tests/prepare_csv_cmd.rs ui/src/api/bindings.ts
git commit -m "feat(import): prepare_csv_import command returns a bounded outcome preview"
```

---

### Task 6: Frontend — surface the prepared outcome + invalidate (D5)

**Files:**
- Modify: `ui/src/api/hooks/csv.ts`
- Modify: `ui/src/components/ImportMappingDialog.tsx`
- Test: `ui/src/components/ImportMappingDialog.test.tsx` (extend)

- [ ] **Step 1: Add `usePrepareImport` hook**

In `ui/src/api/hooks/csv.ts`:
```ts
import type { PreparedImportPreview } from "../client";

/** Speculative import outcome for (account, mapping) — recomputed as the user
 *  edits mapping; keyed so edits supersede in-flight prepares automatically. */
export function usePrepareImport(
  path: string | null,
  accountId: string | null,
  mapping: CsvImportMapping | null,
) {
  return useQuery<PreparedImportPreview>({
    queryKey: ["csv-prepare", path, accountId, mapping],
    queryFn: async () => {
      const r = await commands.prepareCsvImport(path!, accountId!, mapping!);
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    enabled: !!path && !!accountId && !!mapping,
    staleTime: 10_000,
  });
}
```

- [ ] **Step 2: Wire it into the dialog footer**

In `ImportMappingDialog.tsx`, when `canSubmit` is true build the same `mapping` object and pass it to `usePrepareImport(path, accountId, canSubmit ? mapping : null)`. Replace the static "Ready to import" text with the live outcome when available: `` `${prep.rowsImported} new · ${prep.rowsSkippedDuplicates} duplicates · ${prep.rowsQueuedForReview} to review${prep.errors.length ? ` · ${prep.errors.length} errors` : ""}` ``. Show "Checking…" while `isFetching`. The mapping object is already memoization-friendly; wrap it in `useMemo` keyed on the mapping inputs to keep the query key stable.

- [ ] **Step 3: Invalidate the prepare on close and after data changes**

On dialog `onClose` and in the existing `import`/delete-all success handlers, call `queryClient.invalidateQueries({ queryKey: ["csv-prepare"] })` (find the delete-all mutation in `ui/src/api/hooks/` and add the invalidation there). This satisfies the "Delete All / sync invalidates prepared plan" criterion since the plan lives only in React Query.

- [ ] **Step 4: Extend the dialog test**

In `ImportMappingDialog.test.tsx`, mock `commands.prepareCsvImport` to resolve a preview and assert the footer renders "N new · D duplicates · R to review". Keep the existing tests green.

- [ ] **Step 5: Run frontend tests + typecheck**

Run: `cd ui && npx vitest run src/components/ImportMappingDialog.test.tsx && npx tsc --noEmit`
Expected: PASS, 0 TS errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/api/hooks/csv.ts ui/src/components/ImportMappingDialog.tsx ui/src/components/ImportMappingDialog.test.tsx
git commit -m "feat(import): show the prepared outcome live before Import; invalidate on close/delete"
```

---

### Task 7: Post-commit cascade scoping — GATED on Task 1 verdict (D4)

> Only do this task if Task 1's baseline shows the post-commit cascade dominates parse+reconcile. If it does not, **skip and note "deferred — cascade not a bottleneck at sample scale" in the spec**, and proceed to Task 8.

**Files:**
- Modify: `crates/finsight-core/src/anomaly.rs` (`recompute_anomalies` → account/date-scoped variant)
- Modify: `crates/finsight-core/src/repos/net_worth.rs` (`backfill_history_from_transactions` → incremental-from-date variant)
- Modify: `crates/finsight-app/src/commands/import.rs` (call the scoped variants; run the two genuinely-independent steps concurrently)
- Test: `crates/finsight-core/tests/` unit tests for the scoped variants proving equivalence with the full-history result on a seeded DB.

- [ ] **Step 1: Write failing equivalence tests** for the scoped variants: seed a DB, run full recompute, snapshot flags/history; run the scoped variant over the same import window; assert identical rows in the affected range and untouched rows elsewhere.
- [ ] **Step 2: Run — verify fail** (`cargo test -p finsight-core`).
- [ ] **Step 3: Implement scoped variants** keeping the full-history fns as callers/fallbacks.
- [ ] **Step 4: Run tests — pass.**
- [ ] **Step 5: Update `import_csv`** to call scoped variants and `tokio::try_join!` the independent ones (preserve order: `pair_transfers` after `apply_builtin_categorization`).
- [ ] **Step 6: Re-run cascade benches**, record after-numbers.
- [ ] **Step 7: Commit** `perf(import): scope post-commit recomputes to the imported window`.

---

### Task 8: Staleness, invalidation, and edge tests + final verification

**Files:**
- Test: `crates/finsight-providers/tests/prepare_parity.rs` (extend)
- Modify: `docs/superpowers/specs/2026-07-04-...-design.md` (after-numbers)
- Modify: `CLAUDE.md` (green-bar counts)

- [ ] **Step 1: Staleness test** — build a prepare against a seeded DB, insert a matching transaction, assert `ledger_fingerprint` changes (so the signature differs), and assert a fresh `import` still produces correct authoritative counts (proves commit does not trust a stale plan).

- [ ] **Step 2: Edge tests** — empty file → `EmptyFile` error surfaced (not a panic); all-duplicates re-import → `rows_imported == 0`, all skipped; a file with malformed rows → errors captured, good rows still imported; prepare then re-prepare with a flipped `amount_convention` → different signature and different counts.

- [ ] **Step 3: Run the full workspace + frontend suites**

Run: `cargo test --workspace` then `cd ui && npx vitest run && npx tsc --noEmit`
Expected: all green. Record the new totals.

- [ ] **Step 4: Update baselines-after + CLAUDE.md counts**

Append "## Results (after)" to the design doc with before/after per-phase and end-to-end medians. Update the green-bar line in `CLAUDE.md` to the new Rust/FE test counts.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-providers/tests/prepare_parity.rs docs/superpowers/specs/2026-07-04-csv-import-anticipatory-pipeline-design.md CLAUDE.md
git commit -m "test(import): staleness + edge coverage; document before/after"
```

---

## Self-review notes

- **Spec coverage:** D1→Task1, D2→Tasks 3–5, D3→Tasks 2 & 4, D4→Task 7 (gated), D5→Task 6, testing→Tasks 3/5/6/8. All spec deliverables mapped.
- **Parity guarantee:** enforced by *sharing* the fold (Task 3) rather than reimplementing it in `import` (Task 4) — the strongest possible form.
- **IPC bound:** preview truncates errors and never ships per-row decisions (Task 5).
- **Invalidation:** plan lives only in React Query, so Delete-All/close invalidation is a single `invalidateQueries` (Task 6, Step 3).
- **Type consistency:** `PreparedImport`/`PreparedDecision`/`PreparedImportPreview`/`build_preview`/`usePrepareImport`/`ledger_fingerprint`/`mapping_signature` used consistently across tasks.
- **WIP guard:** Task 1 flags re-verifying `NewAccount` fields against the V039 WIP before compiling the bench.

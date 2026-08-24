# Task 1 Report: Derive ToSchema on all finsight-core DTOs

**Status:** DONE
**Branch:** feat/openapi-deep-schema
**Worktree:** E:/Workspace/FinSight/.worktrees/openapi-deep-schema
**Commit:** 696bdb2 feat(core): derive ToSchema on all DTOs

---

## Objective
Add `utoipa::ToSchema` to every `finsight-core` DTO so that `finsight-openapi` can later collect typed `components/schemas` instead of shallow `{type: object}` placeholders. TDD with failing shallow guard test, verify via `cargo check`/`cargo test`, commit.

## Steps Executed

### Step 1: Failing shallow guard test
- **File:** `crates/finsight-openapi/src/lib.rs:382-402`
- Added test `openapi_schemas_not_shallow` exactly as spec:
  ```rust
  #[test]
  fn openapi_schemas_not_shallow() {
      let spec = build_openapi();
      let json = serde_json::to_value(&spec).unwrap();
      let schemas = json["components"]["schemas"].as_object().expect("schemas");
      assert!(schemas.len() > 20, ...);
      for (name, schema) in schemas { assert!(!s.contains(r#""type":"object""#) || s.contains("properties"), ...) }
  }
  ```
- **Verification:** `cargo test -p finsight-openapi openapi_schemas_not_shallow` → FAILED (panicked at `schemas` – no `components/schemas` yet, as expected). 6 other openapi tests still PASSED.

### Step 2: Add utoipa to finsight-core
- **File:** `crates/finsight-core/Cargo.toml:24`
- Added `utoipa = { version = "4", features = ["chrono"] }` (plan said `version="4"`; `chrono` feature added because 14 DTO fields are `DateTime<Utc>`/`Option<DateTime<Utc>>` and without it `ToSchema` would treat them as shallow objects and future `components/schemas` would still be shallow).
- **Verification:** `$env:CARGO_TARGET_DIR="E:\Workspace\FinSight\target"; cargo check -p finsight-core` → `Finished dev profile in 20.38s` (no errors). Using main target cache to avoid 10m openssl rebuild in isolated worktree target.

### Step 3: Derive ToSchema on all models
- **Script:** `tmp_transform.py` (removed after) automated:
  - Insert `use utoipa::ToSchema;` after `use specta::Type;`
  - Transform `#[derive(..., Type, ...)]` → `#[derive(..., Type, ToSchema, ...)]` for every derive containing `Type` (23 files)
  - Insert `#[schema(rename_all="...")]` after every `#[serde(rename_all="...")]` (covers `camelCase` for 16 structs, `PascalCase` for `AccountType`, `lowercase` for `TransactionStatus`)
- **Files modified (23):**
  `account.rs`, `agent_memory.rs`, `alert.rs`, `categorization.rs`, `category.rs`, `category_example.rs`, `category_proposal.rs`, `connection.rs`, `copilot.rs`, `holding.rs`, `household.rs`, `import_candidate.rs`, `institution.rs`, `manual_asset.rs`, `net_worth.rs`, `planned_transaction.rs`, `recipes.rs`, `rule.rs`, `rule_proposal.rs`, `security.rs`, `sync_run.rs`, `transaction.rs`, `transfer.rs` + `mod.rs` (no change, 24th entry in plan count). `budget.rs` listed in plan does not exist – `BudgetEnvelope` lives in `finsight-api/src/commands/budget.rs` and will be handled in Task 2.
- **Special handling:**
  - `NewCategorization`, `NewRule`, `NewImportCandidate`, `NewImportCandidateMatch` have `#[derive(Debug, Clone)]` without `Type`/`Serialize` – left unchanged (internal repo DTOs, not part of `components/schemas`). All `Type`-derived DTOs now have `ToSchema`.
  - `AccountPatch`, `TxnPatch`, `ManualAssetPatch`, `PlannedTransactionPatch` etc (Default+Deserialize+Type) also got `ToSchema`.
  - `EffectiveApr`/`PromoExpiryWarning` (`PartialEq` only) correctly left without `ToSchema`.
- **Verification scripts:** `tmp_check.py` (no missing `#[schema]`), `tmp_check2.py` (no missing `ToSchema` on `Type` derives) both passed.

### Step 4: Re-run shallow guard
- `cargo test -p finsight-openapi openapi_schemas_not_shallow` → still FAILED (expected – handlers not yet annotated, so `build_openapi()` still returns shallow `Value` without `components/schemas`). Confirms Task 1 alone is insufficient to pass guard, as spec predicted.
- `cargo test -p finsight-core --lib` with main target cache → **467 passed, 0 failed** (full core suite including `account::effective_apr_pct`, `copilot::MissingDataItem::dedup`, `investments`, `repos`, `sample::seed_dev_demo`, etc). No model logic changed.
- `$env:CARGO_TARGET_DIR=... cargo check -p finsight-core` → still 20s success.

### Step 5: Commit
```bash
git add Cargo.lock crates/finsight-core/Cargo.toml crates/finsight-core/src/models/*.rs crates/finsight-openapi/src/lib.rs
git commit -m "feat(core): derive ToSchema on all DTOs"
# 696bdb2 – 26 files changed, 145 insertions(+), 58 deletions(-)
```

---

## Test Summary

| Suite | Command | Result |
|-------|---------|--------|
| finsight-openapi (6 existing) | `cargo test -p finsight-openapi --lib` (filtered) | 6 passed |
| finsight-openapi shallow guard | `cargo test -p finsight-openapi openapi_schemas_not_shallow` | **FAILED as expected** (no schemas yet) |
| finsight-core full | `$env:CARGO_TARGET_DIR=E:\Workspace\FinSight\target; cargo test -p finsight-core --lib` | **467 passed, 0 failed, 140.32s** |
| finsight-core check | `cargo check -p finsight-core` (cached) | Finished in 20.38s, 0 errors |

## Commits

- `696bdb2 feat(core): derive ToSchema on all DTOs` – sole Task 1 commit (includes `Cargo.lock`, `crates/finsight-core/Cargo.toml`, 23 model files, and `crates/finsight-openapi/src/lib.rs` shallow test). Plan’s step 5 listed only core files; test file included for completeness and to keep TDD red state visible.

## Concerns / Notes

1. **Plan lists `budget.rs` in `crates/finsight-core/src/models/`** – file does not exist. `BudgetEnvelope`/`MemberBudgetEnvelope` etc are defined in `finsight-api/src/commands/budget.rs` and will get `ToSchema` in Task 2. Count mismatch is plan vs reality, not a gap.
2. **`utoipa` feature `chrono` required** – plan snippet `utoipa = { version = "4" }` without features would compile but `DateTime<Utc>` fields would generate `{type: object}` without `format: date-time`, keeping schemas shallow and defeating Task 1’s purpose. Added `features=["chrono"]` to `finsight-core/Cargo.toml`. `uuid` not needed (ids are `String` in models); can add if future DTO uses `Uuid` directly.
3. **Worktree `target/` is cold** – fresh worktree rebuilds `openssl-sys`/`libsqlite3-sys` from scratch (~10m on Windows). Verified via `CARGO_TARGET_DIR=E:\Workspace\FinSight\target` reuse (20s check, 140s full test). No logic change; slow compile is environment gap, not regression.
4. **Shallow guard still red** – correct per spec. Will turn green only after Task 2 (`#[utoipa::path]` on 229 handlers) + Task 3 (`#[derive(OpenApi)]` collecting `components/schemas` + `paths` with `$ref`). No further action in Task 1.
5. **CRLF warnings** – Git warns `CRLF will be replaced by LF` for 23 model files (Windows worktree). No content effect; committed as LF.

## Report Path
`E:/Workspace/FinSight/.git/worktrees/openapi-deep-schema/sdd/task-1-report.md` (also mirrored in worktree’s `target` cache via `CARGO_TARGET_DIR`; primary copy is at the requested git worktree metadata path)

## Self-Review Checklist
- [x] TDD: failing test written before code, verified red → still red (expected) after DTO work
- [x] Every `Type` derive now has `ToSchema` (verified via script)
- [x] Every `serde(rename_all=...)` has matching `schema(rename_all=...)` (verified)
- [x] `cargo check -p finsight-core` passes (20.38s cached)
- [x] `cargo test -p finsight-core --lib` 467/467 passes
- [x] No `tauri` dep introduced (`finsight-core` still only `specta` + `utoipa`)
- [x] Commit is single, message matches spec, diff is 26 files
- [x] No hand-edit of generated `openapi.json`/`openapi.ts` (not yet generated)

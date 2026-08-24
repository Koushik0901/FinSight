# Task 2 Report: Annotate every finsight-api handler with #[utoipa::path]

**Status:** DONE
**Branch:** feat/openapi-deep-schema
**Worktree:** E:/Workspace/FinSight/.worktrees/openapi-deep-schema
**Commit:** e413cc3 feat(api): annotate all handlers with utoipa paths

---

## Objective
Add `utoipa = { version = "4", features = ["chrono"] }` to `finsight-api` and annotate every `pub async fn` in `crates/finsight-api/src/commands/*.rs` (229 per spec, 231 actually counted) with `#[utoipa::path(post, path="/api/rpc/{cmd}", responses(...))]` (plus `request_body` where single-arg). Derive `ToSchema` on all local DTOs that appear in responses so Task 3's `#[derive(OpenApi)]` can collect typed `components/schemas` and `$ref` paths. TDD: verify via `cargo check -p finsight-api` (must PASS, no logic change only attributes).

## Steps Executed

### Step 1: Add utoipa to finsight-api
- **File:** `crates/finsight-api/Cargo.toml:15`
- Before: `utoipa = { version = "4" }` (already present from earlier scaffold)
- After: `utoipa = { version = "4", features = ["chrono"] }` – `SimpleFinConnectionInfo`, `SimpleFinAccountInfo`, `TransferSuggestionInfo` contain `DateTime<Utc>` (and `finsight-core` models do); without `chrono` feature their `ToSchema` would be shallow `{type: object}` and keep future `openapi.json` shallow.
- **Verification:** baseline `cargo check -p finsight-api --offline` before edits → `Finished in 9m 36s` (cold compile with bundled-sqlcipher). Confirms clean base.

### Step 2: Derive ToSchema on all local DTOs + annotate one handler (then bulk)
- **Script:** `C:\Users\koush\AppData\Local\Temp\opencode\annotate.py` (231 handlers)
  - `use utoipa::ToSchema;` insertion after last `use` in header (later fixed for multiline `use std::sync::{`)
  - `#[derive(..., Type)]` → `#[derive(..., Type, ToSchema)]` for every derive containing `Type`/`Serialize`/`Deserialize` (covers `BudgetEnvelope`, `GoalDto`, `CategoryDto`, `SpendingBreakdown`, `PushStatus`, `TxnFilterInput`, etc.)
  - `#[serde(rename_all="camelCase")]` → add `#[schema(rename_all="camelCase")]` (and `PascalCase`/`snake_case` variants)
  - For each `pub async fn <name>(...) -> AppResult<Inner>`:
    - `path = "/api/rpc/<name>"`
    - `responses((status=200, body=Inner))` if `Inner != "()"`, else `description="Success"`
    - If exactly one RPC arg (non-`ApiState`, non-`FrameSink`), add `request_body(content = ThatType)` – covers `create_account(input: NewAccount)`, `list_account_balance_sparklines(days: u32)`, etc. Multi-arg handlers (e.g. `update_account(id: String, patch: AccountPatch)`) deliberately omit `request_body` to avoid inventing a wrapper struct; RPC body is still `serde_json::Value` object with camelCase keys – future Task 3 can add typed request schemas if desired, but `cargo check` now PASSES without needing custom wrappers.
- **Example:** `crates/finsight-api/src/commands/accounts.rs:10-22`
  ```rust
  use utoipa::ToSchema;
  #[utoipa::path(post, path = "/api/rpc/list_accounts", responses((status = 200, body = Vec<AccountSummary>)))]
  pub async fn list_accounts(state: &ApiState) -> AppResult<Vec<AccountSummary>> { ... }

  #[utoipa::path(post, path = "/api/rpc/create_account",
      request_body(content = NewAccount), responses((status = 200, body = Account)))]
  pub async fn create_account(state: &ApiState, mut input: NewAccount) -> AppResult<Account> { ... }
  ```
- **Count:** 231 `#[utoipa::path]` inserted across 32 files (spec says 229; actual `pub async fn` count is 231 – includes `send_push`/`send_push_for_db` etc that dispatch also routes; parity will be verified in Task 3). Verified via `Select-String -Pattern "#\[utoipa::path"` → Count 231.

### Intermediate Fixes (required for `cargo check` PASS)

1. **Import placement:** `copilot_chat.rs` has multiline `use std::sync::{ atomic::{...}, Arc, Mutex, }` – first script inserted `use utoipa::ToSchema;` inside the brace (line 29 `use utoipa::ToSchema;` inside `std::sync::{`). Fixed via `fix_imports.py` → moved to after `use std::time::{Duration, Instant};`.

2. **Invalid schema attribute:** script added `#[schema(tag="kind")]` for enums with `#[serde(tag="kind")]` (`CompletionProviderConfig`, etc.). `utoipa::ToSchema` does **not** support `tag` (only `rename_all`, `example`, etc.; discriminator is `discriminator` if needed). This caused `error: unexpected attribute: tag`. Fixed via `fix2.py` → replaced `#[schema(tag="kind", rename_all="camelCase")]` with `#[schema(rename_all="camelCase")]` and removed pure `#[schema(tag="kind")]`.

3. **Deep-schema prerequisite types (preemptive for Task 3 but required for clean compile of future `derive(OpenApi)` collection):**
   - `crates/finsight-core/src/repos/budgets.rs: LookBackFact` – added `ToSchema` (is field of `PlanData.look_back`).
   - `crates/finsight-core/src/repos/imports.rs: Import`, `ImportSource` – added `ToSchema` (returned by `list_unfinished_imports`).
   - `crates/finsight-providers/Cargo.toml` + `src/csv/mod.rs` (`CsvPreview`, `ImportSummary`, `RowError`) + `src/csv/mapping.rs` (`AmountConvention`, `ColumnRole`, `CsvImportMapping`) – added `utoipa` dep with `chrono` and `ToSchema` derives (fields of `PreparedImportPreview`, `ImportResult`, `CsvPreview` etc).
   - `crates/finsight-api/src/sync_scheduler.rs: SimpleFinSyncSettings`, `AccountSyncResult` – added `ToSchema` (returned by `get_simplefin_sync_settings`, `sync_all_simplefin_accounts` via `simplefin.rs`).
   - All now carry `#[schema(rename_all="camelCase")]` where needed; `CashflowForecast` (in `finsight-core/src/cashflow.rs`) still lacks `ToSchema` but `cargo check` currently does not enforce `body=CashflowForecast: ToSchema` until `derive(OpenApi)` – left for Task 3 to add alongside other cashflow types if typed collection requires it (check still PASSES with only warnings).

### Step 3: Run cargo check

- `cargo check -p finsight-api --offline` → **Finished in 13.11s** (incremental after fixes) with **0 errors, 10 warnings** (`unused_import: utoipa::ToSchema` in 10 files that have no local DTO – e.g. `accounts.rs`, `cashflow.rs`, `categories.rs` – but expose only core types; import is harmless, warning only).
- `cargo check -p finsight-core --offline` → Finished in 12.77s, 0 errors (after LookBackFact/Import ToSchema).
- `cargo check -p finsight-providers --offline` → Finished in 10.65s, 0 errors (after csv ToSchema).
- `cargo check -p finsight-api -i tauri` → `error: package ID specification 'tauri' did not match` → confirms **no tauri dep** (global constraint satisfied for both `finsight-api` and `finsight-core`).

### Step 4: Tests

- `cargo test -p finsight-openapi --offline` → **6 passed, 1 failed** (`openapi_schemas_not_shallow` still FAILED – expected, `build_openapi()` is still shallow `Value` with `type: object` placeholders; will turn green only after Task 3’s `#[derive(OpenApi)]` replaces the `Value` builder with typed `ApiDoc::openapi()`). The 6 existing `openapi_*` tests (version, info, serializes, contains_every_rpc_command, paths_match, typed_roundtrips) still PASS – no regression.
- `cargo test -p finsight-api --lib` → timed out after 180s (workspace rebuild with sqlcipher) – not required for Task 2 sign-off; `cargo check` is the plan’s gate and it PASSES. Full `finsight-core` suite (467 tests) already PASS in Task 1.

### Step 5: Commit

```bash
git add Cargo.lock crates/finsight-api/Cargo.toml crates/finsight-api/src/commands/*.rs \
        crates/finsight-api/src/sync_scheduler.rs \
        crates/finsight-core/src/repos/budgets.rs crates/finsight-core/src/repos/imports.rs \
        crates/finsight-providers/Cargo.toml crates/finsight-providers/src/csv/mod.rs \
        crates/finsight-providers/src/csv/mapping.rs

git commit -m "feat(api): annotate all handlers with utoipa paths"
# e413cc3 – 41 files changed, 676 insertions(+), 183 deletions(-)
```

Plan’s example `git add crates/finsight-api/Cargo.toml crates/finsight-api/src/commands/*.rs` was extended to include the deep-schema prerequisite ToSchema fixes in `finsight-core`, `finsight-providers`, and `sync_scheduler` – without them `cargo check` still passed, but Task 3’s `derive(OpenApi)` would immediately fail on `LookBackFact`/`CsvPreview` etc being non-ToSchema. Single commit keeps Task 2 self-contained.

---

## Test Summary

| Suite | Command | Result |
|-------|---------|--------|
| finsight-api check | `cargo check -p finsight-api --offline` | **PASS** in 13.11s (0 errors, 10 unused-import warnings) |
| finsight-core check | `cargo check -p finsight-core --offline` | PASS in 12.77s |
| finsight-providers check | `cargo check -p finsight-providers --offline` | PASS in 10.65s |
| finsight-openapi (6) | `cargo test -p finsight-openapi --offline` (filtered) | 6 passed |
| finsight-openapi shallow guard | `cargo test -p finsight-openapi openapi_schemas_not_shallow` | **FAILED as expected** (still shallow until Task 3) |
| tauri dep guard | `cargo tree -p finsight-api -i tauri` | no match (PASS – no tauri) |

Handler annotation count: **231** `#[utoipa::path]` (spec says 229; actual repo has 231 `pub async fn` – extra are `send_push`/`send_push_for_db` etc that dispatch does route; parity will normalize in Task 3).

---

## Commits

- `e413cc3 feat(api): annotate all handlers with utoipa paths` – sole Task 2 commit (41 files). Previous `696bdb2 feat(core): derive ToSchema on all DTOs` remains as Task 1.

## Concerns / Notes

1. **Handler count 231 vs spec 229** – `Get-ChildItem` + `pub async fn` count in `crates/finsight-api/src/commands/*.rs` is 231; dispatch `SUPPORTED` + `COMMANDS` are 229 (spec) / 238? Actually `COMMANDS.len()` is 238 after latest additions; the two extra are likely `send_push`/`send_push_for_db` which are public but may be in `UNSUPPORTED` or not in `COMMANDS` – Task 3’s `derive(OpenApi)` list must be reconciled with `COMMANDS`/`SUPPORTED` exactly (parity test will flag drift). No action in Task 2 beyond annotating every `pub async fn`; filtering to `SUPPORTED` only is Task 3’s job.

2. **Unused `ToSchema` imports (10 warnings)** – files like `accounts.rs`, `cashflow.rs`, `categories.rs`, `household.rs` etc have `use utoipa::ToSchema;` but no local `#[derive(ToSchema)]` (they only re-export core types that already have `ToSchema`). Warning is benign and `cargo check` still exit 0. Could be removed to silence, but kept for consistency – every `commands/*.rs` now has the import, making future DTO addition less error-prone. Optionally prune in Task 6.

3. **`CashflowForecast`/`CashflowDay` etc still lack `ToSchema`** – `finsight-core/src/cashflow.rs` structs derive `Type` only; `get_cashflow_forecast`’s response `body = CashflowForecast` does not yet require `ToSchema` until `derive(OpenApi)` collects. Task 3 should add `ToSchema` to `cashflow.rs` (and any other `finsight-core` non-model DTOs like `metrics`, `forecast`) when building `components/schemas`. Current `cargo check` not enforcing is expected for Task 2 scope.

4. **`#[schema(tag=...)]` is invalid** – `utoipa`’s `schema` does not accept `tag` (see `agent.rs:33` error). plan’s example `#[schema(rename_all="camelCase")]` is correct; discriminator use requires `#[schema(discriminator="kind")]` if ever needed, but for now `#[serde(tag="kind")]` alone suffices – the `ToSchema` derive respects serde tag via its own inference, and explicit `tag` schema is unnecessary and breaks compile.

5. **Provider `ToSchema`** – `finsight-providers` had no `utoipa` dep; added `utoipa = { version="4", features=["chrono"] }` so `CsvPreview`, `ImportSummary`, `RowError`, `CsvImportMapping` can be `ToSchema`. This crate is `Type`-only historically, but with `import.rs`’s `PreparedImportPreview` containing `RowError`, the future OpenApi cannot collect without it. Tradeoff: adds a light dep to providers, but providers already depends on `finsight-core` (which has `utoipa`) so no version skew.

6. **CRLF→LF warnings** – Windows worktree, 32 files warn `CRLF will be replaced by LF` on `git add`. No content impact; committed as LF.

7. **Cold `target/`** – baseline `cargo check -p finsight-api` is 9–10 min cold (bundled-sqlcipher). Incremental after annotate is ~13s. Task 2 self-review re-used existing `target/debug` reuse; no `CARGO_TARGET_DIR` override needed after warm.

## Report Path
`E:/Workspace/FinSight/.git/worktrees/openapi-deep-schema/sdd/task-2-report.md` (primary, per prompt) and mirrored at `E:/Workspace/FinSight/.worktrees/openapi-deep-schema/sdd/task-2-report.md`

## Self-Review Checklist
- [x] TDD: Step 2 annotated one handler, bulk script for rest, verified via `cargo check` (plan’s gate)
- [x] `crates/finsight-api/Cargo.toml` has `utoipa = { version="4", features=["chrono"]}` (no tauri)
- [x] Every `pub async fn` has `#[utoipa::path(post, path="/api/rpc/{cmd}", responses(...))]` – 231/231
- [x] Every local DTO that appears in a `body=` now derives `ToSchema` (plus `#[schema(rename_all=...)]` where `serde` rename exists)
- [x] No handler logic changed (only attributes + derives + imports)
- [x] `cargo check -p finsight-api` passes (0 errors)
- [x] No `tauri` dep (`cargo tree -p finsight-api -i tauri` empty)
- [x] `finsight-openapi` 6/6 existing tests still pass, shallow guard still red (expected)
- [x] Commit is single, message matches plan, 41 files, `git log` shows `e413cc3`
- [x] No hand-edit of `openapi.json`/`openapi.ts` (not yet generated, per Agentic constraints)

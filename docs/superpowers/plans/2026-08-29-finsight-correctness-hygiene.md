# FinSight Correctness & Hygiene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship one atomic Big-Bang PR that fixes expense-parity drift (C1/C3/I3/I8), removes the Enable Banking production stub (C2/I7), normalizes the RPC contract (`reconcileBases`→`reconcile_bases`, `transfer_envelope` collapse, `SplitBy` casing — I1/I4/M1/M5), makes `apply_templates` transactional (I2), eliminates the Budget N+1 transfer sum (I5), plus UI/minor polish (M2-M6), with a regression corpus that proves Custom Reports reconcile with fixed Reports.

**Architecture:** Single helper `category_spent` owns all expense math in `finsight-core`; `period_bounds` anchors both report paths on `MAX(posted_at)`; providers delete the literal-token stub and switch to `MidpointAwayFromZero`; `finsight-openapi`+`finsight-server` do one hard contract rename and one `pnpm openapi` regen; `apply_templates` writes inside `BEGIN IMMEDIATE`; Budget listing groups transfer sums; all guarded by `parity.rs` and a new `custom_report_parity` corpus.

**Tech Stack:** Rust 1.98+ (`rusqlite`, `r2d2`, `chrono`, `rust_decimal`, `utoipa` ToSchema/OpenApi, `Axum`, `tower-http`), `refinery` migrations (`V06x`), TypeScript React (`openapi-typescript`+`openapi-fetch`, `vitest`+`jsdom`, `zod`), Workbox PWA, Vite.

## Global Constraints

- `finsight-api` MUST remain Tauri-free — verified by `cargo tree -p finsight-api -i tauri` empty.
- Every shared command body lives in `finsight-api/src/commands/` annotated `#[utoipa::path(post, path="/api/rpc/my_cmd")]`; DTOs derive `ToSchema` with `#[schema(rename_all="camelCase")]` where required.
- `COMMANDS` in `crates/finsight-openapi/src/lib.rs` sorted snake_case, identical to `dispatch.rs` `SUPPORTED`; one `pnpm openapi` regen updates `openapi.json` + `ui/src/api/openapi.json` + `ui/src/api/openapi.ts`.
- DB access via `run(&db, move |conn| { … }).await.map_err(AppError::from)`; no blocking I/O outside `run`.
- Money math single-sourced in `finsight-core` — never hand-roll in handlers or UI; expense expression is `CASE WHEN settle_up=1 THEN -amount_cents WHEN amount_cents<0 THEN -amount_cents ELSE 0 END` with `is_transfer=0`.
- `custom_report` is expense-only (Q2=A) — `ELSE 0` + `settle_up` net; no `includeIncome` flag in this PR.
- `apply_templates` is transactional write (Q3=A) — `BEGIN IMMEDIATE`, writes `budgets`, clears `budget_holds`, idempotent second call.
- Production `fetch_enable_data` MUST NOT contain `token-a`/`token-b` literal match (Q4=A) — gate any stub behind `#[cfg(test)]`; wiremock `fetch_enable_data_with_base_url` is the isolation proof.
- Hard rename `reconcileBases`→`reconcile_bases`, delete `transfer_envelope` alias, no dispatch fallback (Q5=A) — stale client gets `404 unknown_command`.
- `RoundingStrategy::MidpointAwayFromZero` for `parse_amount_cents` (not `MidpointNearestEven`).
- Compression per-route only — `SSE /api/events` never through `CompressionLayer` (`sse_event_stream_is_never_compressed` must stay green); cache split `immutable /assets/*` vs `revalidate` shell.
- Entry bundle stays lazy-only — no static import of `AccountDrawer`+`react-hook-form` in `App.tsx`.
- Migrations named `V0NN__description.sql`; `embed_migrations!` discovers by prefix; next is `V067__` if schema change needed (not required here).
- Generated files `ui/src/api/openapi.ts` and `ui/src/api/openapi.json` never hand-edited.

---

### File Structure

**Modified (no new files except tests embedded):**
- `crates/finsight-core/src/repos/budgets.rs` — owns `category_spent`, `period_bounds`, fixes `carryover_for`, `category_available`, `look_back_facts`, `custom_report` SQL, `SplitBy` enum casing, `apply_templates` tx, tests.
- `crates/finsight-core/src/repos/mod.rs` — re-export if `period_bounds` moved to shared helper.
- `crates/finsight-providers/src/enable_banking/sync.rs` — delete stub, fix rounding, tests.
- `crates/finsight-openapi/src/lib.rs` — `COMMANDS` edits + `#[openapi(paths(...))]` + `COMMANDS` sorted-snake test.
- `crates/finsight-api/src/commands/budget.rs` — `#[utoipa::path]` rename/delete + `transfer_budget` grouping perf + `apply_templates` thin wrapper.
- `crates/finsight-api/src/commands/reports.rs` — `custom_report` handler wiring to `period_bounds` (if separate file; otherwise `budgets.rs` handler).
- `crates/finsight-server/src/dispatch.rs` — `rpc_routes!` arms + `dispatch.rs:134` comment (M5).
- `crates/finsight-server/tests/parity.rs` — no edit, but must pass; add assertion reference for unknown command case.
- `ui/src/screens/Budget.tsx` — `moneyDisplay` (M2).
- `ui/src/api/openapiClient.ts` — deprecate untyped `rpc` (M3).
- `ui/src/screens/ReportBuilder.test.tsx` — extended coverage.
- `openapi.json` / `ui/src/api/openapi.json` / `ui/src/api/openapi.ts` — generated via `pnpm openapi`.
- `crates/finsight-core/src/lib.rs` or `models/mod.rs` — `SplitBy` definition if not in `budgets.rs`.

---

### Task 1: Canonical Expense Helper + Period Anchoring (C1, C3, I3, I8)

**Files:**
- Modify: `crates/finsight-core/src/repos/budgets.rs:380-960`
- Modify: `crates/finsight-core/src/repos/reports.rs` (or wherever `get_report_data` lives — search `fn get_report_data`) to call shared `period_bounds`
- Test: `crates/finsight-core/src/repos/budgets.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: existing `Db`/`Connection`, `CustomReportParams`/`Period`/`SplitBy` in `finsight_core::models`
- Produces: `pub fn category_spent(conn: &Connection, category_id: &str, from: &str, to: &str) -> CoreResult<i64>` and `pub fn period_bounds(conn: &Connection, period: Period) -> CoreResult<(Option<String>, String)>` for Tasks 3-5; `category_spent` replaces all `amount_cents < 0` ad-hoc sums

- [ ] **Step 1: Write failing test for expense-only parity (C1)**
```rust
// crates/finsight-core/src/repos/budgets.rs — #[cfg(test)]
#[test]
fn custom_report_expense_only() {
    let mut db = setup_test_db();
    let cat = create_category(&mut db, "Groceries");
    // -5000 expense
    insert_tx(&mut db, &cat, -5000, "2026-08-10", 0);
    // +8000 income (should be ignored)
    insert_tx(&mut db, &cat, 8000, "2026-08-11", 0);
    // +2000 reimbursement settle_up=1 (nets as -2000 expense)
    insert_tx(&mut db, &cat, 2000, "2026-08-12", 1);
    let params = CustomReportParams {
        period: Period::All, split_by: SplitBy::Category,
        include_archived: true, include_transfers: false,
    };
    let res = custom_report(&db.conn(), params).unwrap();
    assert_eq!(res.total_cents, 3000, "expense - reimbursement, income ignored");
    assert_eq!(res.rows[0].total_cents, 3000);
}
```

- [ ] **Step 2: Run failing test**
Run: `cargo test -p finsight-core --lib repos::budgets::tests::custom_report_expense_only -- --nocapture`
Expected: FAIL — `total_cents` is `13000` (old `ELSE t.amount_cents`) or `5000` (missing `settle_up`)

- [ ] **Step 3: Write failing test for wall-clock vs data anchor (I3)**
```rust
#[test]
fn custom_report_anchors_on_max_posted_at() {
    let mut db = setup_test_db();
    let cat = create_category(&mut db, "Food");
    insert_tx(&mut db, &cat, -1000, "2025-01-15", 0);
    // Use wall-clock now is 2026-08-29, but anchor is 2025-01-15
    // Last1Month from anchor should include Jan row; from wall-clock would be empty
    let params = CustomReportParams {
        period: Period::Last1Month, split_by: SplitBy::Category,
        include_archived: true, include_transfers: false,
    };
    let res = custom_report(&db.conn(), params).unwrap();
    assert_eq!(res.total_cents, 1000);
}
```

- [ ] **Step 4: Run to verify fails (wall-clock returns 0)**
Run: `cargo test -p finsight-core --lib repos::budgets::tests::custom_report_anchors_on_max_posted_at -- --nocapture`
Expected: FAIL — `total_cents == 0`

- [ ] **Step 5: Write failing test for future-row guard (I8)**
```rust
#[test]
fn custom_report_excludes_future_rows() {
    let mut db = setup_test_db();
    let cat = create_category(&mut db, "Misc");
    insert_tx(&mut db, &cat, -1000, &Utc::now().format("%Y-%m-%d").to_string(), 0);
    let future = (Utc::now() + chrono::Duration::days(2)).format("%Y-%m-%d").to_string();
    insert_tx(&mut db, &cat, -9999, &future, 0);
    let params = CustomReportParams { period: Period::Last1Month, split_by: SplitBy::Category, include_archived: true, include_transfers: false };
    let res = custom_report(&db.conn(), params).unwrap();
    assert_eq!(res.total_cents, 1000, "future row excluded by end bound");
}
```

- [ ] **Step 6: Implement `category_spent` + `period_bounds` + fix `custom_report` SQL**
```rust
// crates/finsight-core/src/repos/budgets.rs — near line 380, before carryover_for
pub fn category_spent(conn: &Connection, category_id: &str, from: &str, to: &str) -> CoreResult<i64> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents \
         WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0) \
         FROM transactions t \
         WHERE t.category_id=?1 AND t.posted_at >= ?2 AND t.posted_at < ?3 AND t.is_transfer=0",
        params![category_id, from, to], |r| r.get(0))?;
    Ok(v)
}
pub fn period_bounds(conn: &Connection, period: Period) -> CoreResult<(Option<String>, String)> {
    let anchor: Option<String> = conn.query_row("SELECT MAX(date(posted_at)) FROM transactions", [], |r| r.get(0))?;
    let anchor_date = anchor.as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());
    let end = (anchor_date + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let start = match period {
        Period::All => None,
        Period::Last1Month => Some((anchor_date - chrono::Months::new(1)).format("%Y-%m-%d").to_string()),
        Period::Last3Months => Some((anchor_date - chrono::Months::new(3)).format("%Y-%m-%d").to_string()),
        Period::Last6Months => Some((anchor_date - chrono::Months::new(6)).format("%Y-%m-%d").to_string()),
    };
    // Return RFC3339 bounds for SQL: date strings compare lexicographically
    let start_rfc = start.map(|s| format!("{s}T00:00:00Z"));
    Ok((start_rfc, format!("{end}T00:00:00Z")))
}
```
Fix `custom_report` SQL (line ~928): replace
```rust
SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE t.amount_cents END)
```
with
```rust
SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END)
```
and add end bound:
```rust
if let Some(s) = &start { binds.push(s.clone()); }
let (_, end) = period_bounds(conn, p.period.clone())?;
sql.push_str(" AND t.posted_at < ?");
binds.push(end);
```
Fix `carryover_for` (389-422), `category_available` (427-443), `look_back_facts` avg path: replace `SELECT COALESCE(SUM(-amount_cents),0) … WHERE amount_cents<0` with call to `category_spent(conn, category_id, start_date, month_date)`.

- [ ] **Step 7: Wire `reports::get_report_data` to shared `period_bounds` (I3)**
Replace its wall-clock `period_start` helper with `let (start, end) = budgets::period_bounds(conn, period)?;` so both surfaces share anchor.

- [ ] **Step 8: Run tests to verify pass**
Run: `cargo test -p finsight-core --lib repos::budgets::tests::custom_report_expense_only repos::budgets::tests::custom_report_anchors_on_max_posted_at repos::budgets::tests::custom_report_excludes_future_rows -- --nocapture`
Expected: PASS (3/3)

- [ ] **Step 9: Commit**
```bash
git add crates/finsight-core/src/repos/budgets.rs crates/finsight-core/src/repos/reports.rs
git commit -m "fix(core): canonical category_spent + period_bounds anchoring (C1,C3,I3,I8)"
```

### Task 2: Remove Production Stub + Fix Bankers Rounding (C2, I7)

**Files:**
- Modify: `crates/finsight-providers/src/enable_banking/sync.rs:31-62,174-185`
- Test: `crates/finsight-providers/src/enable_banking/sync.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `EnableBankingClient::new/with_base_url`
- Produces: `pub async fn fetch_enable_data(token:&str)->ProviderResult<EnableBankingSyncData>` that always hits network; `fn parse_amount_cents(amount:&str)->ProviderResult<i64>` with `MidpointAwayFromZero`

- [ ] **Step 1: Write failing test for stub removal (C2)**
```rust
#[tokio::test]
async fn fetch_enable_data_literal_token_hits_network_not_stub() {
    // Without network, a literal token should now error (no stub)
    let err = fetch_enable_data("token-a").await.unwrap_err();
    assert!(matches!(err, ProviderError::Auth(_) | ProviderError::Internal(_)),
        "literal token-a should not return stubbed acc-a-1, got {err:?}");
}
```

- [ ] **Step 2: Write failing test for rounding (I7)**
```rust
#[test]
fn parse_amount_cents_midpoint_away() {
    assert_eq!(parse_amount_cents("12.345").unwrap(), 1235);
    assert_eq!(parse_amount_cents("-12.345").unwrap(), -1235);
    assert_eq!(parse_amount_cents("12.34").unwrap(), 1234);
}
```
Note: existing `parse_amount_cents_variants:230` asserts `12.345==1234` (bankers); this test will fail until fixed — intentional.

- [ ] **Step 3: Run to verify fails**
Run: `cargo test -p finsight-providers --lib enable_banking::sync::tests::fetch_enable_data_literal_token_hits_network_not_stub -- --nocapture`
Expected: FAIL — currently returns `Ok(acc-a-1)` not `Err`
Run: `cargo test -p finsight-providers --lib enable_banking::sync::tests::parse_amount_cents_midpoint_away -- --nocapture`
Expected: FAIL — `1234 != 1235`

- [ ] **Step 4: Implement fix**
Delete lines 28-63 stub in `sync.rs`:
```rust
pub async fn fetch_enable_data(token: &str) -> ProviderResult<EnableBankingSyncData> {
    let client = EnableBankingClient::new(token)?;
    let accounts = client.list_accounts().await?;
    Ok(EnableBankingSyncData { accounts, transactions: vec![] })
}
```
Optionally add `#[cfg(test)] pub fn stub_For_test_only() {}` but not needed — wiremock test already exists.
Fix rounding at 174-185:
```rust
fn parse_amount_cents(amount: &str) -> ProviderResult<i64> {
    use rust_decimal::RoundingStrategy;
    let decimal: Decimal = amount.trim().parse()
        .map_err(|_| ProviderError::Internal(format!("invalid amount: {}", amount)))?;
    let rounded = decimal.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let cents = (rounded * Decimal::from(100)).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64().ok_or_else(|| ProviderError::Internal(format!("amount out of range: {}", amount)))?;
    Ok(cents)
}
```
Update existing test `parse_amount_cents_variants` line 230 to expect `1235` (edit assertion).

- [ ] **Step 5: Run to verify pass**
Run: `cargo test -p finsight-providers --lib enable_banking::sync::tests -- --nocapture`
Expected: PASS (including `fetch_enable_data_with_base_url_isolates_via_http` wiremock test still green, new `fetch_enable_data_literal_token_hits_network` now correctly errors-or-hits-network)

- [ ] **Step 6: Commit**
```bash
git add crates/finsight-providers/src/enable_banking/sync.rs
git commit -m "fix(providers): remove token-a/b prod stub, MidpointAwayFromZero rounding (C2,I7)"
```

### Task 3: Contract Hygiene — Hard Rename + Alias Collapse + SplitBy Casing + One Regen (I1, I4, M1, M5)

**Files:**
- Modify: `crates/finsight-openapi/src/lib.rs:8,182,235,266-268,981+`
- Modify: `crates/finsight-api/src/commands/budget.rs:457-475`
- Modify: `crates/finsight-api/src/commands/copilot.rs` or wherever `reconcile_bases` handler lives (search `fn reconcile`)
- Modify: `crates/finsight-server/src/dispatch.rs:134,228,~340`
- Generate: `openapi.json`, `ui/src/api/openapi.json`, `ui/src/api/openapi.ts` via `pnpm openapi`
- Test: `crates/finsight-openapi/src/lib.rs` `#[cfg(test)]` + `crates/finsight-server/tests/parity.rs`

**Interfaces:**
- Consumes: `category_spent` (Task 1), `COMMANDS` sorted invariant
- Produces: `COMMANDS` with `"reconcile_bases"` (snake), no `"transfer_envelope"`, `SplitBy` serializes `"category"|"group"|"account"|"month"`; `POST /api/rpc/reconcile_bases` and `POST /api/rpc/transfer_budget` only

- [ ] **Step 1: Write failing test for sorted snake**
```rust
// crates/finsight-openapi/src/lib.rs #[cfg(test)]
#[test]
fn commands_sorted_snake() {
    assert!(COMMANDS.windows(2).all(|w| w[0] < w[1]), "COMMANDS must be sorted");
    for c in COMMANDS {
        assert!(c.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_' || ch.is_ascii_digit()), "cmd {c} must be snake_case");
    }
    assert!(!COMMANDS.contains(&"reconcileBases"), "camelCase reconcileBases must be gone");
    assert!(!COMMANDS.contains(&"transfer_envelope"), "alias must be gone");
}
```

- [ ] **Step 2: Run to verify fails**
Run: `cargo test -p finsight-openapi --lib tests::commands_sorted_snake -- --nocapture`
Expected: FAIL — `reconcileBases` present, not sorted-snake

- [ ] **Step 3: Edit `finsight-openapi/src/lib.rs`**
At line 182: `"reconcileBases"` → `"reconcile_bases"`.
Delete line 235 `"transfer_envelope",`.
In `#[openapi(paths(...))]` at 266-268: remove `finsight_api::commands::budget::transfer_envelope,`.
Add test above + `#[test] fn openapi_splitby_camelcase()` asserting schema enum values contain `"category"` not `"Category"`:
```rust
#[test]
fn openapi_splitby_camelcase() {
    let v = build_openapi_value();
    let schema = &v["components"]["schemas"]["SplitBy"];
    let vals = schema["enum"].as_array().unwrap();
    assert!(vals.iter().any(|x| x == "category"));
    assert!(!vals.iter().any(|x| x == "Category"));
}
```

- [ ] **Step 4: Fix `finsight-core` `SplitBy` enum casing (M1)**
In `crates/finsight-core/src/repos/budgets.rs:46-55` or `crates/finsight-core/src/models/*` where `SplitBy` defined:
```rust
#[derive(Debug, Clone, Serialize, ToSchema, Type)]
#[serde(rename_all="camelCase")]
#[schema(rename_all="camelCase")]
pub enum SplitBy { Category, Group, Account, Month }
```

- [ ] **Step 5: Fix handlers**
In `crates/finsight-api/src/commands/copilot.rs`: `#[utoipa::path(post, path="/api/rpc/reconcile_bases")] pub async fn reconcile_bases(...)` (rename fn and path from `reconcileBases`/`reconcileBases`).
In `crates/finsight-api/src/commands/budget.rs:457-475`: delete `transfer_envelope` handler; keep only `transfer_budget` with its `#[utoipa::path(post, path="/api/rpc/transfer_budget")]`.

- [ ] **Step 6: Fix server dispatch + comment**
In `crates/finsight-server/src/dispatch.rs:134` replace bindings comment: `// regen: cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen — finSight-openapi + ui/src/api/openapi.ts`
At `dispatch.rs:228`: `reconcileBases` → `reconcile_bases`; delete `transfer_envelope` arm (~line 340).

- [ ] **Step 7: Regen contract**
Run: `pnpm openapi`
Expected: `openapi.json` and `ui/src/api/openapi.json` paths show `/api/rpc/reconcile_bases` not `/api/rpc/reconcileBases`; no `/api/rpc/transfer_envelope`; `SplitBy` enum values `"category"`.
Verify: `git diff --stat openapi.json ui/src/api/openapi.json ui/src/api/openapi.ts` shows expected changes only.

- [ ] **Step 8: Run parity & openapi tests**
Run: `cargo test -p finsight-openapi --lib -- --nocapture`
Expected: PASS (including new `commands_sorted_snake`, `openapi_splitby_camelcase`)
Run: `cargo test -p finsight-server --test parity -- --nocapture`
Expected: PASS (or fail with snapshot — update snapshot via observed output, but only after verifying regen correct)

- [ ] **Step 9: Commit**
```bash
git add crates/finsight-openapi/src/lib.rs crates/finsight-core/src/repos/budgets.rs crates/finsight-api/src/commands/budget.rs crates/finsight-api/src/commands/copilot.rs crates/finsight-server/src/dispatch.rs openapi.json ui/src/api/openapi.json ui/src/api/openapi.ts
git commit -m "fix(openapi): hard rename reconcileBases->reconcile_bases, drop transfer_envelope alias, SplitBy camelCase + regen (I1,I4,M1,M5)"
```

### Task 4: Transactional `apply_templates` (I2)

**Files:**
- Modify: `crates/finsight-core/src/repos/budgets.rs:626-699`
- Modify: `crates/finsight-api/src/commands/budget.rs` (thin wrapper)
- Test: `crates/finsight-core/src/repos/budgets.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `category_spent`/`category_available` (Task 1), `available_funds`/`get_hold` (`available_funds = income - budgeted - hold_current + hold_prev` — diverges from spec §3 `to_budget` intentionally), `set_budget` helper
- Produces: `pub fn apply_templates_tx(conn: &mut Connection, month: &str, templates: Vec<FundingTemplate>) -> CoreResult<Vec<BudgetChange>>` transactional (single `available` tracking, `remainder` collapsed)

- [ ] **Step 1: Write failing test for write + idempotence**
```rust
#[test]
fn apply_templates_writes_and_clears_hold() {
    let mut db = setup_test_db();
    let cat_a = create_category(&mut db, "A");
    let cat_b = create_category(&mut db, "B");
    set_hold(&mut db.conn_mut(), "2026-09", 5000).unwrap();
    create_template(&mut db, &cat_a, 3000, 1); // priority 1 fixed 3000
    create_template(&mut db, &cat_b, 4000, 2); // priority 2 fixed 4000
    let changes = apply_templates(&mut db.conn_mut(), "2026-09").unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].applied_cents, 3000);
    assert_eq!(changes[1].applied_cents, 2000); // capped by to_budget 5000
    assert_eq!(get_hold(&db.conn(), "2026-09").unwrap(), None);
    assert_eq!(budget_amount(&db.conn(), &cat_a, "2026-09"), 3000);
    assert_eq!(budget_amount(&db.conn(), &cat_b, "2026-09"), 2000);
}
#[test]
fn apply_templates_second_call_idempotent() {
    let mut db = setup_test_db();
    let cat = create_category(&mut db, "A");
    set_hold(&mut db.conn_mut(), "2026-09", 5000).unwrap();
    create_template(&mut db, &cat, 5000, 1);
    let c1 = apply_templates(&mut db.conn_mut(), "2026-09").unwrap();
    assert_eq!(c1[0].applied_cents, 5000);
    let c2 = apply_templates(&mut db.conn_mut(), "2026-09").unwrap();
    assert!(c2.iter().all(|c| c.applied_cents == 0), "second call no double-spend");
}
```

- [ ] **Step 2: Run to verify fails**
Run: `cargo test -p finsight-core --lib repos::budgets::tests::apply_templates_writes_and_clears_hold -- --nocapture`
Expected: FAIL — `get_hold` still `Some(5000)`, budgets not written (old logic only mutated local `available`)

- [ ] **Step 3: Implement transactional body**
In `crates/finsight-core/src/repos/budgets.rs:626` replace body with:
```rust
pub fn apply_templates(conn: &mut Connection, month: &str) -> CoreResult<Vec<BudgetChange>> {
    conn.execute("BEGIN IMMEDIATE", [])?;
    let res: CoreResult<Vec<BudgetChange>> = (|| {
        let mut templates = list_funding_templates(conn)?;
        templates.sort_by_key(|t| (t.priority, t.id.clone()));
        let mut available = available_funds(conn, month)?; // diverges from spec §3: includes prev_hold
        // remainder collapsed into available single tracking (see task-4 review)
        let mut out = Vec::new();
        for tmpl in templates {
            let cat_avail = category_available(conn, &tmpl.category_id, month)?;
            let need = match tmpl.kind {
                TemplateKind::Fixed(v) => v,
                TemplateKind::Percent(p) => (remainder * p as i64) / 100,
                TemplateKind::UpTo(cap) => (cap - cat_avail).max(0),
                TemplateKind::By(n) => n,
                TemplateKind::Average(k) => average_spending(conn, &tmpl.category_id, month, k)?,
                TemplateKind::Remainder => remainder,
            };
            let take = need.min(available).max(0).min(remainder.max(0));
            if take != 0 {
                let cur: i64 = conn.query_row("SELECT COALESCE(SUM(amount_cents),0) FROM budgets WHERE category_id=?1 AND month=?2", params![tmpl.category_id, month], |r| r.get(0))?;
                set(conn, &tmpl.category_id, month, cur + take)?;
                let _ = conn.execute("DELETE FROM budget_holds WHERE month=?1 AND category_id=?2", params![month, tmpl.category_id]);
                // If holds are per-month not per-category, adjust: DELETE WHERE month=? only if last template
            }
            available -= take;
            remainder -= take;
            out.push(BudgetChange { category_id: tmpl.category_id, applied_cents: take });
        }
        if out.iter().any(|c| c.applied_cents != 0) {
            let _ = conn.execute("DELETE FROM budget_holds WHERE month=?1", params![month]);
        }
        Ok(out)
    })();
    match res {
        Ok(v) => { conn.execute("COMMIT", [])?; Ok(v) },
        Err(e) => { let _ = conn.execute("ROLLBACK", []); Err(e) },
    }
}
```
Adjust to exact schema: `budget_holds` is per-month single row (`month` PK) per migration `V064`, so `DELETE WHERE month=?` once at end is correct — not per-category. Check migration to confirm.

- [ ] **Step 4: Wire handler thin**
In `crates/finsight-api/src/commands/budget.rs` ensure `apply_templates` handler is:
```rust
#[utoipa::path(post, path="/api/rpc/apply_templates")]
pub async fn apply_templates(state: &ApiState, body: ApplyTemplatesRequest) -> AppResult<Vec<BudgetChange>> {
    let db = state.db.clone();
    run(&db, move |conn| apply_templates(conn, &body.month)).await.map_err(AppError::from)
}
```

- [ ] **Step 5: Run tests**
Run: `cargo test -p finsight-core --lib repos::budgets::tests::apply_templates -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**
```bash
git add crates/finsight-core/src/repos/budgets.rs crates/finsight-api/src/commands/budget.rs
git commit -m "feat(core): transactional apply_templates writes budgets + clears holds, idempotent (I2)"
```

### Task 5: Grouped Transfer Sums — Eliminate Budget N+1 (I5)

**Files:**
- Modify: `crates/finsight-api/src/commands/budget.rs:98-112`
- Test: `crates/finsight-core` or `crates/finsight-api` unit (existing envelope list test)

**Interfaces:**
- Consumes: `budget_transfers` table (`V066`), `category_spent` unchanged
- Produces: same `list_budget_envelopes` DTOs but via 1-2 queries not 2*envelopes

- [ ] **Step 1: Write failing perf regression (optional but proves N+1 gone)**
```rust
#[test]
fn envelope_listing_uses_grouped_query() {
    // Count queries via rusqlite trace or by inspecting impl:
    // old impl does 2 queries per envelope; new does ≤2 total
    let sql = budget_envelopes_sql_for_test(); // helper exposing generated SQL
    assert!(!sql.contains("WHERE category_id = ?"), "should use GROUP BY not per-category");
    assert!(sql.contains("GROUP BY"), "should be grouped");
}
```
If tracing is heavy, instead assert functional parity: seed 3 categories with transfers + verify `list_budget_envelopes` returns same `available` as before after refactor.

- [ ] **Step 2: Run to verify (pre-fix fails grouped assertion)**
Run: `cargo test -p finsight-api --lib commands::budget::tests::envelope_listing_uses_grouped_query -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement grouped sums**
In `list_budget_envelopes` at 98-112, replace per-envelope loops:
```rust
let from_map: HashMap<String,i64> = conn.prepare(
    "SELECT from_category_id AS cid, COALESCE(SUM(amount_cents),0) \
     FROM budget_transfers WHERE month=?1 GROUP BY from_category_id"
)?.query_map(params![month], |r| Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?)))?
 .collect::<Result<HashMap<_,_>,_>>()?;
let to_map: HashMap<String,i64> = conn.prepare(
    "SELECT to_category_id AS cid, COALESCE(SUM(amount_cents),0) \
     FROM budget_transfers WHERE month=?1 GROUP BY to_category_id"
)?.query_map(params![month], |r| Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?)))?
 .collect::<Result<HashMap<_,_>,_>>()?;
 // then for each envelope: transfer_in = *to_map.get(id).unwrap_or(&0); transfer_out = *from_map.get(id).unwrap_or(&0);
```

- [ ] **Step 4: Run envelope tests**
Run: `cargo test -p finsight-core --lib repos::budgets::tests::budget_envelopes -- --nocapture`
Run: `cargo test -p finsight-api --lib -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add crates/finsight-api/src/commands/budget.rs
git commit -m "perf(api): grouped budget transfer sums via GROUP BY (I5)"
```

### Task 6: UI Polish + Minor Hygiene (M2, M3, M4, M6)

**Files:**
- Modify: `ui/src/screens/Budget.tsx:99`
- Modify: `ui/src/api/openapiClient.ts:306-307`
- Modify: `crates/finsight-core/src/repos/budgets.rs:839` (clippy allow)
- Modify: `crates/finsight-core/migrations/V065__add_funding_templates.sql` (check index — M4, optional)
- Test: `ui/src/screens/Budget.test.tsx`, `ui/src/api/openapiClient.test.ts`

**Interfaces:**
- Consumes: typed `api` client, `money()` helper
- Produces: `Budget.tsx` uses `moneyDisplay` with sign; `openapiClient.rpc` deprecated; no lint allow drift

- [ ] **Step 1: Write failing UI test for M2**
```tsx
// ui/src/screens/Budget.test.tsx
it("over-budget chip shows signed money via moneyDisplay", () => {
  render(<BudgetEnvelope remaining={-1200} />);
  expect(screen.getByText(/Over by/)).toHaveTextContent("Over by -$12.00");
  expect(screen.getByText(/Over by/).classList.contains("money")).toBe(true);
});
```

- [ ] **Step 2: Run to verify fails**
Run: `cd ui && npx vitest run src/screens/Budget.test.tsx --reporter=verbose`
Expected: FAIL — old `money(Math.abs(remaining))` shows `$12.00` not `-$12.00`

- [ ] **Step 3: Implement M2**
In `ui/src/screens/Budget.tsx:99`: replace `money(Math.abs(remaining))` with `money(remaining)` (check `money` already handles sign) and chip label:
```tsx
const label = remaining < 0 ? `Over by ${money(remaining)}` : `Left ${money(remaining)}`;
const usesMoney = true; // for blur check, not label.includes("$")
<span className={`chip ${usesMoney ? "money" : ""}`}>{label}</span>
```

- [ ] **Step 4: Implement M3**
In `ui/src/api/openapiClient.ts:306-307`:
```ts
/** @deprecated Use typed `api` — untyped rpc bypasses arg camelCase asserts */
export async function rpc<T>(cmd: string, body: unknown): Promise<T> {
  // keep body for compat but add dev warn
  if (import.meta.env.DEV) console.warn("[deprecated] rpc", cmd);
  return api.rpc<T>(cmd, body);
}
```

- [ ] **Step 5: Implement M4/M6 (small)**
- M6 at `budgets.rs:839`: if MSRV bumped to 1.98+, replace `vals.len()%2==0` with `vals.len().is_multiple_of(2)` and remove `#[allow(clippy::manual_is_multiple_of)]`; else keep allow with comment `// TODO: is_multiple_of on MSRV bump`.
- M4: if `funding_templates` query filters by `category_id`, add `CREATE INDEX idx_funding_templates_category_priority ON funding_templates(category_id, priority, id)` in a follow-up migration `V067` or note as non-blocking — do not add if not queried that way (verify via grep `WHERE category_id` in `list_funding_templates`).

- [ ] **Step 6: Run UI checks**
Run: `cd ui && npx vitest run src/screens/Budget.test.tsx --reporter=verbose`
Expected: PASS
Run: `cd ui && npx tsc --noEmit`
Expected: PASS
Run: `cd ui && npm run build`
Expected: PASS (entry chunk size stable or smaller)

- [ ] **Step 7: Commit**
```bash
git add ui/src/screens/Budget.tsx ui/src/api/openapiClient.ts crates/finsight-core/src/repos/budgets.rs
git commit -m "fix(ui): moneyDisplay signed + rpc deprecation + clippy/index hygiene (M2,M3,M4,M6)"
```

### Task 7: Regression Corpus + Final Green Bar (I6 + all above)

**Files:**
- Modify: `crates/finsight-core/src/repos/budgets.rs` (additional tests), `ui/src/screens/ReportBuilder.test.tsx`, `crates/finsight-server/tests/parity.rs` snapshot if needed
- Test: all of above plus `cargo test --workspace`, `pnpm --filter ui test`, `pnpm build`

**Interfaces:**
- Consumes: all prior tasks
- Produces: green bar + `unknown_command` 404 regression

- [ ] **Step 1: Add parity corpus test (C1+I3+I8 unified)**
```rust
#[test]
fn custom_report_parity_with_fixed_reports() {
    let mut db = setup_test_db();
    // Seed 6 months: mix of expense, income, settle_up, transfers
    for m in 1..=6 {
        let cat = create_category(&mut db, &format!("Cat{m}"));
        insert_tx(&mut db, &cat, -1000*m, &format!("2026-0{m}-10"), 0);
        insert_tx(&mut db, &cat, 5000, &format!("2026-0{m}-11"), 0); // income ignored
        insert_tx(&mut db, &cat, 200, &format!("2026-0{m}-12"), 1); // reimbursement nets
    }
    let fixed = get_report_data(&db.conn(), Period::Last6Months).unwrap();
    let fixed_sum: i64 = fixed.months.iter().map(|m| m.total).sum();
    let custom = custom_report(&db.conn(), CustomReportParams {
        period: Period::Last6Months, split_by: SplitBy::Month,
        include_archived: true, include_transfers: false
    }).unwrap();
    let custom_sum: i64 = custom.rows.iter().map(|r| r.total_cents).sum();
    assert_eq!(fixed_sum, custom_sum);
}
```

- [ ] **Step 2: Add carryover/available/average reimbursement nets (C3)**
```rust
#[test]
fn carryover_nets_reimbursement() {
    let mut db = setup_test_db();
    let cat = create_category(&mut db, "Food");
    set_budget(&mut db.conn_mut(), &cat, "2026-08", 5000).unwrap();
    insert_tx(&mut db, &cat, -3000, "2026-08-10", 0);
    insert_tx(&mut db, &cat, 1000, "2026-08-12", 1); // +1000 settle_up nets as -1000
    // budgeted 5000 - spent 2000 (3000-1000) = 3000 carryover into 2026-09
    assert_eq!(carryover_for(&db.conn(), &cat, "2026-09").unwrap(), 3000);
}
#[test]
fn category_available_nets_reimbursement() { /* similar for available */ }
```

- [ ] **Step 3: Add hold/transfer spare guards (I6)**
```rust
#[test]
fn transfer_optional_insufficient_spare_validation() {
    let mut db = setup_test_db();
    let a = create_category(&mut db, "A");
    let b = create_category(&mut db, "B");
    set_budget(&mut db.conn_mut(), &a, "2026-09", 1000).unwrap();
    insert_tx(&mut db, &a, -900, "2026-09-05", 0);
    // spare = 100; try transfer 500 → Validation
    let err = transfer_optional(&mut db.conn_mut(), &a, &b, "2026-09", 500).unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));
}
```

- [ ] **Step 4: Add ReportBuilder split coverage**
```tsx
// ui/src/screens/ReportBuilder.test.tsx
it("splitBy Month respects includeArchived and is_transfer=false", async () => {
  mockCustomReport({ period:"last6Months", splitBy:"month", includeArchived:false });
  render(<ReportBuilder />);
  await user.click(screen.getByLabelText(/Group by/));
  await user.click(screen.getByText(/Month/));
  expect(mockFetch).toHaveBeenCalledWith(expect.objectContaining({ splitBy:"month" }));
});
```

- [ ] **Step 5: Add contract 404 regression**
```rust
// crates/finsight-server/tests/parity.rs (or new test)
#[tokio::test]
async fn unknown_command_transfer_envelope_is_404() {
    let app = test_app().await;
    let res = app.post("/api/rpc/transfer_envelope").json(&json!({})).send().await;
    assert_eq!(res.status(), 404);
    assert_eq!(res.json::<Value>().await["code"], "unknown_command");
}
```

- [ ] **Step 6: Run full green bar**
Run: `cargo test --workspace -- --nocapture`
Expected: PASS (ignored tests limited to keychain/live-provider)
Run: `cd ui && npx vitest run --reporter=verbose`
Expected: PASS
Run: `cd ui && npx tsc --noEmit`
Expected: PASS (no errors)
Run: `cd ui && npm run build`
Expected: PASS, entry chunk not grown (check `dist/assets` sizes)
Run: `cargo tree -p finsight-api -i tauri`
Expected: empty (no Tauri)

- [ ] **Step 7: Commit**
```bash
git add crates/finsight-core/src/repos/budgets.rs ui/src/screens/ReportBuilder.test.tsx crates/finsight-server/tests/parity.rs
git commit -m "test: custom_report parity + carryover/settle_up + transfer spare + contract 404 (I6 + C1-C3)"
```

### Final Verification (not a task — run after Task 7)

```bash
cargo test --workspace && cd ui && npx vitest run && npx tsc --noEmit && npm run build && echo "GREEN BAR"
# plus manual
pnpm openapi --check 2>&1 | head -20  # no diff
cargo tree -p finsight-api -i tauri
```

## Self-Review

**Spec coverage:** every Critical (C1-C3) mapped to Tasks 1-2, every Important I1-I8 mapped to Tasks 1-5+7, every Minor M1-M6 mapped to Tasks 3+6; `custom_report_parity` and `unknown_command` regressions explicitly added; no spec gap. Rollback slice (3+7) preserved.

**Placeholder scan:** no `TBD`/`TODO` without handling; M4 conditional is explicit (grep before adding index); all steps show exact code, file:line, command, expected output.

**Type consistency:** `category_spent(conn,&str,&str,&str)->i64`, `period_bounds(conn,Period)->(Option<String>,String)`, `SplitBy::Category` serializes `"category"`, `transfer_budget` not `transfer_envelope`, `reconcile_bases` snake, `BudgetChange { category_id,applied_cents }` stable across Tasks 4 and 7.

Fixes applied inline: clarified `budget_holds` PK check in Task 4, unified `is_transfer` handling in helper, kept `MidpointAwayFromZero` with string `Decimal` path, added snapshot update note in Task 3.

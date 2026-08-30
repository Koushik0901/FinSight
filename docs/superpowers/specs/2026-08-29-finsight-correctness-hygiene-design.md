# FinSight Correctness & Hygiene — Big-Bang Fix (C1-C3, I1-I8, M1-M6)

**Date:** 2026-08-29  
**Base:** `dd68366 chore: remove unused utoipa::ToSchema imports` (plus `b36cc27 fix(money)`)  
**Scope:** Single atomic PR, 7 commits, one `pnpm openapi` regen. No schema migration beyond V067 unless needed; money helpers only.
**Decisions locked:** Q1=A include all minors, Q2=A custom_report expense-only (`ELSE 0` + `settle_up` net), Q3=A `apply_templates` transactional write, Q4=A stub gated `#[cfg(test)]`, Q5=A hard rename no alias.
**Approach:** Approach 1 Big Bang (7 ordered commits, one green bar).

## 1. Overview & Scope

### Goal
Ship a single correct sweep that makes Custom Reports reconcile with fixed Reports, makes carryover/available drift-free on `settle_up`, removes a production test double, and normalizes the RPC surface before the next release.

### In Scope (verified file:line)

- **Core money:** `crates/finsight-core/src/repos/budgets.rs:928-929` (C1), `389-443` + `852-856` (C3), `884-953` (I3/I8), new `category_spent` helper (fixes all four)
- **Providers:** `crates/finsight-providers/src/enable_banking/sync.rs:31-62,174-185` (C2 stub, I7 rounding)
- **Contract:** `crates/finsight-openapi/src/lib.rs:182,266-268` + `crates/finsight-api/src/commands/budget.rs:457-475` + `crates/finsight-server/src/dispatch.rs:134,228` (I1 `reconcileBases`→`reconcile_bases`, I4 `transfer_envelope` collapse, M5 comment) + `crates/finsight-core/src/repos/budgets.rs:46-55` (M1 SplitBy casing)
- **Writes/perf:** `crates/finsight-core/src/repos/budgets.rs:626-699` (I2 transactional `apply_templates`), `crates/finsight-api/src/commands/budget.rs:98-112` (I5 N+1)
- **Frontend/transport:** `ui/src/screens/Budget.tsx:99` + `ui/src/api/openapiClient.ts:306-307` (M2/M3)
- **Tests:** I6 new corpus + `custom_report_parity_with_fixed_reports`; one `pnpm openapi` regen (`openapi.json` + `ui/src/api/openapi.json` + `ui/src/api/openapi.ts`)

### Out of Scope
New report dimensions, `includeIncome` toggle (deferred), SimpleFIN behavior, Copilot block changes, migrations beyond V067 if alias was persisted (it was not).

### Success Criteria
`cargo test --workspace` (incl. `parity.rs`/`mcp.rs`), `pnpm --filter ui test`, `pnpm typecheck`, `pnpm build` all green; `custom_report(split=Category/Month/Account)` totals equal `get_report_data` monthly totals on a fixture with `settle_up` reimbursements; `carryover_for("2026-09")` matches `budgeted - spent` with reimbursements; non-test `token-a` hits network error not stub; `COMMANDS` sorted snake_case `windows(2)` test passes; `apply_templates` second call idempotent; `POST /api/rpc/transfer_envelope` returns `404 unknown_command`.

## 2. Architecture & Component Boundaries

### Principle
Each fix stays in its owning crate; no cross-crate SQL, no hand-rolled money math in handlers.

### `finsight-core` — sole money owner
- New pure helper `category_spent(conn, category_id, from: &str, to: &str) -> CoreResult<i64>` at `repos/budgets.rs:380-390` (before `carryover_for`). Canonical expense expression:
  ```rust
  SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents
           WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END)
  ```
  Filter: `WHERE t.category_id=? AND t.posted_at >= ? AND t.posted_at < ? AND t.is_transfer=0`. No other file may compute spent.
- `carryover_for`, `category_available`, `look_back_facts:average_spending`, `custom_report` SQL call `category_spent` or reuse its WHERE fragment via shared `period_bounds` helper. `available`/`to_budget` remain computed, not stored.
- New `period_bounds(period: Period, anchor: NaiveDate) -> (Option<String>, String)` — anchor = `MAX(posted_at)` `YYYY-MM-DD` (fallback `Utc::now` if no rows), returns RFC3339 `start` + `end=anchor+1d`. Shared by `reports::get_report_data` and `budgets::custom_report` (fixes I3/I8). Truncates to day boundary `YYYY-MM-DD`.
- `apply_templates` transactional: `BEGIN IMMEDIATE` → loop `category_available` → `take = need.min(available).max(0)` → `INSERT INTO budgets ... ON CONFLICT(category_id,month) DO UPDATE SET amount_cents = excluded.amount_cents` (or existing `set_budget` helper) + `DELETE FROM budget_holds WHERE month=?` per touched category → `COMMIT`. On failure `ROLLBACK`.
- `SplitBy` gets `#[serde(rename_all="camelCase")] #[schema(rename_all="camelCase")]` so `Category` → `"category"` matches `Period` casing (M1).

### `finsight-providers` — no prod test doubles
- Delete literal `match token { "token-a" => ... "token-b" => ... }` from prod `fetch_enable_data` (`sync.rs:31-62`). Prod:
  ```rust
  pub async fn fetch_enable_data(token:&str)->ProviderResult<EnableBankingSyncData> {
    let client = EnableBankingClient::new(token)?; client.list_accounts().await.map(|accounts| EnableBankingSyncData{accounts, transactions: vec![]})
  }
  ```
  Test helper `fetch_enable_data_stub_for_test` behind `#[cfg(test)]` if ever needed; wiremock `fetch_enable_data_with_base_url_isolates_via_http` becomes sole isolation proof (C2).
- `parse_amount_cents` at `sync.rs:174-185` switches to `decimal.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)` (fixes I7). Add `12.345→1235` regression.

### `finsight-api` — transport-agnostic handlers
- Correct `#[utoipa::path(post, path="/api/rpc/reconcile_bases")]` (was `reconcileBases`), delete `transfer_envelope` `#[utoipa::path]` — keep only `transfer_budget` (`commands/budget.rs:457-475`). Stays Tauri-free (`cargo tree -p finsight-api -i tauri` empty).
- `apply_templates` handler thin: `run(&db, move |conn| apply_templates_tx(conn, month, templates))`.

### `finsight-openapi` + `finsight-server` — single contract regen
- `COMMANDS` at `lib.rs:182` `"reconcileBases"`→`"reconcile_bases"`, delete `"transfer_envelope"` at `lib.rs:235`, keep only `transfer_budget` in `#[openapi(paths(...))]` at `lib.rs:266-268`. `COMMANDS` stays sorted snake_case — add test `windows(2).all(|w| w[0] < w[1]) && all snake` (I1).
- `dispatch.rs:228` arm updated, delete `transfer_envelope` arm; stale `dispatch.rs:134` bindings comment → "`finsight-openapi` + `ui/src/api/openapi.ts` regen" (M5).
- One `pnpm openapi` (`cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen`) bumps `openapi.json`/`ui/src/api/openapi.json`/`ui/src/api/openapi.ts`.

### `ui` — no new routes, tiny edits
- `Budget.tsx:99` `money(Math.abs(remaining))` → `moneyDisplay(remaining,{includeSign:true})`; chip label uses the same display string (M2).
- `openapiClient.ts:306-307` `rpc<T>` marked `/** @deprecated use typed api */` or gated `if cfg!(debug_assertions)` (M3). `App.tsx` stays lazy-only.

## 3. Data Flow & Invariants

### Custom Report Parity (C1+I3+I8)
1. Caller `POST /api/rpc/custom_report` with `CustomReportParams { period, splitBy, includeArchived, includeTransfers }` (typed via `openapiClient.ts` → `arg(&p,"period")` camelCase contract).
2. Handler `finsight-api/src/commands/reports.rs:custom_report` calls `run(&db, |conn| { let (start,end) = period_bounds(period, anchor_max_posted_at(conn)?); custom_report_rows(conn, params, start, end) })`.
3. `custom_report_rows` builds one SQL: `SELECT {label} AS label, SUM(CASE WHEN t.settle_up=1 THEN -t.amount_cents WHEN t.amount_cents<0 THEN -t.amount_cents ELSE 0 END) AS total ... FROM transactions t {join} WHERE 1=1 AND t.posted_at >= ? AND t.posted_at < ?` plus `is_transfer` / `archived_at` branches per `split_by`. End bound fixes I8.
4. `period_bounds` anchor: `SELECT MAX(date(posted_at)) FROM transactions` → `NaiveDate` (fallback `Utc::now().date_naive()`). `Last1Month` = anchor minus 1 month inclusive `>= anchor-1mo 00:00`. Same helper called by `reports::get_report_data` (replace wall-clock calc) — I3 fixed.
5. Invariant: for same `period` + `is_transfer=0`, `Σ custom_report(split=Month).total == Σ get_report_data.monthly.totals` on any fixture with reimbursements.

### Carryover / Available Drift Fix (C3)
- `carryover_for(cat, month)` = `budgeted SUM(budgets WHERE month ∈ [first_budgeted, month))` minus `category_spent(conn, cat, start_date, month_date)`.
- `category_available(cat, month)` = `budgeted_this_month + carryover_for - category_spent(cat, month_start, next_month_start)`.
- `look_back_facts` `average_spending` also calls `category_spent` per look-back month.
- Drift invariant: reimbursement `+2000,settle_up=1` reduces both `envelope.spent` and `carryover_for` by 2000.

### `apply_templates` Transactional Write (I2)
Input `Vec<FundingTemplate>` ordered `priority ASC, id ASC`. Handler resolves `available = available_funds(month)` (`income - budgeted - hold_current + hold_prev` — diverges from spec §3 Global Constraints `to_budget` intentionally; `remainder` collapsed into `available` single tracking). Inside `BEGIN IMMEDIATE`:
- For each `tmpl` in order: `need = match kind { Fixed(v)=>v, Percent(p)=>p%*remainder, UpTo(cap)=> (cap - category_available(cat,month)).max(0), By(n)=>n, Average(k)=>avg(k), Remainder=>remainder }`
- `take = need.min(available).max(0);` `budgets.set(cat, month, budgeted+take)`; `available -= take;` `DELETE FROM budget_holds WHERE month=?` if `take>0` (holds are per-month).
- `COMMIT` → `Vec<BudgetChange { categoryId, appliedCents }>`. Second call with `available==0` yields `take==0` — idempotent.

### Enable Banking Isolation (C2+I7)
`fetch_enable_data(token)` → `EnableBankingClient::new(token)` → `GET {base}/accounts` with `Authorization: Bearer {token}`. No literal match — `token-a` hits network and fails in prod if not real (correct). `parse_amount_cents("12.345")` → `1235` via `MidpointAwayFromZero`.

## 4. Error Handling, Delivery Perf & Security

### Error Handling
- Core `category_spent` / `period_bounds` return `CoreResult`; `rusqlite` → `CoreError::Db` → `AppError::from` → `500 {code:"db_error"}`. `MAX(posted_at)` null → fallback anchor; malformed `YYYY-MM` → `CoreError::Validation("invalid month")` → `400`.
- `custom_report` / `apply_templates` use `run(&db, move |conn| …).await.map_err(AppError::from)`. `apply_templates` explicit `ROLLBACK` on inner `Err`; `COMMIT` failure → `500`. Validation (`Percent>100`, over-spare) → `422`.
- Enable Banking: empty token or `401` → `ProviderError::Auth` → `401/502`; invalid amount → `ProviderError::Internal("invalid amount: …")` → `400`.
- Deleting `transfer_envelope` / renaming `reconcileBases` → `404 {code:"unknown_command"}` via existing not-found path. Untyped `rpc` callers get TS deprecation, not runtime.
- Frontend keeps `useQuery` error states; empty `periodBounds` → `stub` not spinner.

### Delivery Performance (pinned in `router.rs` tests — must not regress)
- Compression per-route, never `Router::layer`. `/api/events` stays uncompressed (`sse_event_stream_is_never_compressed` must stay green). `custom_report` inherits RPC compression.
- Cache policies unchanged — `openapi.json` bump is content-hashed via `openapi.ts`.
- Entry bundle lean: only `Budget.tsx:99` edit + `openapiClient.ts` comment (no dep). `transfer_envelope` deletion reduces `openapi.ts` ~0.5kB. I5 N+1 fix reduces Budget paint DB trips from `2*envelopes` scalars to one `GROUP BY category_id` CTE (two grouped queries max).

### Security Invariants (no relaxation)
- `custom_report` SQL uses static fragments (`match split_by` constants) + `?` binds — no injection.
- PAT/OAuth scopes, Argon2id, SQLCipher, `Origin` vs `FINSIGHT_PUBLIC_ORIGIN`, `403 insufficient_scope`, refresh rotation, `mcp:<tokenId>` provenance — untouched; `reconcile_bases`/`transfer_budget` keep existing scopes — rename does not widen scope.
- `EnableBankingClient` uses Bearer header only; removing stub eliminates trivial bearer guess bypass.
- No new `settings` keys, no `FINSIGHT_DATA_DIR` change; `BEGIN IMMEDIATE` holds <10 rows, no deadlock (pool `max_size=8,min_idle=0` unchanged).

## 5. Testing, Rollout & Verification

### Tests

**Core unit (`cargo test -p finsight-core`):**
- `custom_report_expense_only` — seed `-5000` expense, `+8000` income, `+2000 settle_up`; assert `total==3000` not `13000` (C1).
- `custom_report_parity_with_fixed_reports` — seed 12 months mixed, assert Σ equal between `get_report_data` monthly and `custom_report(split=Month)` on same anchor (C1+I3+I8).
- `custom_report_excludes_future_rows` — `now+2d` excluded (I8).
- `category_spent_settle_up_nets`, `carryover_nets_reimbursement`, `category_available_nets_reimbursement`, `average_spending_nets_reimbursement` (C3).
- `apply_templates_writes_and_clears_hold` + second-call idempotent (I2).
- `transfer_optional_spare_validation` + concurrent no-overdraft via `BEGIN IMMEDIATE`.
- `hold_park_reappear`, `available_funds_month_boundary`.
- `enable_parsing_midpoint_away` — `12.345==1235`, `-12.345==-1235` (I7); `enable_stub_removed_hits_network`.
- `commands_sorted_snake` + `transfer_envelope_absent_from_openapi` + `SplitBy camelCase` (I1,M1).

**OpenAPI/server (`cargo test -p finsight-openapi`, `cargo test -p finsight-server --test parity`, `mcp`):**
- `parity.rs` + snapshot must pass after rename & deletion.
- `mcp.rs` sort + `403 insufficient_scope` unchanged.

**UI (`pnpm --filter ui test`, `pnpm typecheck`):**
- `ReportBuilder.test.tsx` — `splitBy=Month/Account` archiv filtering, `include_archived` toggle, empty → stub; `Budget.tsx` chip `moneyDisplay`.
- Typed `transferBudget` still works, `transferEnvelope` hook deleted.

### Rollout — 7 Ordered Commits (Approach 1 Big Bang)
1. `fix(core): category_spent + period_bounds` (C1,C3,I3,I8)
2. `fix(providers): remove token-a/b stub + MidpointAwayFromZero` (C2,I7)
3. `fix(openapi): rename reconcile_bases, drop transfer_envelope, SplitBy camelCase` + `pnpm openapi` (I1,I4,M1,M5)
4. `feat(core): transactional apply_templates` (I2)
5. `perf(api): grouped budget transfer sums` (I5)
6. `fix(ui): moneyDisplay + rpc deprecation` (M2,M3) + `chore` M4/M6
7. `test: parity + budget/provider regressions` (I6 + all above)

**Gate:** `cargo test --workspace` (live-provider ignored only), `pnpm --filter ui test`, `pnpm typecheck`, `pnpm build`, `cargo tree -p finsight-api -i tauri` empty, `pnpm openapi` no unstaged diff.

**Rollback:** revert commits 3+7 only (contract slice) — core money fixes (1,2) ship independently.

## 6. OpenAPI & Dispatch Contract

- Maintain AGENTS.md checklist for any future command: `finsight-api` body `#[utoipa::path]` + DTO `ToSchema` → `finsight-openapi::COMMANDS` sorted snake → `finsight-server::dispatch::rpc_routes!` `arg(&p,"camelCase")` → `pnpm openapi`.
- This spec's contract change is exactly: `reconcileBases→reconcile_bases` (hard rename), `transfer_envelope` deleted, `SplitBy` enum values `Category→category` etc. No new command added; `openapi.json` path count decreases by 1.

## 7. Risks & Mitigations

- **External caller break on rename** → hard 404 with `unknown_command` code; rollback slice is commits 3+7 only.
- **Budget write double-spend** → `BEGIN IMMEDIATE` + idempotent second-call test.
- **Carryover divergence reintroduces** → single helper `category_spent` is only allowed `spent` implementation; grep CI for `amount_cents < 0` without `settle_up` should yield zero hits after this PR.
- **Bankers rounding drift** → `MidpointAwayFromZero` with string `Decimal` path, no float.

## 8. Alternatives Considered

- **Two-phase PRs** — smaller review but I3/I8 would ship inconsistent anchor between PRs; two `openapi` regens; deferred hygiene.
- **Feature-flagged helpers** — zero-downtime toggle but double code paths and flag debt for pure correctness fixes; violates single-source money math invariant.

Chosen: Big Bang for coupled money helper (one helper fixes 3 issues), single breaking contract regen already accepted.


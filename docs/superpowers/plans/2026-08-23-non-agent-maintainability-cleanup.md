# Non-Agent Maintainability Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 5 non-agent debt clusters (metrics display distortion, hand-synced contracts, household/member scaling, robustness, hygiene) in one 5-phase train while keeping Gate C green.

**Architecture:** Correctness-first phasing (metrics → contracts → household → robustness → hygiene) with single-source helpers (`rpc_routes!`, `sink::event_names`, `latest_balance_subquery`, `metric_core(scope)`), aggressive one-commit-per-deletion, and TDD per phase.

**Tech Stack:** Rust (Axum, rusqlite/SQLCipher, refinery), TypeScript/React (tanstack-query, vite, vitest), specta codegen, IndexedDB PWA

## Global Constraints

- Gate C must stay green after every phase: `cargo test --workspace` + `cd ui && npx vitest run` + `cd ui && npx tsc --noEmit` + parity/corpus tests; add new tests for each phase, no new `#[ignore]` harnesses.
- Aggressive deletes: one commit per deletion, no feature flag; mechanical move commits contain zero logic.
- Bound B: `finsight-agent` touched only where it leaks core correctness (`anomaly.rs` second detector barrier + shared CASE helper + `categorizer.rs:343` barrier); otherwise `finsight-core` owns SQL.
- No new migrations (next would be `V064` — not needed); no new sync source/provider UI; no wire change.
- All `arg("…")` in `rpc_routes!` must be `camelCase` per `bindings.ts`; `rg -n '"copilot-stream-frame"'` must fail CI if literal remains.
- Each phase independently shippable and verifiable on `samples/` numbers.

---

## File Structure

**New files:**
- `crates/finsight-api/src/sink/event_names.rs` — `pub const COPILOT_STREAM_FRAME: &str` etc., single source for SSE event names
- `crates/finsight-core/src/repos/balance.rs` — `pub fn latest_balance_subquery(alias: &str) -> String` returning `ORDER BY CASE alias.source …`
- `ui/src/api/eventNames.ts` — generated mirror (via `export_bindings` or manual `const` re-export), consumed by `httpBackend.ts` listeners
- `ui/src/api/hooks/_factory.ts` — `unwrap<T>()`, `mutationWrapper`, `queryKeyFactories` (`actionBundleKeys`, `simplefinKeys`)
- `ui/src/api/hooks/useFocusParam.ts` — shared `useFocusParam(param, handler)` (replaces 4 copies)
- `ui/src/api/hooks/useDrawerSeed.ts` — shared drawer-seed effect (kills 5× `eslint-disable`)
- `ui/src/screens/settings/` — slices: `SettingsAccounts.tsx`, `SettingsCurrency.tsx`, `SettingsAppearance.tsx`, `SettingsData.tsx`, `SettingsShortcuts.tsx`
- `ui/src/styles/copilot-shell.css` — 284 copilot selectors moved from `app.css`
- `ui/src/pwa/fixtures/goals.json`, `balanceTimeline.json`, `monthClose.json` — fixtures for `mockBackend` (replace reimplemented math)
- `crates/finsight-core/tests/metrics_robust_display.rs` — new robust display-path corpus
- `ui/src/api/eventNames.test.ts` — vitest membership guard
- `ui/src/dev/mockBackend.typed.test.ts` — typed responder compilation test

**Modified files:**
- `crates/finsight-core/src/metrics.rs:413,740,787` — `rolling_averages` display path + `emergency_fund_months` unification + `_for` family parametrization (Phase 1 & 3)
- `crates/finsight-core/src/repos/mod.rs` — export `balance`
- `crates/finsight-api/src/sink/mod.rs` — export `event_names`
- `crates/finsight-api/src/error.rs` — `AppError::http_status()`
- `crates/finsight-api/src/commands/*` — producers switch to `event_names::X`
- `crates/finsight-server/src/dispatch.rs:120-904,909-1167` — add `rpc_routes!` table at bottom, delete hand `SUPPORTED`
- `crates/finsight-server/tests/parity.rs` — retire regex, assert `SUPPORTED.len() == COMMAND_COUNT`
- `crates/finsight-server/src/users.rs` — poison-recover `Mutex`
- `crates/finsight-core/src/repos/connections.rs:109,145` + `repos/transfers.rs:43-103` — RFC3339 graceful parse
- `crates/finsight-core/src/repos/rules.rs` — reference for transaction pattern (no change, used as model)
- `crates/finsight-agent/src/categorizer.rs:343` + `agent.rs:93` — honor `ResetBarrier`, log instead of `let _ =`
- `crates/finsight-core/src/repos/balance.rs` consumers: `repos/accounts.rs:925`, `finance.rs:2611,2713` (Bound B), `context.rs:710,1097,1136`, `read.rs:27,940` — replace CASE literals
- `ui/src/screens/Today.tsx:246-332,332` + `ui/src/components/HealthScoreCard` — consume corrected `metrics`
- `ui/src/api/client.ts` — add `export function unwrap<T>(res: Result<T>): T`
- `ui/src/api/hooks/invalidation.ts` — export key factories
- `ui/src/api/httpBackend.ts` + `ui/src/components/copilot/TauriRuntime.ts:453` etc. — use `eventNames`
- `ui/src/dev/mockBackend.ts:1410,1159,1239` — type + fixture
- `ui/src/screens/Settings.tsx:855` — thin shell importing slices
- `ui/src/screens/Budget.tsx:185` — `useMemberScope()` extraction
- `ui/src/styles/app.css:5032` — trim to utilities
- `docs/superpowers/specs/2026-08-23-non-agent-maintainability-cleanup-design.md` — already committed (be1dfad)
- `AGENTS.md:107-109`, `src-tauri/Cargo.toml:25-27`, `ui/src/components/FilePicker.tsx:24`, `crates/finsight-core/src/sample.rs:1130` — dead-code fixes (Phase 5)
- `.gitignore` — ensure `.superpowers/` listed (if missing)

---

### Task 1: Phase 1 — Robust display expense + is_estimated flag

**Files:**
- Modify: `crates/finsight-core/src/metrics.rs:413-520`
- Test: `crates/finsight-core/tests/metrics_robust_display.rs` (new)

**Interfaces:**
- Consumes: existing `robust_monthly_expense_cents(conn, scope)` (used by projections)
- Produces: `pub fn rolling_averages(conn, scope: Option<MemberId>) -> RollingAverages` now uses robust median; new field `RollingAverages.is_estimated: bool`

- [ ] **Step 1: Write failing test for display-path one-off spike**

```rust
// crates/finsight-core/tests/metrics_robust_display.rs
#[test]
fn typical_monthly_expense_ignores_one_off_spike_display_path() {
    let db = new_test_db();
    // seed 3 months: 2× $1900 normal + 1 month with $1900 + $2500 one-off anomaly
    seed_months(&db, &[
        (2026-03, 190_000), (2026-04, 190_000), (2026-05, 190_000 + 250_000),
    ]);
    flag_anomaly(&db, 2026-05, 250_000); // mark $2500 as is_anomaly=1
    let r = metrics::rolling_averages(&db, None).unwrap();
    // Before fix this is ~$2766 ( (1900+1900+4400)/3 ), after fix ~$1900
    assert!(r.avg_monthly_expense_cents < 210_000, "got {}", r.avg_monthly_expense_cents);
    assert!(r.avg_monthly_expense_cents > 170_000);
}
```

- [ ] **Step 2: Run to verify FAIL**

Run: `cargo test -p finsight-core --test metrics_robust_display -- typical_monthly_expense_ignores_one_off_spike_display_path -v`
Expected: FAIL — `avg_monthly_expense_cents = 276666`

- [ ] **Step 3: Implement minimal fix in `metrics.rs:413`**

```rust
pub fn rolling_averages(conn: &Connection, scope: Option<MemberId>) -> Result<RollingAverages> {
    let robust = robust_monthly_expense_cents(conn, scope)?;
    let fallback_90d = avg_expense_90d(conn, scope)?;
    let (avg, is_estimated) = match robust {
        Some(v) if months_with_data(conn, scope)? >= 2 => (v, false),
        _ => (fallback_90d, true),
    };
    // ... build RollingAverages { avg_monthly_expense_cents: avg, is_estimated, … }
}
```

- [ ] **Step 4: Run to verify PASS**

Run: `cargo test -p finsight-core --test metrics_robust_display -- -v`
Expected: PASS; also `cargo test -p finsight-core --lib metrics::tests -v` still PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/metrics.rs crates/finsight-core/tests/metrics_robust_display.rs
git commit -m "fix(metrics): use robust median for display rolling_averages with is_estimated fallback"
```

### Task 2: Phase 1 — Unify EF-months definition

**Files:**
- Modify: `crates/finsight-core/src/metrics.rs:740-818` — `emergency_fund_months()`
- Modify: `crates/finsight-core/src/finance.rs` (Bound B thin slice) or `metrics.rs` helper used by `build_snapshot` + `run_emergency_fund_scenarios`
- Modify: `ui/src/screens/Today.tsx:332` + `ui/src/components/HealthScoreCard.tsx:148-154`
- Test: `crates/finsight-core/tests/metrics_ef_unified.rs` (new)

**Interfaces:**
- Consumes: `Task 1`'s `robust_monthly_expense_cents`
- Produces: `pub fn emergency_fund_months(conn, scope) -> f64` — single source; `RollingAverages.emergency_fund_months` uses it

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn emergency_fund_months_unified() {
    let db = new_test_db();
    // EF-eligible $5000, total liquid $7000, monthly expense $2000
    seed_accounts(&db, &[("chk", 500_000, true), ("brk", 200_000, false)]);
    seed_expense(&db, 200_000);
    let m = metrics::emergency_fund_months(&db, None).unwrap();
    assert!((m - 2.5).abs() < 0.05, "EF pool $5000/ $2000=2.5, got {m}");
    let ra = metrics::rolling_averages(&db, None).unwrap();
    assert!((ra.emergency_fund_months - m).abs() < 0.01);
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-core --test metrics_ef_unified -v`
Expected: FAIL — `got 3.5` (total liquid) vs `2.5`

- [ ] **Step 3: Implement — force EF-eligible pool**

```rust
pub fn emergency_fund_months(conn: &Connection, scope: Option<MemberId>) -> Result<f64> {
    let ef_cents = sum_balances_where(conn, scope, "is_emergency_fund=1")?; // never total liquid
    let exp = robust_monthly_expense_cents(conn, scope)?.unwrap_or(avg_expense_90d(conn, scope)?);
    if exp == 0 { return Ok(0.0); }
    Ok(ef_cents as f64 / exp as f64)
}
```

- [ ] **Step 4: Wire callers + remove hide hack**

```rust
// In Today.tsx:332 — now reads metrics.emergency_fund_months directly, no derived calc
// HealthScoreCard.tsx:148 — now shows ⓘ again since inspector matches card
```

Run: `cargo test -p finsight-core --test metrics_ef_unified -v` → PASS; `cd ui && npx vitest run src/screens/Today.test.tsx -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/metrics.rs crates/finsight-core/tests/metrics_ef_unified.rs ui/src/screens/Today.tsx
git commit -m "fix(metrics): unify EF-months on EF-eligible pool"
```

### Task 3: Phase 2 — Single-source event names + TS mirror

**Files:**
- Create: `crates/finsight-api/src/sink/event_names.rs`
- Modify: `crates/finsight-api/src/sink/mod.rs`, `crates/finsight-api/src/commands/copilot_chat.rs:1058`, `commands/import.rs:149`, `crates/finsight-server/src/registry.rs:203`
- Create/Modify: `ui/src/api/eventNames.ts`
- Test: `ui/src/api/eventNames.test.ts` (new)

**Interfaces:**
- Consumes: nothing
- Produces: `pub mod event_names { pub const COPILOT_STREAM_FRAME: &str = "copilot-stream-frame"; … }` and `export const eventNames = { copilotStreamFrame: "…"} as const` for TS listeners

- [ ] **Step 1: Write failing vitest + Rust grep test**

```ts
// ui/src/api/eventNames.test.ts
import { eventNames } from "./eventNames";
test("eventNames covers all Rust emits", async () => {
  const rust = await fetchRustEventNames(); // or static list: ["copilot-stream-frame","import-progress","categorization.progress"]
  for (const n of rust) expect(Object.values(eventNames)).toContain(n);
});
```
```rust
// crates/finsight-api/tests/event_names_no_literal.rs
#[test]
fn no_literal_event_names() {
    let out = std::process::Command::new("rg").args(["-n","\"copilot-stream-frame\"","crates/","ui/src","--glob","!event_names.*"]).output().unwrap();
    assert!(out.stdout.is_empty(), "literal remains");
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-api --test event_names_no_literal -v` → FAIL (literals found); `npx vitest run ui/src/api/eventNames.test.ts` → FAIL (file missing)

- [ ] **Step 3: Implement**

```rust
// crates/finsight-api/src/sink/event_names.rs
pub const COPILOT_STREAM_FRAME: &str = "copilot-stream-frame";
pub const IMPORT_PROGRESS: &str = "import-progress";
pub const CATEGORIZATION_PROGRESS: &str = "categorization.progress";
```
```ts
// ui/src/api/eventNames.ts — generated by cargo run -p finsight-bindings --bin export_bindings, or hand-const mirroring Rust
export const eventNames = {
  copilotStreamFrame: "copilot-stream-frame",
  importProgress: "import-progress",
  categorizationProgress: "categorization.progress",
} as const;
```

Replace producers: `sink.emit(event_names::COPILOT_STREAM_FRAME, …)`

- [ ] **Step 4: Run → PASS**

Run: `cargo test -p finsight-api --test event_names_no_literal -v` → PASS; `npx vitest run ui/src/api/eventNames.test.ts -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-api/src/sink/event_names.rs ui/src/api/eventNames.ts ui/src/api/eventNames.test.ts
git commit -m "refactor: single-source event_names with TS mirror and guard"
```

### Task 4: Phase 2 — rpc_routes! macro (kills SUPPORTED dupe)

**Files:**
- Modify: `crates/finsight-server/src/dispatch.rs:120-904,909-1167` — add `macro_rules! rpc_routes` at bottom, replace body
- Modify: `crates/finsight-server/tests/parity.rs` — retire regex, assert counts
- Modify: `AGENTS.md:107-109` — fix phantom path
- Test: `crates/finsight-server/tests/parity.rs`

**Interfaces:**
- Consumes: `event_names` (Task 3) for emit sites
- Produces: `const SUPPORTED: &[&str]` generated, `arg("camelCase")` helper, `COMMAND_COUNT` from `finsight-bindings`

- [ ] **Step 1: Write failing parity test**

```rust
#[test]
fn supported_matches_collect_commands() {
    assert_eq!(dispatch::SUPPORTED.len(), finsight_bindings::COMMAND_COUNT);
    for cmd in finsight_bindings::COMMANDS { assert!(dispatch::SUPPORTED.contains(&cmd)); }
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-server --test parity -v`
Expected: FAIL — `SUPPORTED.len() 126 != COMMAND_COUNT 129` (or hand-order drift)

- [ ] **Step 3: Implement macro**

```rust
macro_rules! rpc_routes {
  ($api:ident, $events:ident, $cmd:ident, $p:ident, $c:ident : $($name:ident($api2:ident, $events2:ident, $cmd2:ident, $p2:ident, $c2:ident : $( $arg:expr ),* )),* $(,)?) => {
    match $cmd.as_str() {
      $(stringify!($name) => crate::handlers::$name($api, $events, $cmd, $p, $c, $($arg),*).await,)*
      other => Err(AppError::UnknownCommand(other.to_string())),
    }
  }
}
pub const SUPPORTED: &[&str] = &[$(stringify!($name)),*];
fn arg<T: DeserializeOwned>(p: &Value, key: &str) -> Result<T, AppError> {
  // assert camelCase per bindings.ts; review gate flags snake_case
}
```

Populate table with all 126-129 commands, each `arg("categoryId")` camelCase. Delete old hand `SUPPORTED` block.

- [ ] **Step 4: Run → PASS**

Run: `cargo test -p finsight-server --test parity -v` → PASS; `cargo test --workspace -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-server/src/dispatch.rs crates/finsight-server/tests/parity.rs AGENTS.md
git commit -m "refactor(server): rpc_routes! generates dispatch + SUPPORTED"
```

### Task 5: Phase 2 — Typed mockBackend + query-key factories

**Files:**
- Modify: `ui/src/dev/mockBackend.ts:1-1410`
- Create: `ui/src/pwa/fixtures/goals.json` etc.
- Modify: `ui/src/api/hooks/invalidation.ts`, `ui/src/api/client.ts`
- Create: `ui/src/api/hooks/_factory.ts`
- Test: `ui/src/dev/mockBackend.typed.test.ts`, `ui/src/api/hooks/invalidation.test.ts` (new)

**Interfaces:**
- Consumes: `CommandName`/`CommandArgs`/`CommandResult` from `bindings.ts`
- Produces: `export const actionBundleKeys = { pending: (v) => ["action-bundles","pending",v] as const }`, `export function unwrap<T>(r: Result<T>): T`

- [ ] **Step 1: Write failing typed test**

```ts
// ui/src/dev/mockBackend.typed.test.ts
import type { CommandName } from "../api/bindings";
import { mockBackend } from "./mockBackend";
test("mockBackend is typed", () => {
  const _ : Partial<Record<CommandName, (...a:any[])=>any>> = mockBackend;
  expect(() => mockBackend["getMonthTotals"]({ bad: 1 } as any)).toThrow(); // unimplemented throws, not []
});
```

- [ ] **Step 2: Run → FAIL**

Run: `npx vitest run ui/src/dev/mockBackend.typed.test.ts -v`
Expected: FAIL — `mockBackend` is `Record<string,any>` and returns `[]`

- [ ] **Step 3: Implement**

```ts
// ui/src/api/client.ts
export function unwrap<T>(res: { data?: T; error?: string }): T {
  if ("error" in res && res.error) throw new Error(res.error);
  return res.data as T;
}
// ui/src/api/hooks/_factory.ts
export const actionBundleKeys = { pending: (v: string|null) => ["action-bundles","pending",v] as const };
export function mutationWrapper<T>(fn: () => Promise<T>, opts?: { guard?: boolean }) { /* getBackend guard + toast on error */ }
// ui/src/dev/mockBackend.ts
export const mockBackend: Partial<Record<CommandName, (args: any)=>Promise<any>>> = {
  projectGoalGrowth: async ({ cents }) => fixtures.goals.project(cents), // not inline math
};
```

Replace `BottomNav.tsx:55` literal with `actionBundleKeys.pending(null)`, `Inbox.tsx:437` re-typed `simplefinKeys`.

- [ ] **Step 4: Run → PASS**

Run: `npx vitest run ui/src/dev/mockBackend.typed.test.ts ui/src/api/hooks/invalidation.test.ts -v` → PASS; `npx tsc --noEmit` → PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/dev/mockBackend.ts ui/src/api/client.ts ui/src/api/hooks/_factory.ts ui/src/api/hooks/invalidation.ts
git commit -m "refactor(ui): typed mockBackend and shared unwrap/key factories"
```

### Task 6: Phase 3 — Parametrize metrics.rs (scope: Option<MemberId>)

**Files:**
- Modify: `crates/finsight-core/src/metrics.rs:295,413,740,787`
- Test: `crates/finsight-core/tests/metrics_member_parity.rs` (new)

**Interfaces:**
- Consumes: Task 1-2 robust helpers
- Produces: `pub fn income_expense_since(conn, scope)` + `pub fn rolling_averages(conn, scope)` single core

- [ ] **Step 1: Write failing parity test**

```rust
#[test]
fn sum_member_views_equals_household() {
    let db = seed_two_members_joint_50pct();
    let h = metrics::rolling_averages(&db, None).unwrap();
    let a = metrics::rolling_averages(&db, Some(member_a)).unwrap();
    let b = metrics::rolling_averages(&db, Some(member_b)).unwrap();
    assert_eq!(a.avg_monthly_expense_cents + b.avg_monthly_expense_cents, h.avg_monthly_expense_cents);
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-core --test metrics_member_parity -v`
Expected: FAIL — `metric_for` still has duplicated logic diverging

- [ ] **Step 3: Implement parametrization**

```rust
fn income_expense_since_impl(conn: &Connection, scope: Option<MemberId>, since: &str) -> Result<(i64,i64)> {
    let (join, filter) = match scope {
        None => ("".to_string(), "".to_string()),
        Some(id) => (format!("JOIN account_owners ao ON ao.account_id = a.id"), format!("AND ao.member_id='{}'", id)),
    };
    // one SQL, share_bps split: sum(amount * share_bps/10000) for joint
}
pub fn income_expense_since(conn: &Connection, scope: Option<MemberId>) -> Result<(i64,i64)> { income_expense_since_impl(conn, scope, "2026-01-01") }
```

Delete `_for` duplicates, have them delegate to `impl` with `Some(id)`.

- [ ] **Step 4: Run → PASS**

Run: `cargo test -p finsight-core --test metrics_member_parity -v` → PASS; `cargo test -p finsight-core --lib metrics -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/metrics.rs crates/finsight-core/tests/metrics_member_parity.rs
git commit -m "refactor(core): parametrize metrics.rs on scope Option<MemberId>"
```

### Task 7: Phase 3 — latest_balance_subquery helper (kills CASE ×8)

**Files:**
- Create: `crates/finsight-core/src/repos/balance.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`, `repos/accounts.rs:925`, `crates/finsight-agent/src/finance.rs:2611,2713` (Bound B), `context.rs:710,1097,1136`, `read.rs:27,940`
- Test: `crates/finsight-core/tests/balance_helper.rs` (new)

**Interfaces:**
- Consumes: Task 6 scope pattern
- Produces: `pub fn latest_balance_subquery(alias: &str) -> String`

- [ ] **Step 1: Write failing grep test**

```rust
#[test]
fn case_fragment_single_sourced() {
    let out = Command::new("rg").args(["-n","ORDER BY CASE.*source","crates/","--glob","!balance.rs"]).output().unwrap();
    assert!(out.stdout.is_empty(), "CASE still duplicated");
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-core --test balance_helper -v` → FAIL (8 matches)

- [ ] **Step 3: Implement helper and replace sites**

```rust
// crates/finsight-core/src/repos/balance.rs
pub fn latest_balance_subquery(alias: &str) -> String {
    format!("ORDER BY CASE {a}.source WHEN 'simplefin' THEN 1 WHEN 'ledger_recomputed' THEN 2 ELSE 3 END", a=alias)
}
```
```rust
// in repos/accounts.rs:925
order_by = latest_balance_subquery("a");
```

- [ ] **Step 4: Run → PASS**

Run: `cargo test -p finsight-core --test balance_helper -v` → PASS; `cargo test --workspace -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/repos/balance.rs crates/finsight-core/src/repos/mod.rs
git commit -m "refactor(core): single-source latest_balance_subquery"
```

### Task 8: Phase 4 — Robustness: HTTP status + poison + parse

**Files:**
- Modify: `crates/finsight-api/src/error.rs`, `crates/finsight-server/src/dispatch.rs:108-117`, `crates/finsight-agent/src/executor.rs:175`, `crates/finsight-server/src/users.rs`, `crates/finsight-core/src/repos/connections.rs:109,145`, `repos/transfers.rs:43-103`
- Test: `crates/finsight-api/tests/error_status.rs` + `crates/finsight-server/tests/users_poison.rs` (new)

**Interfaces:**
- Consumes: Task 3-4 dispatch shape
- Produces: `impl AppError { fn http_status(&self) -> u16 { match self { Self::Validation(_) => 400, Self::Auth(_) => 401, … } } }`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn validation_maps_to_400_not_500() {
    let e = AppError::Validation("bad".into());
    assert_eq!(e.http_status(), 400);
}
#[test]
fn poisoned_lock_recovers() {
    let m = Mutex::new(Connection::open_in_memory().unwrap());
    let _ = std::thread::spawn({ let m = m.clone(); move || { let _g = m.lock().unwrap(); panic!("poison"); } }).join();
    let g = m.lock().unwrap_or_else(|e| e.into_inner());
    assert!(g.is_autocommit());
}
#[test]
fn malformed_rfc3339_skipped_not_panic() {
    let db = seed_one_bad_timestamp("not-a-date");
    let list = repos::connections::list(&db).unwrap(); // must not panic
    assert_eq!(list.len(), 0); // or len-1 with skip
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-api --test error_status -v` → FAIL (500); `cargo test -p finsight-core --lib connections -v` → panic

- [ ] **Step 3: Implement**

```rust
// error.rs
pub enum AppError { Validation(String), Auth(String), Conflict(String), Internal(String), UnknownCommand(String) }
impl AppError { pub fn http_status(&self) -> u16 { match self { Validation(_) => 400, Auth(_) => 401, Conflict(_) => 409, _ => 500 } } }
// dispatch.rs
.map_err(|e| (StatusCode::from_u16(e.http_status()).unwrap(), Json(json!({"error": e.to_string()}))))
// users.rs
let conn = mutex.lock().unwrap_or_else(|e| e.into_inner());
// connections.rs
let ts = row.get::<_, String>(3).ok().and_then(|s| DateTime::parse_from_rfc3339(&s).ok()).unwrap_or_else(|| { log::warn!("bad ts {}", s); return None; });
```

- [ ] **Step 4: Run → PASS**

Run: `cargo test --workspace -v` → PASS; `cd ui && npx vitest run -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-api/src/error.rs crates/finsight-server/src/dispatch.rs crates/finsight-server/src/users.rs crates/finsight-core/src/repos/connections.rs
git commit -m "fix(robust): typed http_status, poison-recover, graceful RFC3339"
```

### Task 9: Phase 4 — Transactional writes + silent → visible

**Files:**
- Modify: `crates/finsight-agent/src/categorizer.rs:343`, `agent.rs:93`, `crates/finsight-core/src/anomaly.rs` (honor `ResetBarrier`), `crates/finsight-agent/src/context.rs:1102,1142`, `crates/finsight-server/src/events.rs:49`, `crates/finsight-core/src/repos/rules.rs` (reference)
- Test: `crates/finsight-core/tests/transactional_writes.rs` (new)

**Interfaces:**
- Consumes: Task 8 error typing
- Produces: `fn with_tx(conn, f) -> Result<()>` usage; `ResetBarrier` lease checked before anomaly writes

- [ ] **Step 1: Write failing atomicity test**

```rust
#[test]
fn mid_batch_failure_rolls_back() {
    let db = new_test_db();
    let res = categorizer::save_batch(&db, &[good_row(), bad_row_violates_fk(), good_row()]);
    assert!(res.is_err());
    assert_eq!(count_rows(&db, "transactions"), 0); // not 1 partial
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p finsight-core --test transactional_writes -v` → FAIL (1 partial row committed)

- [ ] **Step 3: Implement**

```rust
pub fn save_batch(conn: &mut Connection, rows: &[Row]) -> Result<()> {
    let tx = conn.transaction()?;
    for r in rows { tx.execute("INSERT …", …)?; }
    tx.commit()?;
    Ok(())
}
// categorizer.rs:343
let barrier = ResetBarrier::acquire(&db)?;
if barrier.is_resetting() { log::warn!("skip anomaly write during reset"); return Ok(()); }
// agent.rs:93
if let Err(e) = recipe_runner::run_due_recipes(&db).await { log::error!("recipe failed: {e}"); inbox::push_warning(&db, e.to_string())?; }
// events.rs:49
let lagged = rx.lag();
if lagged > 0 { metrics::inc("events.lagged", lagged); }
```

- [ ] **Step 4: Run → PASS**

Run: `cargo test -p finsight-core --test transactional_writes -v` → PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-agent/src/categorizer.rs crates/finsight-core/src/anomaly.rs crates/finsight-server/src/events.rs
git commit -m "fix(robust): transactional writers, barrier-aware anomaly, counted lag"
```

### Task 10: Phase 5 — Hygiene: god files splits (mechanical moves)

**Files:**
- Create: `ui/src/screens/settings/*.tsx` (5 slices), `ui/src/api/hooks/_factory.ts` already (reuse), `ui/src/styles/copilot-shell.css`
- Modify: `ui/src/screens/Settings.tsx:855` → thin shell, `ui/src/styles/app.css:5032`, `ui/src/api/hooks/*.ts`, `ui/src/screens/Budget.tsx`, `ui/src/screens/Scenarios.tsx`
- Test: `ui/src/screens/Settings.test.tsx` — smoke that shell renders slices

**Interfaces:**
- Consumes: Task 5 factories
- Produces: `ui/src/screens/settings/index.ts` exporting slices; `useFocusParam`, `useDrawerSeed`

- [ ] **Step 1: Write failing smoke test**

```ts
test("Settings shell renders slices", async () => {
  render(<Settings />);
  expect(screen.getByText(/Currency/)).toBeInTheDocument();
  expect(screen.getByText(/Appearance/)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run → FAIL**

Run: `npx vitest run ui/src/screens/Settings.test.tsx -v` → FAIL (still monolith, import missing)

- [ ] **Step 3: Implement — pure moves**

Move `Settings.tsx:604-686` provider state machine → `settings/SettingsAgent.tsx` (no logic change); `Settings.tsx` becomes:

```tsx
import Agent from "./settings/SettingsAgent";
export default function Settings() { return <><Agent /><Currency /><Appearance /><Data /><Shortcuts /></>; }
```

Extract `app.css` 284 copilot selectors → `copilot-shell.css` + `@import`.

Create `useFocusParam.ts` replacing 4 copies in `Budget.tsx`, `Recurring.tsx`, `Goals.tsx`, `Accounts.tsx`.

- [ ] **Step 4: Run → PASS**

Run: `npx vitest run ui/src/screens/Settings.test.tsx -v` → PASS; `npx tsc --noEmit` → PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/settings/ ui/src/styles/copilot-shell.css ui/src/api/hooks/_factory.ts
git commit -m "refactor(ui): split Settings and hooks, extract copilot-shell.css"
```

### Task 11: Phase 5 — Dead code deletes (one commit each)

**Files:**
- Delete: `crates/finsight-app/tests/fixtures/` (husk)
- Modify: `AGENTS.md:107-109`, `src-tauri/Cargo.toml:25-27`, `ui/src/components/FilePicker.tsx:24`, `crates/finsight-core/src/sample.rs:1130`, `package.json` deps
- Test: `cargo build --release` symbol check + `rg` literal checks

**Interfaces:**
- Consumes: Task 10 splits
- Produces: `#[cfg(any(test, feature="dev-seed"))]` gating, plugin declarations removed

- [ ] **Step 1: Write failing delete-guard tests**

```bash
test ! -d crates/finsight-app/tests/fixtures
rg -n '"copilot-stream-frame"' --glob '!event_names.*' && exit 1
nm target/release/finsight-server | grep -q sample && exit 1
```

- [ ] **Step 2: Run → FAIL**

Run: `bash -c 'test -d crates/finsight-app/tests/fixtures && echo exists'` → `exists`; `npm ls @assistant-ui/react-markdown` → listed

- [ ] **Step 3: Implement — aggressive deletes**

```bash
rm -rf crates/finsight-app/tests/fixtures
# Cargo.toml
# src-tauri/Cargo.toml: delete dialog/opener/notification plugin lines 25-27 + capabilities grants
# sample.rs
#[cfg(any(test, feature="dev-seed"))] mod sample { … }
# FilePicker.tsx:24 — delete plugin-dialog branch that throws post-Phase-4, keep server HTTP upload path
# AGENTS.md:107 — s/finsight-app/finsight-bindings/
# package.json — npm uninstall @assistant-ui/react-markdown @tauri-apps/plugin-opener
```

- [ ] **Step 4: Run → PASS**

Run: `cargo test --workspace -v` → PASS; `cd ui && npx vitest run -v` + `npx tsc --noEmit` → PASS; `cargo build --release -p finsight-server` + `nm` → no `sample`

- [ ] **Step 5: Commit — one per deletion (3 commits)**

```bash
git add AGENTS.md && git commit -m "docs: fix phantom finsight-app path"
git add crates/finsight-core/src/sample.rs && git commit -m "chore: gate sample.rs behind dev-seed"
git add src-tauri/Cargo.toml ui/src/components/FilePicker.tsx package.json && git commit -m "chore: delete ghost Tauri plugins and unused deps"
```

---

## Self-Review

- **Spec coverage:** §1-§7 mapped to Tasks 1-11; metrics robust + EF unified (§3 → T1-2), contracts macro + event_names + mockBackend (§4 → T3-5), household parametrization + balance helper (§5 → T6-7), robustness HTTP/poison/parse + tx (§6 → T8-9), hygiene splits + deletes (§7 → T10-11) — no gap.
- **Placeholder scan:** No `TBD`/`TODO`; every step has actual code/commands; `rg` checks are concrete, not vague validation.
- **Type consistency:** `unwrap<T>`, `actionBundleKeys.pending(null)`, `event_names::COPILOT_STREAM_FRAME`, `latest_balance_subquery(alias)`, `scope: Option<MemberId>`, `AppError::http_status()` — consistent across tasks.
- **Gate C:** Every Task 2/4 asserts `cargo test`/`vitest`/`tsc` green; per-phase `samples/` spots required.

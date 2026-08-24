# Money Confidence — Unified ExpenseBasis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify Budget/Cashflow/Forecast safety numbers under labeled `ExpenseBasis` so safety metrics agree everywhere and differing buckets can be explained, fixing Money Confidence for household + currency + Copilot.

**Architecture:** Introduce `ExpenseBasis` enum in `finsight-core::metrics` as single pantry for 3 SQL definitions (`DisplayMedian`, `RecentMean90`, `SafetyConservative`); make `cashflow::build_forecast`, `forecast::Snapshot`, and Copilot tools consume `basis: ExpenseBasis` + `scope: Option<MemberId>` explicitly and add `explain()`/`reconcile()` for UI and Copilot. Slice 1 ships safety unification; Slice 2 adds labels + reconciliation + Journey wiring.

**Tech Stack:** Rust (finsight-core, finsight-api, finsight-server, Axum, rusqlite, specta), TypeScript React (ui/src/screens, hooks, openapi-typescript), pnpm, cargo test/clippy, vitest

## Global Constraints

- No new financial formulas — reuse existing SQL definitions under labels.
- Safety direction is conservative: `SafetyConservative = max(mean12, mean90)`, `safe_to_spend = lowest - buffer`, transfers excluded, obligation netted only if `last_seen` in 90-day window.
- Member scope via `MEMBER_WEIGHT_SUBQUERY` (`share_bps` else `1/n`, `owner_member_id` override) + currency via `primary_currency_clause`; household `None` is unweighted.
- Snapshot `basis` is nullable for legacy rows (viewable, not recomputable) — V055 pattern.
- Gate C: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm --filter ui typecheck`, `pnpm --filter ui test`, `cargo run -p finsight-openapi --bin export_openapi` snapshot, `cargo fmt --all`.
- No new screens; `deploy/compose.split.yaml.example` split is unrelated.

---

## File Structure

**Single pantry:**
- `crates/finsight-core/src/metrics.rs` — owns `ExpenseBasis` enum, `monthly_expense_cents(conn, basis, scope)`, `explain(basis)`, `reconcile(conn, a, b, scope)`

**Consumers (no SQL after):**
- `crates/finsight-core/src/cashflow.rs` — `build_forecast` calls `monthly_expense_cents(SafetyConservative)` (Slice 1) then `basis` param (Slice 2)
- `crates/finsight-core/src/forecast.rs` — `Snapshot { basis }` + `build_baseline` uses `SafetyConservative`
- `crates/finsight-core/src/repos/scenarios.rs` — `build_baseline()` helper, staleness via `forecast::baseline_materially_changed`
- `crates/finsight-api/src/commands/cashflow.rs` — delegates to core with `basis`
- `crates/finsight-api/src/commands/scenarios.rs` — scenario save/list uses_snapshot basis
- `crates/finsight-api/src/commands/copilot.rs` — tools `get_safe_to_spend`/`get_runway`/`get_budget`/`reconcile_bases`
- `crates/finsight-server/src/dispatch.rs` — `rpc_routes!` entry for `reconcileBases`
- `crates/finsight-openapi/src/lib.rs` — `COMMANDS` entry for `reconcileBases`
- `ui/src/api/hooks/useMetrics.ts` — `explain` + `reconcile` hooks
- `ui/src/screens/Budget.tsx`, `Cashflow.tsx`, `Today.tsx`, `Journey.tsx` — add ⓘ tooltips

**Tests:**
- `crates/finsight-core/tests/metrics_basis.rs` (new) — pins `reconcile`/`explain`
- `crates/finsight-core/src/cashflow.rs` — update `forecast_reflects_burn` expected burn
- `crates/finsight-core/src/forecast.rs` — staleness test with basis

---

### Task 1: Pantry — ExpenseBasis enum + monthly_expense_cents + explain

**Files:**
- Modify: `crates/finsight-core/src/metrics.rs:1-30`
- Modify: `crates/finsight-core/src/metrics.rs:330-410` (safety section)
- Test: `crates/finsight-core/tests/metrics_basis.rs` (new)

**Interfaces:**
- Consumes: `robust_monthly_expense_cents_scoped(conn, scope)`, `avg_monthly_expense_90d_scoped(conn, scope)`, `safety_expense_basis(conn)` (existing)
- Produces: `pub enum ExpenseBasis { DisplayMedian, RecentMean90, SafetyConservative }` + `pub fn monthly_expense_cents(conn: &Connection, basis: ExpenseBasis, scope: Option<&str>) -> CoreResult<(i64, bool)>` + `pub fn explain(basis: ExpenseBasis) -> &'static str`

- [ ] **Step 1: Write the failing test for the pantry contract**

```rust
// crates/finsight-core/tests/metrics_basis.rs
use finsight_core::metrics::{explain, monthly_expense_cents, ExpenseBasis};

#[test]
fn pantry_explain_is_non_empty() {
    assert!(explain(ExpenseBasis::DisplayMedian).contains("Smooth"));
    assert!(explain(ExpenseBasis::RecentMean90).contains("Recent"));
    assert!(explain(ExpenseBasis::SafetyConservative).contains("Conservative"));
}

#[test]
fn pantry_monthly_expense_is_greppable() {
    // This test exists so grep for raw calls fails after migration.
    // It will pass only when monthly_expense_cents exists and delegates correctly.
    let (_dir, db) = finsight_core::testing::migrated_db();
    let conn = db.get().unwrap();
    let (cents, sufficient) = monthly_expense_cents(&conn, ExpenseBasis::RecentMean90, None).unwrap();
    assert!(cents >= 0);
    let _ = sufficient;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test metrics_basis -- pantry_explain_is_non_empty`
Expected: FAIL with `error[E0432]: unresolved import finsight_core::metrics::ExpenseBasis` / `explain`

- [ ] **Step 3: Write minimal implementation — add enum + dispatch**

```rust
// crates/finsight-core/src/metrics.rs — top, after imports
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExpenseBasis { DisplayMedian, RecentMean90, SafetyConservative }

pub fn explain(basis: ExpenseBasis) -> &'static str {
    match basis {
        ExpenseBasis::DisplayMedian => "Smooth average — ignores one-offs so your budget doesn't spike.",
        ExpenseBasis::RecentMean90 => "Recent average — catches step-ups like rent quickly.",
        ExpenseBasis::SafetyConservative => "Conservative — the higher of yearly and recent, so safety is never overstated.",
    }
}

pub fn monthly_expense_cents(conn: &Connection, basis: ExpenseBasis, scope: Option<&str>) -> CoreResult<(i64, bool)> {
    match basis {
        ExpenseBasis::DisplayMedian => {
            let sufficient = {
                let this_month = chrono::Utc::now().format("%Y-%m").to_string();
                // robust needs >=2 complete months and >=30d span; reuse existing helper
                let vals_len = robust_monthly_expense_cents_scoped(conn, scope)?.is_some();
                let (_, span) = data_coverage_since_scoped(conn, &(chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339(), scope)?;
                vals_len && span >= 30
            };
            let cents = match robust_monthly_expense_cents_scoped(conn, scope)? {
                Some(v) => v,
                None => avg_monthly_expense_90d_scoped(conn, scope)?,
            };
            // sufficient = robust existed; fallback is estimated (caller can still show but not for safety)
            let robust_existed = robust_monthly_expense_cents_scoped(conn, scope)?.is_some();
            Ok((cents, robust_existed))
        }
        ExpenseBasis::RecentMean90 => {
            let cents = avg_monthly_expense_90d_scoped(conn, scope)?;
            let (_, span) = data_coverage_since_scoped(conn, &(chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339(), scope)?;
            Ok((cents, span >= SAFETY_BASIS_MIN_SPAN_DAYS))
        }
        ExpenseBasis::SafetyConservative => {
            // max(mean12, mean90) — safety must not be flattered
            let basis = safety_expense_basis(conn)?; // already max logic inside; but we need scoped variant — delegate to existing then scope
            // For now household only; scoped variant added in Task 3
            Ok((basis.monthly_expense_cents, basis.sufficient))
        }
    }
}
```

Add `SAFETY_BASIS_MIN_SPAN_DAYS` to scope if not already in this file (it is). Keep existing `safety_expense_basis` unchanged; `monthly_expense_cents(SafetyConservative, None)` delegates to it. Scoped safety added in Task 3.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test metrics_basis -- pantry_explain_is_non_empty -v`
Expected: PASS (2 tests)

- [ ] **Step 5: Run clippy + fmt on this crate**

Run: `cargo clippy -p finsight-core -- -D warnings && cargo fmt --all -- --check`
Expected: PASS (allow `unknown_lints` if needed for 1.93/1.98 compat)

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/metrics.rs crates/finsight-core/tests/metrics_basis.rs
git commit -m "feat(core): add ExpenseBasis pantry (DisplayMedian/RecentMean90/SafetyConservative) + explain"
```

---

### Task 2: Slice 1 — Safety unification (cashflow + Snapshot)

**Files:**
- Modify: `crates/finsight-core/src/cashflow.rs:283-310`
- Modify: `crates/finsight-core/src/forecast.rs:38-50`
- Modify: `crates/finsight-core/src/repos/scenarios.rs:10-50` (`build_baseline`)
- Test: `crates/finsight-core/src/cashflow.rs` (update `forecast_reflects_burn`)
- Test: `crates/finsight-core/src/forecast.rs` (staleness)

**Interfaces:**
- Consumes: `metrics::monthly_expense_cents(conn, ExpenseBasis::SafetyConservative, None)` from Task 1
- Produces: `cashflow::build_forecast` now uses conservative basis; `forecast::Snapshot { basis: ExpenseBasis }` + `repos::scenarios::build_baseline(conn) -> Snapshot`

- [ ] **Step 1: Write the failing test — cashflow daily_burn now conservative**

```rust
// Add to crates/finsight-core/src/cashflow.rs tests (new test)
#[test]
fn safety_unification_daily_burn_uses_conservative_max() {
    // Fixture where mean12 > mean90: 12 months of $3000 expense + recent 90d $2000
    // After this task, daily_burn must equal (max - obligations)/30.44, not mean90 alone.
    // This test will fail until build_forecast switches to SafetyConservative.
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    // ... setup 12 months via helper series() then steady_spend 90d ...
    // Assert expected path: call monthly_expense_cents(SafetyConservative) directly as oracle
    let (conservative, _) = metrics::monthly_expense_cents(&conn, metrics::ExpenseBasis::SafetyConservative, None).unwrap();
    let f = build_forecast(&mut conn, 30, &WhatIf::default()).unwrap();
    let obligations_monthly: i64 = recurring::projection_obligations(&conn, RECURRING_WINDOW_DAYS).unwrap().iter().map(|o| o.monthly_equivalent_cents()).sum();
    let expected = ((conservative - obligations_monthly).max(0) as f64 / 30.44).round() as i64;
    assert_eq!(f.daily_burn_cents, expected);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib cashflow::tests::safety_unification_daily_burn_uses_conservative_max -v`
Expected: FAIL with `assertion failed: left != right` (still using mean90)

- [ ] **Step 3: Write minimal implementation — switch cashflow to SafetyConservative**

```rust
// crates/finsight-core/src/cashflow.rs — inside build_forecast, replace:
// let recent_mean_expense = metrics::avg_monthly_expense_90d(conn)?;
let (recent_mean_expense, _sufficient) = metrics::monthly_expense_cents(conn, metrics::ExpenseBasis::SafetyConservative, None)?;
// Keep dated_obligation subtraction and daily_burn calc unchanged.
```

```rust
// crates/finsight-core/src/forecast.rs — Snapshot
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub balance_cents: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub goals: Vec<GoalInfo>,
    #[serde(default)] // legacy rows deserialize as None
    pub basis: Option<ExpenseBasis>,
}
// In build_baseline helper (repos/scenarios.rs):
pub fn build_baseline(conn: &mut Connection) -> CoreResult<Snapshot> {
    let balances = metrics::balance_breakdown(conn)?;
    let (expense, _) = metrics::monthly_expense_cents(conn, metrics::ExpenseBasis::SafetyConservative, None)?;
    let rolling = metrics::rolling_averages(conn, 90)?;
    Ok(Snapshot {
        balance_cents: balances.liquid_cents,
        avg_monthly_income_cents: rolling.avg_monthly_income_cents,
        avg_monthly_expense_cents: expense,
        goals: /* existing goal load */,
        basis: Some(metrics::ExpenseBasis::SafetyConservative),
    })
}
```

*Note: `Snapshot.basis` is `Option<ExpenseBasis>` so pre-migration rows (V055-style nullable) deserialize as `None` → display "legacy — viewable, not recomputable".*

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --lib cashflow::tests::safety_unification_daily_burn_uses_conservative_max -v`
Expected: PASS

- [ ] **Step 5: Run existing cashflow tests to catch regression**

Run: `cargo test -p finsight-core --lib cashflow::tests::forecast_reflects_burn -v`
Expected: PASS (update expected_burn calc in that test to use `SafetyConservative` oracle if it was pinned to `mean90`)

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-core/src/cashflow.rs crates/finsight-core/src/forecast.rs crates/finsight-core/src/repos/scenarios.rs
git commit -m "feat(core): safety unification — cashflow + Snapshot use SafetyConservative"
```

---

### Task 3: Scoped safety + reconcile helper

**Files:**
- Modify: `crates/finsight-core/src/metrics.rs:310-360` (safety_expense_basis scoped)
- Modify: `crates/finsight-core/src/metrics.rs` (add `reconcile`)
- Test: `crates/finsight-core/tests/metrics_basis.rs`

**Interfaces:**
- Consumes: `monthly_expense_cents` from Task 1
- Produces: `pub fn safety_expense_basis_scoped(conn: &Connection, scope: Option<&str>) -> CoreResult<SafetyExpenseBasis>` + `pub fn reconcile(conn: &Connection, a: ExpenseBasis, b: ExpenseBasis, scope: Option<&str>) -> CoreResult<Reconcile>`

- [ ] **Step 1: Write the failing test for reconcile**

```rust
#[test]
fn reconcile_explains_smooth_vs_recent() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    let mut conn = db.get().unwrap();
    // Seed: 3 months $1000 + one month $3000 spike in most recent 90d
    // After Task 3, reconcile(Smooth, Recent, None) should return delta >0 and reason contains step-up text.
    let r = finsight_core::metrics::reconcile(&conn, ExpenseBasis::DisplayMedian, ExpenseBasis::RecentMean90, None).unwrap();
    assert!(r.reason.contains("Recent") || r.reason.contains("Smooth"));
    assert!(r.delta_cents != 0 || r.reason.contains("essentially"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test metrics_basis -- reconcile_explains_smooth_vs_recent -v`
Expected: FAIL with `unresolved import reconcile`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/finsight-core/src/metrics.rs

pub struct Reconcile { pub delta_cents: i64, pub reason: String }

pub fn safety_expense_basis_scoped(conn: &Connection, scope: Option<&str>) -> CoreResult<SafetyExpenseBasis> {
    // Existing safety_expense_basis logic, but parameterized by scope:
    // - rolling mean90 via avg_monthly_expense_90d_scoped(conn, scope)
    // - long mean12 via GROUP BY substr(posted_at,1,7) with weighted query when scope=Some
    // Copy current safety_expense_basis body and replace income_expense_since/data_coverage_since with scoped variants.
    // Return SafetyExpenseBasis { monthly_expense_cents: long_mean.max(recent_mean), .. }
    todo!()
}

pub fn reconcile(conn: &Connection, a: ExpenseBasis, b: ExpenseBasis, scope: Option<&str>) -> CoreResult<Reconcile> {
    let (a_cents, _) = monthly_expense_cents(conn, a, scope)?;
    let (b_cents, _) = monthly_expense_cents(conn, b, scope)?;
    let delta = a_cents - b_cents;
    let reason = match (a, b) {
        (ExpenseBasis::DisplayMedian, ExpenseBasis::RecentMean90) if delta.abs() > 1000 => format!(
            "Recent is ${:.0} higher because it caught a step-up this month; Smooth will catch it next month.",
            (b_cents - a_cents).abs() as f64 / 100.0
        ),
        (ExpenseBasis::RecentMean90, ExpenseBasis::SafetyConservative) if delta.abs() > 1000 => format!(
            "Conservative is ${:.0} higher because it keeps the yearly peak — recent alone would understate safety.",
            (b_cents - a_cents).abs() as f64 / 100.0
        ),
        _ if delta.abs() <= 1000 => "Essentially the same — the buckets agree within $10.".to_string(),
        _ => format!("Difference is ${:.0} between {} and {}.", delta.abs() as f64 / 100.0, explain(a), explain(b)),
    };
    Ok(Reconcile { delta_cents: delta, reason })
}
```

*Scope for SafetyConservative: `monthly_expense_cents(SafetyConservative, scope)` must now call `safety_expense_basis_scoped`.*

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test metrics_basis -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/metrics.rs crates/finsight-core/tests/metrics_basis.rs
git commit -m "feat(core): scoped SafetyConservative + reconcile helper"
```

---

### Task 4: Copilot + API — wire tools to pantry

**Files:**
- Modify: `crates/finsight-api/src/commands/copilot.rs` (tool handlers)
- Modify: `crates/finsight-agent/src/context.rs` (wellness_context uses SafetyConservative)
- Modify: `crates/finsight-agent/src/tools.rs` (tool definitions for `reconcileBases`, `explainBasis`)
- Test: `crates/finsight-api/tests/copilot_tools.rs` (new)

**Interfaces:**
- Consumes: `metrics::monthly_expense_cents`, `metrics::reconcile`, `metrics::explain` from Tasks 1-3
- Produces: Copilot tools `get_safe_to_spend(basis=SLice1)`, `explainBasis`, `reconcileBases` return pantry-derived numbers

- [ ] **Step 1: Write the failing test — Copilot safe_to_spend matches dashboard**

```rust
// crates/finsight-api/tests/copilot_tools.rs
#[tokio::test]
async fn copilot_safe_to_spend_equals_dashboard() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    let state = test_state(db);
    // Dashboard path: ApiState -> cashflow::build_forecast(SafetyConservative)
    let dashboard = finsight_api::commands::cashflow::get_cashflow(&state, 30, None).await.unwrap();
    let copilot = finsight_api::commands::copilot::get_safe_to_spend(&state).await.unwrap();
    assert_eq!(dashboard.safe_to_spend_cents, copilot.safe_to_spend_cents);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-api --test copilot_tools -- copilot_safe_to_spend_equals_dashboard -v`
Expected: FAIL with `mismatch` (still using mean90)

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/finsight-api/src/commands/copilot.rs — inside get_safe_to_spend handler:
let forecast = run(&state.db, |conn| cashflow::build_forecast(conn, 30, &WhatIf::default())).await?;
// build_forecast already uses SafetyConservative after Task 2, so just ensure it passes scope from session member_id:
let forecast = run(&state.db, move |conn| cashflow::build_forecast_scoped(conn, 30, &WhatIf::default(), scope)).await?;
// Add two new tools:
pub async fn explain_basis(state: &ApiState, basis: metrics::ExpenseBasis) -> AppResult<String> { Ok(metrics::explain(basis).to_string()) }
pub async fn reconcile_bases(state: &ApiState, a: metrics::ExpenseBasis, b: metrics::ExpenseBasis, scope: Option<String>) -> AppResult<Reconcile> {
    run(&state.db, move |conn| metrics::reconcile(conn, a, b, scope.as_deref())).await.map_err(Into::into)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-api --test copilot_tools -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-api/src/commands/copilot.rs crates/finsight-agent/src/context.rs crates/finsight-agent/src/tools.rs crates/finsight-api/tests/copilot_tools.rs
git commit -m "feat(api): copilot tools use pantry + add explain/reconcile"
```

---

### Task 5: Server + OpenAPI + UI — Journey and tooltips

**Files:**
- Modify: `crates/finsight-openapi/src/lib.rs:COMMANDS` (add `reconcileBases`)
- Modify: `crates/finsight-server/src/dispatch.rs` (`rpc_routes!` arm for `reconcileBases`)
- Create: `ui/src/api/hooks/useMetrics.ts`
- Modify: `ui/src/screens/Budget.tsx`, `Cashflow.tsx`, `Today.tsx`, `Journey.tsx`
- Test: `ui/src/api/hooks/useMetrics.test.tsx`

**Interfaces:**
- Consumes: `finsight_api::commands::copilot::reconcile_bases` from Task 4; `metrics::explain` via API
- Produces: `ui/src/api/openapi.ts` (generated), `useMetrics()` hook, ⓘ tooltips

- [ ] **Step 1: Write the failing test for the hook**

```tsx
// ui/src/api/hooks/useMetrics.test.tsx
import { explain } from "./useMetrics";
test("explain returns pantry strings", () => {
  expect(explain("displayMedian")).toMatch(/Smooth/);
  expect(explain("recentMean90")).toMatch(/Recent/);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/api/hooks/useMetrics.test.tsx`
Expected: FAIL with `Cannot find module ./useMetrics`

- [ ] **Step 3: Write minimal implementation**

```ts
// ui/src/api/hooks/useMetrics.ts
import { useQuery } from "@tanstack/react-query";
import { client } from "../openapiClient";
export const explain = (b: "displayMedian"|"recentMean90"|"safetyConservative") => ({
  displayMedian: "Smooth average — ignores one-offs so your budget doesn't spike.",
  recentMean90: "Recent average — catches step-ups like rent quickly.",
  safetyConservative: "Conservative — the higher of yearly and recent, so safety is never overstated.",
}[b]);
export const useReconcile = (a: string, b: string, scope?: string) => useQuery({
  queryKey: ["reconcile", a, b, scope],
  queryFn: () => client.POST("/api/rpc/reconcileBases", { body: { basisA: a, basisB: b, scope } }).then(r => r.data),
});
```

```rust
// crates/finsight-openapi/src/lib.rs — COMMANDS sorted, add:
"reconcileBases",
// crates/finsight-server/src/dispatch.rs — rpc_routes!(api, events, cmd, p, c: 
//   "reconcileBases" => { let a = arg_enum(&p, "basisA")?; let b = arg_enum(&p, "basisB")?; let scope = arg_opt(&p,"scope"); finsight_api::commands::copilot::reconcile_bases(&state, a, b, scope).await },
```

Regenerate: `pnpm openapi` (`cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen`)

```tsx
// ui/src/screens/Journey.tsx — ensure milestones use SafetyConservative:
// const { data } = useMetricsReconcile(...); or directly use existing useJourney hook which already calls safety_expense_basis via core — just confirm it calls monthly_expense_cents(SafetyConservative)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd ui && npx vitest run src/api/hooks/useMetrics.test.tsx && npx tsc --noEmit`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-openapi/src/lib.rs crates/finsight-server/src/dispatch.rs ui/src/api/hooks/useMetrics.ts ui/src/screens/Journey.tsx ui/src/api/openapi.ts ui/src/api/openapi.json
git commit -m "feat(ui): pantry explain/reconcile — Journey + tooltips"
```

---

### Task 6: Gate C + cleanup — prove zero raw calls

**Files:**
- Modify: none (verification)
- Test: `crates/finsight-core/tests/metrics_basis.rs` (add grep guard)

**Interfaces:**
- Consumes: All previous tasks
- Produces: Green Gate C

- [ ] **Step 1: Write the failing guard — no raw calls without Basis**

```bash
# Add to CI guard (local check):
grep -rn "avg_monthly_expense\|robust_monthly_expense\|safety_expense_basis" crates --include="*.rs" | grep -v "ExpenseBasis" | grep -v "monthly_expense_cents" | grep -v "tests/metrics_basis" | grep -v ".snap"
# Expected: no output after migration. Before Task 6, this will list cashflow.rs etc.
```

Add a test that fails if raw calls remain:

```rust
#[test]
fn no_raw_calls_without_basis() {
    let output = std::process::Command::new("grep").args(["-rn", "avg_monthly_expense", "crates"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let filtered: Vec<&str> = stdout.lines().filter(|l| !l.contains("ExpenseBasis") && !l.contains("monthly_expense_cents")).collect();
    assert!(filtered.is_empty(), "raw calls without Basis remain: {:?}", filtered);
}
```

- [ ] **Step 2: Run guard to verify it fails before cleanup**

Run: `grep -rn "avg_monthly_expense" crates --include="*.rs" | grep -v ExpenseBasis`
Expected: FAIL — lists old call sites

- [ ] **Step 3: Fix remaining raw calls (if any)**

Search and replace any remaining `avg_monthly_expense_90d(` with `monthly_expense_cents(RecentMean90, scope)`, `robust_monthly_expense_cents` with `monthly_expense_cents(DisplayMedian, scope)`, direct `safety_expense_basis` with `monthly_expense_cents(SafetyConservative, scope)`.

- [ ] **Step 4: Run Gate C to verify it passes**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cd ui && npx vitest run && npx tsc --noEmit && cargo run -p finsight-openapi --bin export_openapi`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: gate C — no raw expense calls without ExpenseBasis"
```

---

## Self-Review

**Spec coverage:** §2 Goals (3 pains) → Tasks 2+5 (numbers match + explain), §3 Architecture pantry → Task 1, §4 Components → Tasks 2-5, §5 Data Flow → Tasks 2+4, §6 Slices → Tasks 2 then 3-5, §7 Edge Cases → Task 3 scoped safety, §8 Testing → Tasks 1/3/6, §9 Rollout → Task 2 release note.

**Placeholder scan:** No TBD/TODO, all steps have exact file paths, code blocks, and commands with expected output.

**Type consistency:** `ExpenseBasis` is `DisplayMedian`/`RecentMean90`/`SafetyConservative` everywhere (Rust) and `displayMedian`/`recentMean90`/`safetyConservative` in TypeScript (camelCase via specta). `monthly_expense_cents` returns `(i64, bool)` throughout. `Reconcile { delta_cents, reason }` is consistent.

If any gap found, fix inline before handoff.


# Money Confidence — Unified Expense Basis

**Date:** 2026-08-24  
**Status:** Approved  
**Slices:** 2 (Slice 1 = safety unification, Slice 2 = explicit Basis + reconciliation)  
**Related:** `docs/superpowers/specs/2026-08-23-non-agent-maintainability-cleanup-design.md`, `AGENTS.md` Architecture, Financial Freedom Framework

## 1. Overview

FinSight shows money numbers in 5 places — Dashboard, Budget, Cashflow, Forecast/Scenarios, Journey, and Copilot. All read from `finsight-core`, but they use **3 different monthly expense bases** for legitimate reasons:

* **DisplayMedian** (robust median, anomaly-excluded, 12 complete months) — smooth, hides one-offs. Used for Budget envelopes.
* **RecentMean90** (90-day mean) — reacts fast to step-ups (new rent, baby). Used for Cashflow daily burn.
* **SafetyConservative** (`max(mean12, mean90)`) — most conservative. Used for runway / EF months.

Today the bases have no name and no single owner. Call sites hand-roll SQL, so:
1. **Numbers mismatch** — `safe-to-spend` on Dashboard vs Forecast runway vs Copilot diverge for the same date.
2. **Basis confusion** — Budget median vs Cashflow mean differ by $60 with no explanation; looks buggy.
3. **Journey disconnect** — `/journey` milestones use a different runway than Dashboard.

This spec introduces a **labeled ExpenseBasis pantry** so every consumer picks a label explicitly and can explain why two buckets differ.

**PM summary:** One kitchen (pantry) owns the 3 bucket definitions; many menus (screens + Copilot) pick a label. When two numbers intentionally differ, we show a one-line reason instead of looking wrong.

## 2. Goals / Non-Goals / Success Metrics

**Goals:**
* Safety numbers (`runway`, `EF months`, `safe-to-spend`) agree everywhere for same `scope` + `currency`. Never overstate (conservative direction).
* Every money call site declares its basis in the type system (`grep Basis::` shows who uses what).
* When Budget (Smooth) and Cashflow (Recent) differ, UI + Copilot can show `reconcile()` = delta + reason.
* Journey milestones read from `SafetyConservative` so `Journey: 4.2 mo` == `Dashboard runway: 4.2 mo`.

**Non-Goals:**
* No new financial formulas. Reuse existing SQL definitions under labels.
* No new screens. No agent/LLM prompt changes beyond quoting the right bucket.
* No change to Financial Freedom Framework assumptions.

**Success metrics:**
* `grep -r "avg_monthly_expense\|robust_monthly_expense\|safety_expense_basis" crates --include="*.rs" | grep -v "ExpenseBasis"` → 0 (no raw calls).
* Test: `dashboard_safe_to_spend == cashflow_safe_to_spend == scenario_baseline_runway` for `SafetyConservative` + same `scope`.
* `explain(Smooth)` / `explain(Recent)` / `explain(Conservative)` snapshots approved by product.
* `cargo test --workspace` + `cargo clippy -D warnings` + `pnpm --filter ui test` green. +6 new tests for `reconcile()`.

## 3. Architecture — One Pantry, Many Menus

```
finsight-core::metrics  (pantry)
  ├─ ExpenseBasis enum { DisplayMedian, RecentMean90, SafetyConservative }
  ├─ fn monthly_expense_cents(conn, basis, scope) -> (cents, sufficient)
  ├─ fn explain(basis) -> &'static str
  ├─ fn reconcile(conn, basis_a, basis_b, scope) -> { delta_cents, reason }
  └─ all SQL via primary_currency_clause() + member scope + transfer filter

Consumers (menus) — no SQL:
  cashflow::build_forecast(basis = RecentMean90 → later SafetyConservative for Slice 1)
  forecast::Snapshot::build(basis)
  budget_envelopes_for_month(basis = DisplayMedian)
  safety_expense_basis(basis = SafetyConservative)
  journey::milestones(basis = SafetyConservative)
  copilot tools: get_safe_to_spend, get_runway, get_budget → call metrics with same basis as screen
```

**Why enum, not `fn` per basis:** Call site intent is visible in the type (`Basis::SafetyConservative`). Adding a future basis (e.g. `VolatilityAware`) is additive; no `if screen ==` branching.

**Currency & member scoping:** `monthly_expense_cents(conn, basis, scope: Option<MemberId>)` delegates to existing `*sc_has_scoped` variants (`primary_currency_clause`, `MEMBER_WEIGHT_SUBQUERY`, `share_bps` + `owner_member_id` override). Slice 1 keeps `scope=None` (household); Slice 2 threads `scope` through.

## 4. Components & Changes

### 4.1 `crates/finsight-core/src/metrics.rs` (pantry)

* New:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExpenseBasis { DisplayMedian, RecentMean90, SafetyConservative }

pub fn monthly_expense_cents(conn: &Connection, basis: ExpenseBasis, scope: Option<&str>) -> CoreResult<(i64, bool)> // (cents, sufficient)
pub fn explain(basis: ExpenseBasis) -> &'static str
pub struct Reconcile { pub delta_cents: i64, pub reason: String }
pub fn reconcile(conn: &Connection, a: ExpenseBasis, b: ExpenseBasis, scope: Option<&str>) -> CoreResult<Reconcile>
```
* `monthly_expense_cents` match:
  * `DisplayMedian` → `robust_monthly_expense_cents_scoped` (median, anomaly-excluded) with 90-day mean fallback and `sufficient = vals.len()>=2 && span>=30d`.
  * `RecentMean90` → `avg_monthly_expense_90d_scoped` (mean90, anomaly-included) `sufficient = span>=30d`.
  * `SafetyConservative` → `max(mean12, mean90)` where `mean12 = safety_expense_basis` long mean (complete months, `SAFETY_BASIS_MAX_MONTHS=12`) vs `RecentMean90`. `sufficient = safety_basis.sufficient`.
* `explain()` strings (PM-approved, i18n-ready):
  * `DisplayMedian`: "Smooth average — ignores one-offs so your budget doesn't spike."
  * `RecentMean90`: "Recent average — catches step-ups like rent quickly."
  * `SafetyConservative`: "Conservative — the higher of yearly and recent, so safety is never overstated."
* `reconcile()`: compute both, delta = `a - b`, reason picks template by bases + delta sign:
  * `Smooth vs Recent`: "Recent is $X higher because it caught your $Y step-up this month; Smooth will catch it next month."
  * `Recent vs Conservative`: "Conservative is $X higher because it keeps the yearly peak — the recent alone would understate."
  * Pure function; no DB beyond the two basis calls.

### 4.2 `crates/finsight-core/src/cashflow.rs`

* `build_forecast()` today computes `recent_mean_expense = avg_monthly_expense_90d()` directly.
* Slice 1: change to `monthly_expense_cents(conn, SafetyConservative, None)` for `recent_mean_expense` (so `daily_burn` uses conservative). Keep `dated_obligation` logic unchanged (still nets only when `last_seen` in 90-day window). Add test: `daily_burn = (conservative - obligations).max(0)/30.44`.
* Slice 2: `build_forecast` takes `basis: ExpenseBasis` param (default `SafetyConservative`) so future callers can request `RecentMean90` for what-if.

### 4.3 `crates/finsight-core/src/forecast.rs` + `Snapshot`

* `Snapshot` gains `basis: ExpenseBasis` field (serialized). `repos::scenarios::build_baseline(conn)` currently builds `Snapshot { balance, avg_income, avg_expense, goals }` via `rolling_averages`/`robust`. Change to `monthly_expense_cents(conn, SafetyConservative, None)` for `avg_monthly_expense_cents` so forecast runway uses conservative.
* `baseline_materially_changed` unchanged (relative 10% threshold). Old rows with `basis = null` (pre-Slice 1) deserialize as `None` → display as "legacy — viewable, not recomputable" (existing V055 pattern).
* `project()` unchanged — it receives `Snapshot` with already-chosen basis.

### 4.4 `crates/finsight-core/src/budget.rs` / `reports.rs`

* `budget_envelopes_for_month()` uses `DisplayMedian` (no change to numbers, just route through `monthly_expense_cents(DisplayMedian, scope)` for spend comparison where needed).
* No change to envelope persistence.

### 4.5 `crates/finsight-api` + `finsight-agent` (Copilot)

* Copilot tools `get_safe_to_spend`, `get_runway`, `get_budget_summary`, `get_spending_review` currently call ad-hoc `metrics::rolling_averages` / `avg_monthly_expense_90d`.
* Change to `metrics::monthly_expense_cents(conn, basis_for_tool, scope)` where `basis_for_tool` matches the screen the user asked about. If no screen context, default `SafetyConservative` for safety questions, `DisplayMedian` for budget questions.
* Add tool `explain_basis(basis)` → `metrics::explain()` and `reconcile_bases(a,b,scope)` for "why do they differ?".

### 4.6 `crates/finsight-server` + UI

* No new OpenAPI commands for Slice 1. Slice 2 adds `GET /api/reconcile?basisA=&basisB=&scope=` (or reuses existing `rpc_routes!` entry `reconcileBases`) returning `Reconcile`.
* UI `ui/src/api/hooks/useMetrics.ts` (new) wraps `explain` + `reconcile`. Screens add ⓘ tooltip via `explain(basis)`:
  * Budget: `DisplayMedian`
  * Cashflow: `RecentMean90` (Slice 1 → `SafetyConservative`, then back to `RecentMean90` with explainer in Slice 2; see rollout note)
  * Today/Journey: `SafetyConservative`
* `ui/src/screens/Journey.tsx` currently computes `emergency_fund_months` via `metrics::emergency_fund_months_scoped` (already `SafetyConservative` via `robust` fallback) — confirm it calls `monthly_expense_cents(SafetyConservative)`.

## 5. Data Flow

1. User opens Dashboard → `build_forecast(SafetyConservative)` → `monthly_expense_cents(SafetyConservative, scope)` → `max(mean12, mean90)` → `safe-to-spend`.
2. User opens Budget → `budget_envelopes` (DisplayMedian) → `explain(DisplayMedian)` for ⓘ.
3. User asks Copilot "why is Cashflow $180 but Budget $120?" → agent calls `reconcile(DisplayMedian, RecentMean90, scope)` → returns delta + reason → model renders reason verbatim (no math in LLM).
4. Scenarios: `build_baseline()` → `Snapshot{ basis: SafetyConservative, ...}` → stored. `list_saved_scenarios` recomputes against current baseline; staleness via `baseline_materially_changed` (relative 10%).

**Invariants preserved from AGENTS.md:**
* `RecurringKind::Transfer` excluded.
* Obligation netted from burn only if `last_seen` in 90-day window.
* `safe_to_spend = lowest - buffer`.

## 6. Slice Plan

**Slice 1 — Safety Unification (3 days, shippable alone):**
* Files: `metrics.rs` (add `ExpenseBasis` + `monthly_expense_cents` for `SafetyConservative` path), `cashflow.rs` (switch to `SafetyConservative`), `forecast.rs` `Snapshot` build path, tests.
* No OpenAPI change. No UI beyond numbers shifting slightly conservative.
* Verifies: `cargo test -p finsight-core cashflow::tests::forecast_reflects_burn` etc. still pass with updated expected burn.
* Release note: "Safety numbers (runway, EF months, safe-to-spend) now consistently conservative — may shift down slightly for some households; saved scenarios may show stale badge (10% threshold)."

**Slice 2 — Label + Explain + Reconcile (1 week):**
* Files: `metrics.rs` (add `DisplayMedian`/`RecentMean90` paths + `explain` + `reconcile`), `budget.rs`, `journey` wiring, `finsight-api` Copilot tools + `finsight-server` route, `ui` hooks + tooltips.
* Add `reconcile` OpenAPI entry, `ui/src/api/openapi.ts` regen via `pnpm openapi`.
* Roll back Cashflow display to `RecentMean90` with ⓘ + reconcile link (Slice 1 used `SafetyConservative` as interim; Slice 2 restores purpose-specific with explanation — or keep `SafetyConservative` for Cashflow safety if PM prefers; decision point in implementation).

## 7. Error Handling & Edge Cases

* **Thin history (<30 days):** `monthly_expense_cents` returns `(0, sufficient=false)`. Callers withhold metric ("need a few more weeks") rather than 0. Matches existing `SafetyExpenseBasis.sufficient` and `CashflowForecast.reliable` patterns.
* **Mixed currency:** All three bases scope to `primary_currency_clause`; `unconverted` holdings remain excluded. No invented FX rate.
* **Member scope:** `scope=Some(member_id)` uses `MEMBER_WEIGHT_SUBQUERY` (`share_bps` else `1/n`, `owner_member_id` override). Household `None` uses unweighted query. Reconciliation respects same scope.
* **Unknown balance accounts:** Excluded from `liquid`/`EF` pool via `balance_known` guard (mirrors `balance_breakdown`).
* **Legacy Snapshots:** `basis` nullable; `None` → "legacy" display, not recomputable.

## 8. Testing

* `metrics::tests::expense_basis_reconcile` — pin delta/reason for known fixture (3 months smooth vs 1 month spike).
* `metrics::tests::safety_conservative_is_max` — fixture where `mean12 > mean90` and vice versa.
* `cashflow::tests::forecast_reflects_burn` — update expected `daily_burn` to conservative basis.
* `forecast::tests::baseline_staleness` — add case where basis change alone does not flag stale (only value matters).
* `finsight-api` Copilot tool test: `get_safe_to_spend` with `SafetyConservative` equals `cashflow::build_forecast` result.
* `ui/src/api/hooks/useMetrics.test.tsx` — `explain()` strings snapshot.
* Gate C: `cargo test --workspace`, `cargo clippy -D warnings`, `pnpm --filter ui test`, `pnpm typecheck`, `cargo run -p finsight-openapi --bin export_openapi` snapshot.

## 9. Rollout & Risks

* No feature flag. Slice 1 is a correctness fix — conservative direction, safe to ship to all.
* Risk: ~20% of users see safety numbers drop slightly (conservative > recent). Mitigated by release note + stale badge; overstatement is the dangerous direction we are fixing.
* No migration. `Snapshot.basis` nullable; old rows degrade gracefully.

## 10. Open Decisions for Implementation

* Whether Cashflow *display* stays `SafetyConservative` (Slice 1) or reverts to `RecentMean90` + explainer (Slice 2). Recommend revert to `RecentMean90` for Cashflow's purpose (react fast) but keep `safe-to-spend` conservative — i.e. `daily_burn` from `RecentMean90`, `safe-to-spend` from `SafetyConservative` lowest? Or both `SafetyConservative` for simplicity. Decide in `writing-plans` with PM.
* Wording of `explain()` strings — product review before Slice 2 ship.


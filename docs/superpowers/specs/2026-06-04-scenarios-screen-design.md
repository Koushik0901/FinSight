# Scenarios screen — design spec

**Date:** 2026-06-04
**TODO item:** §1 (Scenarios screen) + §15a (Scenarios in nav)
**Status:** Approved, ready for implementation plan

## Summary

The Scenarios screen is a natural-language what-if planner. The user asks a
question ("What if I take a 6-month sabbatical?") or picks a preset chip, and the
app re-projects their cash-flow trajectory, reports whether the move is coverable,
how runway changes, which goals are affected, and a list of considerations.

It is currently absent from FinSight — no route, no nav entry, no backend.

## Guiding constraint

The configured AI provider can be `Unconfigured` in a default install. Therefore
**scenario numbers must never depend on the LLM.** All math (verdict, runway delta,
trajectories, considerations) is computed by one deterministic engine. The LLM is
used *only* to parse free-text input into parameters, and that path degrades
gracefully when no provider is set.

## Architecture

```
chips ─────────────────────┐
                            ├──► ScenarioParams ──► projection engine ──► ScenarioResult
free text ──(LLM extract)───┘     (small, validated)    (pure, deterministic)
```

- **Chips** build `ScenarioParams` in the frontend and pass them to the backend
  directly. No LLM, no interpretation step.
- **Free text** is the only path that touches the LLM, and only to fill
  `ScenarioParams`. If the provider is `Unconfigured`, the command returns a typed
  error that the UI renders as a friendly nudge ("Configure an AI provider in
  Settings, or pick a scenario below"). Chips keep working regardless.
- **All numbers are deterministic** — computed from `ScenarioParams` plus a
  financial snapshot (account balance, 12-month net history, goals).

### `ScenarioParams` (the extension seam)

Models exactly the four chip archetypes plus generic deltas — nothing more (YAGNI).

```rust
struct ScenarioParams {
    income_delta_pct: i32,            // "Cut income 50%" → -50
    monthly_expense_delta_cents: i64, // "Eliminate dining" → -<dining avg>;
                                      // "Add $500/mo to savings" → +50000 (treated as outflow)
    one_time_cents: i64,              // "Buy a car $35k" → 3500000
    start_month_offset: u32,          // month the change kicks in (default 0)
    label: String,                    // echoed back for display
}
```

The four preset chips map to:

| Chip | Params |
|------|--------|
| Cut income 50% | `income_delta_pct: -50` |
| Eliminate dining out | `monthly_expense_delta_cents: -<dining monthly avg>` |
| Buy a car $35k | `one_time_cents: 3500000` |
| Add $500/mo to savings | `monthly_expense_delta_cents: 50000` |

## Backend

### Core (`finsight-core`)

**`forecast.rs`** (new shared module, pure functions, no DB):

- `runway_days(balance_cents: i64, expenses_this_month_cents: i64, day_of_month: u32) -> i64`
  — the canonical formula, identical to TODO §3d so the future Today runway stat
  reconciles: `avg_daily_burn = expenses_this_month / day_of_month`,
  `runway_days = balance / avg_daily_burn`. Returns a large sentinel (or caps) when
  burn is zero.
- `project(snapshot: &Snapshot, params: &ScenarioParams, months: u32) -> Projection`
  — produces `baseline_monthly` and `scenario_monthly` (cumulative net per month),
  the runway delta, the monthly impact, and goal-slip strings. Pure and unit-tested.

`Snapshot` carries: current total balance, average monthly income, average monthly
expense (for baseline burn), per-goal `(name, target, current, monthly, target_date)`,
and the dining (or other) category monthly average where needed for chip resolution.

**`repos/scenarios.rs`** — CRUD for the `scenarios` table: `insert`, `list`, `delete`.

### App commands (`crates/finsight-app/src/commands/scenarios.rs`)

- `run_scenario(description: String, months: u32, params: Option<ScenarioParams>) -> ScenarioResult`
  - Gathers the snapshot: balance via `accounts::list_summaries`, 12-month net via
    the existing report monthly query, goals via `goals::list`.
  - If `params` is `Some` (chip path): use them directly.
  - If `params` is `None` (free-text path): call the live LLM provider
    (`state.agent_provider`) to extract `ScenarioParams` from `description`. If the
    provider is `None`, return `AppError` with a recognizable code so the UI shows
    the configure-provider nudge.
  - Runs `forecast::project`, builds and returns `ScenarioResult`.
  - **Not persisted** — matches the "nothing happens to your real money" copy.
- `save_scenario(description: String, result: ScenarioResult) -> SavedScenario`
- `list_scenario_history() -> Vec<SavedScenario>`
- `delete_scenario(id: String)`

Register all four in `build_specta_builder()` in `crates/finsight-app/src/lib.rs`,
then regenerate bindings: `cargo run -p finsight-tauri --bin export_bindings`.

### Result types

```rust
struct ScenarioResult {
    verdict: bool,                 // can the user cover this?
    runway_change_days: i64,
    monthly_impact_cents: i64,
    considerations: Vec<String>,   // templated from computed numbers, NOT LLM-invented
    baseline_monthly: Vec<i64>,    // N months of cumulative net (current trajectory)
    scenario_monthly: Vec<i64>,    // N months of cumulative net (scenario trajectory)
    goals_affected: Vec<String>,   // e.g. "House Fund: +2 mo" for the impact grid
}

struct SavedScenario {
    id: String,
    description: String,
    result: ScenarioResult,        // stored as result_json
    created_at: String,
}
```

**Considerations** are templated from the deterministic numbers — for example:
"Runway shortens by N days", "Emergency fund drops to ~X months of expenses at
peak", "Goal Y's ETA slips by Z months". They never reference data the model
does not hold (no invented people/jobs as in the mock). LLM phrasing enrichment is
explicitly out of scope for v1.

## Frontend

### `ui/src/screens/Scenarios.tsx`

- Header: eyebrow "Scenarios", title, intro paragraph.
- Big ask input: `I.Sparkle` + text field + Run button (`btn primary`).
- Suggested chips row: the four presets, each carrying its `ScenarioParams`.
- Results panel (after submit):
  - Verdict card — green ("Coverable") / red ("Not coverable") gradient with
    explanation sentence.
  - Impact grid — three stats: runway change, monthly impact, goals affected.
  - SVG dual-line chart — solid baseline vs dashed accent scenario, X = months,
    Y = cumulative net, "TODAY"/"scenario starts" marker. Reuse the NetLine SVG
    approach from `Reports.tsx`. Forecast-range toolbar: 6M / 12M / 24M.
  - Numbered considerations list.
  - Action row: "Save scenario", "Discard".
- History sidebar: saved scenarios with description, verdict chip, date, "Re-run".
- Free-text + no-provider: render the configure-provider nudge inline instead of
  results.

### `ui/src/api/hooks/useScenarios.ts`

tanstack-query wrappers: `useScenarioHistory`, `useRunScenario` (mutation),
`useSaveScenario` (mutation, invalidates history), `useDeleteScenario` (mutation,
invalidates history).

### Routing / nav (§15a folded in)

- Add `/scenarios` route in `App.tsx`.
- Add nav entry in `Sidebar.tsx` `NAV_MAIN` and `routes.ts`, positioned between
  Goals and Reports, using `I.Bolt`.

## Migration

**V005** `crates/finsight-core/migrations/V005__scenarios.sql`:

```sql
CREATE TABLE scenarios (
  id          TEXT PRIMARY KEY,
  description TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at  TEXT NOT NULL
);
```

## Testing

- **Rust unit tests** for `forecast::project` and `runway_days`:
  - coverable scenario, not-coverable scenario
  - income cut (`income_delta_pct`)
  - one-time purchase (`one_time_cents`)
  - recurring savings/expense delta
  - zero-burn edge case (no divide-by-zero)
- **Rust repo test:** `scenarios` insert → list → delete round-trip.
- **Frontend test** `Scenarios.test.tsx`: render with mocked hooks; chip click
  produces a results panel; history re-run calls the run mutation.
- Keep the green bar: `cargo test --workspace`, `cd ui && npx vitest run`,
  `cd ui && npx tsc --noEmit`.

## Out of scope (v1)

- The mock's "Add the constraints to your forecast" and "Set a reminder" buttons
  (no backend exists for either).
- LLM-authored prose considerations (deterministic templates only).
- Multi-currency handling beyond the existing app-wide default.

# Budgets page redesign — design

- Date: 2026-07-15
- Status: approved (proceeding under `/goal` auto-mode — see §0), pending spec self-review → implementation plan
- Origin: user request via `/goal` — "too hard to set categories, monthly budgets, see my spending on normal months on a category and compare it with my spending on this month," wants something in the spirit of Actual Budget's Budget page. A full mockup exists in the Claude Design project "Plutus" (`fdbc4798-c6d0-41df-9499-e6ca4294d142`), which is also the same project the app's current design tokens/icon set were ported from in an earlier session.

## 0. Process note

This spec was presented in chat (architecture, data model, testing, Linear plan) and the three real forks — rollover semantics, category-management scope, and Plan Next Month scope — were resolved via an explicit `AskUserQuestion` round:

1. **Rollover:** automatic for all categories (not per-category opt-out, not skipped).
2. **Categories:** fix in place, keep Budget and Categories as separate screens (not a merge).
3. **Plan Next Month:** full rebuild to the mockup's 7-step flow (not "keep + polish").

Two remaining implementation-level interpretive calls (buffer semantics, sinking-funds-as-goal-type) were flagged to the user for a final sanity check; no blocking response arrived (auto-mode session, no interactive turn available), so this spec proceeds with the stated defaults, each justified below on its own merits. Anyone reviewing this doc can override either call — they're isolated enough to change without touching the rest.

## 1. Problem

The current Budget screen (`ui/src/screens/Budget.tsx`) already implements a hero "month progress" card, a "To Budget" unassigned-income bar, a "needs a glance" row, a grouped/sortable envelope grid, a spending-history table, and a "Plan next month" wizard. It is not a blank slate. But four concrete gaps cause the user's exact complaints:

1. **"Hard to set monthly budgets."** `list_budget_envelopes` (`crates/finsight-app/src/commands/budget.rs:25-77`) only returns categories where `budget_cents > 0 OR spent_cents > 0`. A freshly created category with no spend yet is invisible on the Budget screen — there is no way to assign it a budget until it happens to get a transaction first.
2. **"Hard to set categories."** `repos::categories::list_groups()` exists (`crates/finsight-core/src/repos/categories.rs:6-23`) but has no Tauri command, no binding, no hook. New categories always fall into "first group by sort order" (`repos/categories.rs:88-99`); there is no UI to create a group or move a category between groups.
3. **"Compare spending this month vs normal months."** `list_budget_history` (`crates/finsight-app/src/commands/budget.rs:602-718`) returns actuals-only per month — no budgeted amount alongside it, no baseline/"typical" reference.
4. **No rollover.** The `budgets` table (`crates/finsight-core/migrations/V004__budgets_goals.sql`) is `(category_id, month, amount_cents)` — one flat number per category per month. There is no carryover concept anywhere in the codebase (confirmed by grep across migrations, repos, and commands).

## 2. Goals / non-goals

Goals:
- Every active category is visible and budgetable on the Budget screen, budgeted or not.
- Unspent budget rolls forward as a bonus; overspend rolls forward as a deficit — computed, not manually tracked.
- Users can create/rename category groups and assign categories to them, from the UI.
- The spending-history table shows budgeted vs. spent per month, so "normal" is visible next to "this month."
- Plan Next Month becomes a 7-step guided flow: look back, fixed costs, sinking funds, buffer, goals, adjust, review.

Non-goals (this pass):
- **No Categories↔Budget screen merge.** Explicitly decided against (user chose "keep separate"); the app's own `docs/design/2026-07-14-ui-redesign-direction.md` already flags this as a deferred, higher-risk idea — not reopened here.
- **No per-category rollover opt-out.** Explicitly decided against (user chose "automatic for all"). A per-category toggle is a natural fast-follow if a real use case appears (e.g. reimbursable categories), but nothing in this pass blocks adding one later.
- **No household/per-person budget split.** Already a settled v1 scope call (`docs/audits/2026-07-10-finsight-product-audit.md:174`) — not reopened.
- **No LLM-generated "look back" narrative.** Deterministic Rust facts only (§6.3) — this is a wizard step, not a Copilot surface.
- **No persisted "buffer" field.** See §6.2 — it's wizard-time math, not a stored concept.
- **No true multi-month grid/spreadsheet view** (the mockup's own comment: "not a multi-month spreadsheet"). The Actual Budget inspiration is about *capability* (rollover, comparison, easy budget-setting), not that specific UI paradigm.

## 3. Data model changes

### 3.1 `BudgetEnvelope` gains `carryoverCents`

```rust
// crates/finsight-app/src/commands/budget.rs
pub struct BudgetEnvelope {
    pub category_id: String,
    pub category_label: String,
    pub category_color: String,
    pub group_label: String,
    pub budget_cents: i64,
    pub spent_cents: i64,
    pub carryover_cents: i64,   // NEW
    pub txn_count: i64,
}
```

`carryover_cents` = running sum of `(budgeted − spent)` per month, starting at the **first month that category has a `budgets` row with `amount_cents > 0`**, up to (not including) the current month, capped at a 24-month lookback (same clamp convention as `list_budget_history`). See §6.1 for why this epoch, not "since account inception."

Available-to-spend for an envelope becomes `budget_cents + carryover_cents`; `remaining = available − spent_cents`. **"To Budget" (unassigned income) is unaffected** — it stays `income − Σ budget_cents` (fresh assignment only; carryover is prior-period money, not this period's income).

### 3.2 `list_budget_envelopes` stops filtering out zero-budget categories

Current query only includes categories where `budget > 0 OR spent > 0`. New behavior: every active (`archived_at IS NULL`) category is returned, budget/spent/carryover defaulting to 0. The Budget screen gets a new lightweight grouping for these ("Not yet budgeted") so they're visible but don't clutter the main grid — see §7.1.

### 3.3 `MonthlyActual` gains `budgetedCents`

```ts
// currently: { month: string; label: string; cents: number }
export type MonthlyActual = { month: string; label: string; spentCents: number; budgetedCents: number };
```

`list_budget_history` adds a join against `budgets` per month (it currently only aggregates `transactions`). The frontend computes a trailing average client-side from the returned months — no new "typical" field on the backend; the data needed is already in the array once `budgetedCents` exists.

### 3.4 Category groups — no migration needed

`category_groups (id, label, hint, sort_order)` already exists (V001); `repos::categories::create()` already accepts `group_id: Option<&str>`. Adding:

```rust
// crates/finsight-core/src/repos/categories.rs
pub fn create_group(conn: &mut Connection, label: &str, hint: Option<&str>) -> CoreResult<CategoryGroup>;
pub fn set_group(conn: &mut Connection, category_id: &str, group_id: &str) -> CoreResult<()>;
```

`create_group` mirrors `create()`'s slug-id-with-dedup pattern; `set_group` is a one-line `UPDATE categories SET group_id = ?1 WHERE id = ?2` (validate the group exists first — see §8 error handling).

### 3.5 Goals gain a `"sinking-fund"` goal_type

No migration — `goals.goal_type` is a plain `TEXT` column with no `CHECK` constraint (confirmed against every migration touching `goals`). `"sinking-fund"` is purely an application-level addition, validated the same ad-hoc way the other four `goal_type` values already are (there is no central enum today; `Goals.tsx` hardcodes the four strings in `TYPE_LABELS`/`GoalFilter` — this pass adds a fifth in the same places).

## 4. Backend commands

New or changed, all in `crates/finsight-app/src/commands/`:

| Command | Change |
|---|---|
| `list_budget_envelopes` | Include zero-budget categories; compute + return `carryover_cents` |
| `list_budget_history` | Join `budgets` per month; return `budgeted_cents` alongside `spent_cents` |
| `list_category_groups` | **New.** Thin wrapper over `repos::categories::list_groups` (already exists, just never wired to a command) |
| `create_category_group` | **New.** `(label, hint) -> CategoryGroup` |
| `set_category_group` | **New.** `(category_id, group_id) -> ()` |
| `get_plan_next_month_data` | Extended: add `look_back: Vec<LookBackFact>` (§6.3), `sinking_funds: Vec<GoalDto>` (goals filtered by `goal_type == "sinking-fund"`) |
| `create_goal` | No signature change — `"sinking-fund"` flows through the existing free-string `goal_type` param |

Every new/changed command needs `cargo run -p finsight-tauri --bin export_bindings` after implementation, per the project's standard flow.

## 5. Frontend changes

### 5.1 `Budget.tsx`
- `envelopeStatus()` and `EnvelopeCard` use `budgetCents + carryoverCents` as "available" for remaining/severity math.
- `EnvelopeCard` gains a "Carried from {prior month} ±$X" line (mirrors the mockup), shown only when `carryoverCents !== 0`.
- New "Not yet budgeted" section: zero-budget-and-never-spent categories, deprioritized below the main grid, each with a one-click "Set budget" CTA — this is the direct fix for complaint #1.
- Spending-history table adds a second row per category (budgeted vs. spent) or a combined bar, plus a computed trailing average shown as a reference line/label ("your typical: $X").

### 5.2 `Categories.tsx`
- "New category" form gains a group `<select>` (populated by `useCategoryGroups`), replacing the hardcoded `groupId: null`.
- An inline "+ New group" affordance (name + optional hint) calling `useCreateCategoryGroup`.
- Manage panel gains a "Move to group" control calling `useSetCategoryGroup`.

### 5.3 `PlanNextMonthModal.tsx` — full rebuild to 7 steps

Replacing the current `["Income","Essentials","Wants","Goals","Recurring","Review"]` with `["Look back","Fixed costs","Sinking funds","Buffer","Goals","Adjust","Review"]`. Per-step behavior:

1. **Look back** — deterministic facts from `get_plan_next_month_data.look_back` (§6.3): biggest overage, biggest underage, longest zero-spend streak.
2. **Fixed costs** — same as current "Essentials" (categories whose group is fixed), pre-filled with 3-month average.
3. **Sinking funds** — new step: lists goals with `goal_type == "sinking-fund"`, a monthly-contribution slider per fund (mirrors current per-category number inputs pattern).
4. **Buffer** — a slider with no persisted backing (§6.2); purely reduces the running "planned" total for this session, growing next month's "To Budget."
5. **Goals** — same as current "Goals" step, excluding sinking funds (they have their own step now).
6. **Adjust** — new step: surfaces 2-3 deterministic suggestions (e.g., "you hit this category's cap 3 of the last 4 months — raise it?") the user can accept/skip; no LLM.
7. **Review** — same as current "Review," extended to show sinking-fund and buffer lines in the summary.

The live preview panel (`renderPreview`) extends its stacked-bar segments to include sinking funds and buffer. `usePlanNextMonthData`/`useApplyNextMonthPlan`'s contracts extend (don't replace) — `apply_next_month_plan` still takes category-id → amount assignments; sinking-fund contributions go through the existing goal-contribution path, not a new mutation.

### 5.4 `Goals.tsx`
- `GoalFilter` union, `TYPE_LABELS`, and the filter-button row gain `"sinking-fund"` → "Sinking fund".
- `CompoundGrowthProjector` eligibility filter excludes `"sinking-fund"` alongside the existing `"spending-cap"` exclusion (a multi-decade compounding projection is meaningless for a $480, 6-month car-insurance fund).
- Pause/resume eligibility (`canPause`) needs no change — sinking funds fall through to the default "can pause" bucket, same as `build-balance`.

## 6. Interpretive calls, explained

### 6.1 Carryover epoch: "first budgeted month," not "since inception"

Summing `(budgeted − spent)` over *every* month since a category's first transaction would surface a large, meaningless accumulated deficit for any category that was spent on for years before ever being budgeted — a scary, wrong number on the day this ships. Anchoring the running sum to the category's first `budgets` row with `amount_cents > 0` means carryover only ever reflects money the user actually earmarked. This is a correctness decision, not a style preference — it's the difference between "your budgeting history" and "your entire spending history," and only the former is what "carryover" means.

### 6.2 Buffer: wizard-time math, not a stored field

The mockup's buffer step is a slider contributing to a "planned" sum (`fixed + sinks + buffer + goals + daily`), with `remaining = income − planned`. Nothing in the mockup persists "buffer" anywhere independent of that sum. Giving it a dedicated DB column/goal would be inventing a stored concept the mockup itself doesn't have — the honest read is that buffer is a deliberate-slack decision aid: it reduces how much of this session's income gets assigned, and the reduction shows up naturally as next month's unassigned "To Budget." No new schema; no new mutation.

### 6.3 Look-back facts: deterministic, not LLM

The mockup shows example facts like "Dining ran $12 over budget" and "Travel sat at $0 — fourth month in a row." All of these are simple comparisons already computable from `list_budget_history` + `list_budget_envelopes` data (biggest overage = max(spent − budget) last month; biggest underage = max(budget − spent); zero-spend streak = consecutive trailing months with spent == 0). Routing this through the LLM/Copilot layer would add a dependency, latency, and non-determinism to what is fundamentally a wizard step, not a chat surface — and the app's own convention (per `crates/finsight-agent` grounding pattern) is that numbers shown to the user are server-synthesized, not model-trusted. Plain Rust wins on every axis here.

### 6.4 Sinking funds as a `goal_type`, not a new table

A sinking fund (car insurance due in 4 months, target $480, current $200) is structurally identical to a `build-balance` goal — target amount, current amount (ledger-derived), monthly contribution, optional target date. Reusing the existing `goals` table + `goal_contributions` ledger avoids a parallel CRUD/contribution system for a concept that's a naming/UI distinction, not a data-shape distinction.

## 7. Edge cases / error handling

### 7.1 "Not yet budgeted" section sizing
If a household has many uncategorized/unbudgeted categories, the new "show everything" behavior in `list_budget_envelopes` could flood the grid. Mitigation: the "Not yet budgeted" group is visually deprioritized (collapsed-by-default or listed after all budgeted groups) rather than interleaved — most households will see this section shrink to nothing after one budgeting pass.

### 7.2 `set_category_group` on a nonexistent group
Validate `group_id` exists before the `UPDATE`; return a `validation` `AppError` if not (matching the existing pattern in `commands/transactions.rs::set_category_spending_type`'s whitelist check).

### 7.3 Carryover for an archived-then-unarchived category
Out of scope for this pass — carryover simply resumes computing from whatever `budgets` rows exist; archiving doesn't delete history, so no special-casing is needed.

### 7.4 Negative carryover compounding indefinitely
By design (matches Actual/YNAB): if a category is chronically over budget, its deficit keeps growing until the user raises the budget or a "cover from another envelope" action (already exists in the UI, currently a suggestion only — money doesn't actually move) is taken. Not new behavior to design around; it's the intended signal.

## 8. Testing

- `crates/finsight-core/src/repos/budgets.rs` currently has **zero tests**. Add unit tests for the carryover running-sum math: first-budgeted-month anchoring, 24-month cap, negative (overspend) carryover, a category with gaps in its budgeted months.
- `list_budget_envelopes` / `list_budget_history` Rust command tests (none exist today) for: zero-budget categories now appearing, `carryover_cents` and `budgeted_cents` wiring.
- `repos::categories`: tests for `create_group` (slug dedup, mirroring existing `create()` tests) and `set_group` (happy path + invalid group id).
- Frontend: extend `Budget.history.test.tsx` for the budgeted-vs-spent history table and the "Not yet budgeted" section; extend `Categories.test.tsx` for group creation/assignment; `PlanNextMonthModal.test.tsx` needs substantial rework for the new 7-step flow (step count, sinking-funds step, buffer step, adjust step) — existing "Apply and close," "Back/Close," and "loading state" tests carry over conceptually but the step-index assertions will all change.
- Regenerate `ui/src/api/bindings.ts` after every Rust command/type change, before touching frontend code that consumes them.

## 9. Linear tracking

One Linear project ("Budgets page redesign") under the existing "Ai-job-hunter" team, with issues split along the workstreams in this doc: (1) rollover computation + tests, (2) zero-budget visibility fix, (3) category groups end-to-end, (4) history comparison, (5) Plan Next Month 7-step rebuild. Each issue scoped to be independently shippable and reviewable, per Linear best practice of small, clear-scoped issues over one monolithic tracking ticket.

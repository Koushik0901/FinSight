# Design Conformance Deep Audit — 2026-07-01

## Context

The 2026-06-30 design-conformance-sweep (`2026-06-30-design-conformance-sweep.md`) matched all screens against the Claude Design mockup (project `fdbc4798-c6d0-41df-9499-e6ca4294d142`) at a structural level — CSS classes, casing, dead links. That pass was too shallow: it dismissed real gaps as "cosmetic" or "out of scope" without listing them, and never traced whether ported UI elements were actually wired to live data. This document is the corrected, exhaustive follow-up: every screen re-audited component-by-component for three mismatch flavors, with confirmation of real vs. dummy wiring.

**Ground rules used for this audit:**
- A visible difference is a defect until proven a hard technical impossibility, or it falls under one of two accepted exclusions: (1) the household/multi-user "Mira & Adam" concept — the app is single-user; (2) genuinely fabricated, no-real-backend concepts already excluded from the original sweep (the "trust dial" autonomy slider, the fake "Developer API" section, Copilot screen entirely).
- Differences in literal data *values* (app shows "$0" because it's a fresh month, mock shows "$1,850") are not defects — only flagged if the rendering logic itself looks broken.
- Confirmed one methodology risk before trusting the screenshots: on 2026-07-01 (day 1 of the month), Categories/Budget/Today all show $0 spend, which flattens every colored bar to gray. Re-verified Categories under "Year" scope — category dot colors and the stream bar render correctly with real distinct colors. The "app has no colors" read was a fresh-month artifact, not a systemic color-system bug (confirmed separately: `styles.css` design tokens and `ui/src/styles/tokens.css` are near-identical — accent, category colors, positive/negative/warning are the same hex values).

## Method

- CSS/token diff: design `styles.css` custom properties vs. `ui/src/styles/tokens.css` — aligned, not a source of drift.
- Every design component file read directly (`components/*.jsx`) via `mcp__claude-design__read_file`.
- Every corresponding app screen/component read in full.
- For every non-trivial visual element, traced back to its data source (hook → Tauri command → Rust repo/SQL) to confirm real vs. hardcoded/orphaned.
- Cross-validated: Budget/Recurring/Goals was independently audited twice (one subagent accidentally spawned a duplicate) — both runs converged on 28/28 equivalent findings.

## Findings are organized in three tiers, not by severity alone

- **Tier A — Clean wins.** Real, unambiguous bugs: dead buttons, hardcoded/wrong strings, real data fetched but never shown (or shown under the wrong scope). Fix regardless of any other decision.
- **Tier B — Buildable for real.** The design shows something absent from the app, but it's achievable using data the backend already has or can cheaply compute — no fabrication required.
- **Tier C — Fake-in-the-mockup-itself.** The design's version is a canned animation / hardcoded illustrative content with no real backend behind it (in the mockup, not just in our port). Matching these literally means reproducing dummy content, which conflicts with the "not just dummy elements" mandate. Needs an explicit human decision per item: build a real reduced version, or skip.

---

## Tier A — Clean wins (execute first)

| # | Screen | Finding | File:line | Status |
|---|---|---|---|---|
| A1 | Categories | ~~Extra "Type" column, orphaned data~~ — **correction after deeper check**: `spending_type` is NOT orphaned. `get_spending_breakdown` (Rust) aggregates by it and Budget.tsx's "Spending mix" stream bar renders that breakdown. Real remaining issue is Flavor 2 only: the column exists on Categories (where design has nothing) while its one real consumer lives on Budget — a placement judgment call, not a bug. Left for explicit decision, not auto-fixed. | `ui/src/screens/Categories.tsx:139,186`, `crates/finsight-app/src/commands/transactions.rs:310-364` | Verified, not a bug — needs a decision, not a fix |
| A2 | Categories | "Transactions" column ignores the scope toggle — always month-scoped even under Year/vs-average, producing contradictions like "Year total $9,460" next to "Transactions: 0" | `ui/src/screens/Categories.tsx:185`, `crates/finsight-app/src/commands/transactions.rs:228` | Open |
| A3 | Goals | `Goals.tsx:51` hardcodes the literal string "Linked to Car loan" for *every* liability-linked goal, regardless of which liability it is. `GoalDrawer.tsx:35-38` already does the correct lookup. | `ui/src/screens/Goals.tsx:51` | Open |
| A4 | Goals | "Sinking funds" section duplicates goals already shown above on the same screen (same `save-by-date` goals rendered twice) instead of being a distinct concept | `ui/src/screens/Goals.tsx:315` | Open |
| A5 | Goals | "Personal" chip is a static, meaningless label on every goal card (no real per-goal ownership data, and it's identical everywhere) | `ui/src/screens/Goals.tsx:39` | Open |
| A6 | Goals | "Pause" button has no `onClick` | `ui/src/screens/Goals.tsx:68` | Open |
| A7 | Goals | "PROGRESS" eyebrow label doesn't switch to "This month" for spending-cap goals like design does | `ui/src/screens/Goals.tsx:55` | Open |
| A8 | Budget | "Cover from another envelope" button has no `onClick` | `ui/src/screens/Budget.tsx:116` | Open |
| A9 | Budget | "Assign to a goal" / "Park in House Fund" buttons have no `onClick` | `ui/src/screens/Budget.tsx:235` | Open |
| A10 | Budget | Envelope/Tracking toggle has no `onClick`, no backing state — purely decorative | `ui/src/screens/Budget.tsx:196` | Open |
| A11 | Budget | Projected-EOM pill shows "Over plan"/"Under plan" text only, omitting the dollar delta that's already computed locally | `ui/src/screens/Budget.tsx:221` | Open |
| A12 | Budget | Group headers show only the label, no per-group spent/budget subtotal (computable client-side, no backend change) | `ui/src/screens/Budget.tsx:244` | Open |
| A13 | Reports | Merchant table drops already-fetched `categoryLabel`/`categoryColor` fields — no category shown in "Top merchants" despite the data being in the payload | `ui/src/screens/Reports.tsx:148-158`, `ui/src/api/bindings.ts:1373` | Open |
| A14 | Reports | `ReportData.monthlyLastYear` is fetched from the backend and never read anywhere in the frontend | `ui/src/screens/Reports.tsx`, confirmed via grep | Open |
| A15 | Reports | "Net worth" and "Spending deep dive" tabs render the same bar-chart component as "Monthly overview," just recolored — not distinct visualizations despite being separate tabs | `ui/src/screens/Reports.tsx:100-133` | Open |
| A16 | Reports | `useNetWorthHistory`/`net_worth_snapshots` (real, used on Today) is never called from Reports despite "Net worth" being a whole tab | `ui/src/screens/Reports.tsx` | Open |
| A17 | Recurring | Eyebrow omits the already-computed subscription count ("Recurring · N items" instead of "· N items · M subscriptions") | `ui/src/screens/Recurring.tsx:70` | Open |
| A18 | Rules | (verify) Agent proposal engine only ever produces pattern→category rules; confirm this isn't presented as covering more than it does | `crates/finsight-app/src/commands/agent.rs:248-266` | Open — verify only |

Already fixed this session (for reference, not part of this execution batch): Today's net-worth chart heading frozen at "last 6 months" regardless of range ([NetWorthChart.tsx](../../../ui/src/components/NetWorthChart.tsx)); Insights net-worth card leaking a raw account UUID instead of display name ([Insights.tsx](../../../ui/src/screens/Insights.tsx)).

---

## Tier B — Buildable for real (no fabrication needed)

- ~~**Categories**: per-category icon tiles (design uses icon-in-colored-square; app uses a plain dot) — needs a category icon field or client-side icon map.~~ **Done 2026-07-02** — client-side `iconFor(id)` lookup added, mirroring the existing `paletteFor(id)` color lookup, no backend changes. Spec: [2026-07-02-category-icon-tiles-design.md](../specs/2026-07-02-category-icon-tiles-design.md), plan: [2026-07-02-category-icon-tiles.md](2026-07-02-category-icon-tiles.md).
- **Reports**: Sankey money-flow diagram, donut/pie breakdown, category-trend sparkline grid, goals-progress widget, category table "trend + vs-prior-year" columns, FIRE calculator (pure client math) — all buildable from data already available or cheaply computable.
- ~~**Goals**: "Horizon" timeline visualization — data already available in-component (`monthsToGoal`/`etaLabel`).~~ **Done 2026-07-02** — combined per-goal ETA timeline with dynamic window sizing, excludes spending-cap and no-contribution goals, flags behind-schedule goals in red with a text cue (not color alone). Spec: [2026-07-02-goals-horizon-timeline-design.md](../specs/2026-07-02-goals-horizon-timeline-design.md), plan: [2026-07-02-goals-horizon-timeline.md](2026-07-02-goals-horizon-timeline.md).
- **Budget**: carry-forward ("Carried from April") line and history-strip bar visualization with forward-looking planned months — needs a small backend addition (carry_cents derivation, projecting `PlanNextMonth` data forward).
- **Recurring**: Calendar view (day-by-day spend grid — CSS scaffolding already exists in `app.css` under `.rcal-*`, unused) and Subscriptions/Audit view (usage tracking, price-history, real cancel-eligibility signal) — real backend work needed for occurrence-projection and subscription-status fields, but achievable without fabrication.
- **Settings/Rules**: a real "Agent" settings section (auto-categorize toggle etc.) — Rules' "Trust dial" copy already promises this exists in Settings; it doesn't yet.

## Tier C — Fake in the mockup itself (needs a per-item human decision)

- Recurring's subscription **cancellation-assistant** — design's version is a canned `setTimeout` animation, not a real integration.
- Rules/Settings/Onboarding **"Trust dial"** autonomy slider — already excluded per the original sweep instruction; keeps resurfacing because copy elsewhere references it as if it exists.
- Onboarding's missing "Concept," "Watch it work," "First goal" steps — buildable in a real reduced form, but the design's versions use canned counters/animations.
- Onboarding's `StepWelcome` hardcodes "$48,920 across 6 accounts" etc. as literal illustrative copy (faithfully ported from the mock) — a brand-new user with zero accounts sees what reads as personalized real data on their first screen.
- Reports' entire customizable/draggable widget-dashboard architecture (add/remove/resize/reorder widgets, saved report configs, widget library) — fundamentally a different, larger product shape than a few fixed tabs.

---

## Execution log

All 18 Tier A items resolved 2026-07-01:

- **A1** — corrected, not a bug (see strikethrough above). `spending_type` feeds Budget's real "Spending mix" bar via `get_spending_breakdown`; left as-is pending an explicit placement decision.
- **A2** — added `year_txn_count` to `CategoryWithSpending` (Rust: `crates/finsight-app/src/commands/transactions.rs`, migration-free — same query, added a COUNT column), regenerated bindings, wired `txnCountFor(category, scope)` in `Categories.tsx` so the Transactions column switches with the scope toggle. Regression test added.
- **A3** — `Goals.tsx` `GoalCard` now looks up the real liability name via `useLiabilities()` instead of hardcoding "Car loan". Regression test added using a liability named something else ("Home Mortgage") to prove it isn't hardcoded.
- **A4** — removed the duplicate "Sinking funds" section entirely (it re-rendered the same `save-by-date` goals already shown in the main list, with less detail).
- **A5** — removed the static "Personal" chip.
- **A6** — wired Pause/Resume: Pause sets `monthlyCents` to 0 via the existing `useUpdateGoalMonthly` mutation (already used by the What-If scenario), remembers the prior amount client-side for Resume. Real mutation, no fabrication — documented limitation: the remembered amount doesn't survive a reload (Resume falls back to directing the user to Adjust). **Follow-up fix (caught in review):** the "Paused" chip initially keyed off `monthlyCents === 0`, which also matches goals that were simply never given a monthly contribution (e.g. a fresh goal), mislabeling them "Paused" and colliding with the pre-existing "Needs attention" chip that uses the same condition. Fixed by gating the chip on an actual pause action taken this session (`goal.id in pausedPrevious`) rather than the raw zero value; added regression tests for both the never-configured case and the pause→chip-appears→resume→chip-disappears flow.
- **A7** — `PROGRESS` eyebrow now reads "This month" for spending-cap goals, matching design intent.
- **A8** — "Cover from another envelope" now computes the real envelope with the most spare budget and surfaces it in a toast (matches the design's own toast-only fidelity for this action).
- **A9** — "Assign to a goal" navigates to `/goals`; "Park in {goal name}" performs a real `useUpdateGoalBalance` mutation parking the unassigned amount into the first real goal (same pattern as Today's `SmartSweepCard`). Button label now shows the real goal name.
- **A10** — removed the dead "Tracking" toggle button (no second mode exists).
- **A11** — Projected EOM pill now shows the real dollar delta ("Over by $X" / "Under by $X").
- **A12** — group headers now show a real spent/budget subtotal when sorted by group.
- **A13** — Top merchants table now shows the real `categoryLabel`/`categoryColor` already present in the payload.
- **A14** — `monthlyLastYear` now drives a real year-over-year delta line under the Monthly overview chart.
- **A15/A16** — the three Reports tabs are now genuinely distinct: Overview keeps the income/expense bars, Net worth reuses the real `NetWorthChart` component wired to `useNetWorthHistory` (previously fetched nowhere in Reports despite a whole tab existing for it), Spending deep dive shows a real per-category breakdown from `topCategories` instead of the same recolored bars.
- **A17** — Recurring eyebrow now includes the already-computed subscription count.
- **A18** — verified clean, no fix needed. `RuleProposal`'s schema is genuinely scoped to pattern→category rules only, and the Rules.tsx copy accurately describes that scope — no overclaiming found.

Verification: `cargo check -p finsight-app` clean, `cargo test --workspace` full run **passed** (all Rust crates, 0 failed), `cd ui && npx tsc --noEmit` clean, full frontend suite 208/208 passing (up from 197 baseline — added/updated regression tests for A2, A3, A6, A9, A10, A13, A14, A15, A16). All 18 Tier A items are complete and fully verified.

# Design Conformance Sweep Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.
>
> **Deviation from standard plan template:** This is a design-conformance task (porting existing, already-designed markup/CSS onto live screens), not new-logic development. There is no new business logic to unit-test with TDD. "Verification" for each task means: (1) the screen's existing vitest file passes or is updated only for *presentational* assertions that literally changed, (2) `npx tsc --noEmit` is clean, (3) a visual screenshot via the dev server confirms the screen renders and roughly matches the design reference described in each task. Do not invent new tests; do not delete tests that assert real behavior (data fetching, mutations, empty/loading states).

**Goal:** Bring every screen except Copilot into 1:1 visual/structural conformance with the reference Claude Design project "Plutus" (`https://claude.ai/design/p/fdbc4798-c6d0-41df-9499-e6ca4294d142`), while keeping all existing Tauri command wiring, tanstack-query hooks, and TypeScript types unchanged.

**Architecture:** The design project is a static HTML/Babel-in-browser prototype (`data.js` mock data, CDN React) — it is a **markup/CSS/copy reference only**. Never port its data layer or component state logic. The app's `ui/src/styles/tokens.css` + `ui/src/styles/app.css` already share almost the entire class vocabulary with the design's `styles.css` (`.card`, `.stat`, `.h-display`, `.act-item`, `.bigchart`, `.stream`, `.toolbar`, `.cswatch`, `.rule`, `.tog`, `.sidebar`, `.nav-item`, `.cmdk-*`, etc. — confirmed by direct diff of both files during planning). So the foundation is **not** drifted. The drift is per-screen: places where a screen's `.tsx` uses ad-hoc inline styles, emoji, or altered copy/layout instead of the shared classes and structure the design defines. Each task fetches the matching design component via the `claude-design` MCP (`mcp__claude-design__read_file`, project id `fdbc4798-c6d0-41df-9499-e6ca4294d142`), diffs it conceptually against the current screen file, and ports over structure/classes/copy — not the mock data or the CDN-React scaffolding.

**Tech Stack:** React 18 + TypeScript, Vite, tanstack-query, react-router-dom, existing `ui/src/components/*` primitives, `ui/src/styles/{tokens,app}.css`.

**Excluded:** `ui/src/screens/Copilot.tsx` and everything under `ui/src/components/copilot/` — explicitly out of scope per user instruction.

**Known contradiction, resolved:** The design has a standalone `components/transactions.jsx` screen. The app deliberately removed its standalone Transactions screen in commit `5ff25ef` in favor of per-account `AccountTransactions`. Per user decision: **do not re-add a standalone Transactions route.** Instead, Task 6 ports the design's transaction-row markup/filter-bar styling into the existing `AccountTransactions.tsx` where applicable.

---

## Task list overview

| # | App file(s) | Design file(s) |
|---|---|---|
| 1 | `ui/src/components/Sidebar.tsx` | `components/sidebar.jsx` |
| 2 | `ui/src/components/CommandPalette.tsx` | `components/command-palette.jsx` |
| 3 | `ui/src/screens/Today.tsx` | `components/today.jsx` |
| 4 | `ui/src/screens/Accounts.tsx` | `components/accounts.jsx` |
| 5 | `ui/src/screens/Categories.tsx` | `components/categories.jsx` |
| 6 | `ui/src/screens/AccountTransactions.tsx` | `components/transactions.jsx` (style/markup only) |
| 7 | `ui/src/screens/Recurring.tsx` | `components/recurring.jsx` |
| 8 | `ui/src/screens/Goals.tsx` | `components/goals.jsx` |
| 9 | `ui/src/screens/Budget.tsx` | `components/budget.jsx` |
| 10 | `ui/src/screens/PlanNextMonthModal.tsx` | `components/plan-next-month.jsx` |
| 11 | `ui/src/screens/Insights.tsx` | `components/insights.jsx` |
| 12 | `ui/src/screens/Scenarios.tsx` | `components/scenarios.jsx` |
| 13 | `ui/src/screens/Reports.tsx` | `components/reports.jsx`, `components/reports-config.jsx`, `components/reports-widgets.jsx` |
| 14 | `ui/src/screens/Rules.tsx` | `components/rules.jsx` |
| 15 | `ui/src/screens/Settings.tsx` | `components/settings.jsx` |
| 16 | `ui/src/screens/Onboarding.tsx` (+ `ui/src/screens/onboarding/StepCategories.tsx`) | `components/onboarding.jsx` |

Tasks 1–2 (shared chrome) should land first since every screen renders inside them. Tasks 3–16 are independent of each other and can be parallelized across subagents once 1–2 are merged.

---

### Task 1: Sidebar conformance

**Files:**
- Modify: `ui/src/components/Sidebar.tsx`
- Reference (read-only, fetch via MCP): `components/sidebar.jsx`, and `.sidebar`/`.nav-item`/`.brand`/`.who`/`.search-trigger` rules already in `ui/src/styles/app.css:1049-1253`
- Test: `ui/src/components/CommandPalette.test.tsx` (sidebar has no dedicated test file today — check for one via `Glob ui/src/components/Sidebar.test.tsx` before assuming)

- [ ] **Step 1:** Fetch `components/sidebar.jsx` via `mcp__claude-design__read_file` (project `fdbc4798-c6d0-41df-9499-e6ca4294d142`).
- [ ] **Step 2:** Read current `ui/src/components/Sidebar.tsx`. Diff structure: brand mark, "who" account/profile row, search trigger (⌘K), `.nav-section` groupings, `.nav-item` icons/badges/pulse dot, footer. Port any structural or class gaps into the TSX, keeping `useNavigate`/`NavLink`/existing route list intact — only change markup/classes/copy, not routing logic.
- [ ] **Step 3:** Run `cd ui && npx vitest run` for any test file that renders Sidebar (search test files that import Sidebar) and `npx tsc --noEmit`.
- [ ] **Step 4:** Start the dev server (`mcp__Claude_Preview__preview_start`) and screenshot the app shell to confirm the sidebar renders without layout breakage.
- [ ] **Step 5:** Commit: `git add ui/src/components/Sidebar.tsx && git commit -m "style: reconcile Sidebar with design reference"`

### Task 2: Command palette conformance

**Files:**
- Modify: `ui/src/components/CommandPalette.tsx`
- Reference: `components/command-palette.jsx`
- Test: `ui/src/components/CommandPalette.test.tsx`

- [ ] **Step 1:** Fetch `components/command-palette.jsx` via MCP.
- [ ] **Step 2:** Diff against `ui/src/components/CommandPalette.tsx`: `.cmdk-mask`/`.cmdk`/`.cmdk-input`/`.cmdk-list`/`.cmdk-section`/`.cmdk-item`/`.cmdk-foot` structure, the "answer" mode (`.cmdk.answer`, `.cmdk-answer-header`, `.cmdk-thinking`, `.cmdk-answer-prose`) if the app has an equivalent AI-answer state. Port markup/classes only.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/components/CommandPalette.test.tsx` and `npx tsc --noEmit`. Update only assertions that check presentational text/DOM shape that intentionally changed.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile CommandPalette with design reference"`

### Task 3: Today screen conformance

**Files:**
- Modify: `ui/src/screens/Today.tsx`
- Reference: `components/today.jsx` (already fetched during planning — see below)
- Test: none dedicated found; check `Glob ui/src/screens/Today.test.tsx` first

**Known deltas from planning-phase diff (fix these explicitly):**
- Day header uses emoji `🔒` for the "Local-only" chip; design uses `<I.Lock width="11" height="11" />` from `ui/src/components/Icons.tsx`. Replace the emoji with the Lock icon.
- Day header uses a static bullet `•` for the agent-status chip; design uses `<span className="dot" />`. Use the shared `.dot` span instead of a literal character.
- Net-worth hero figure sets `fontSize: 112` inline; design uses the `.h-display` class (already defined in `app.css:326`) wrapping a `.figure` span. Replace the inline style with `className="h-display"` + nested `<span className="figure money">`.
- Design's hero-meta includes a narrative clause ("You're tracking N% below last month's spending") in addition to the trend pill — app only shows the trend pill and spend-so-far text. Add an equivalent narrative clause computed from real data (e.g. compare `totalSpendRaw` against last month's total from `useCategoriesWithSpending`), not hardcoded copy.
- Eyebrow text casing: design eyebrows are natural case with a leading `<span className="dot"/>` (e.g. "Net worth", "Morning briefing · 60 seconds"), the app renders them upper-cased as literal strings (e.g. "TODAY · ...", "NET WORTH"). `.eyebrow` CSS already applies `text-transform: uppercase`, so the source strings should be natural-case — remove manual `.toUpperCase()`/upper-cased literals so the CSS does the transform (avoids double/all-caps artifacts and matches design copy casing).

- [ ] **Step 1:** Apply the four deltas above directly to `ui/src/screens/Today.tsx`.
- [ ] **Step 2:** Fetch `components/today.jsx` again if needed for exact copy wording of the morning-briefing card, agent-activity card, and "Looking ahead" card sections; align section headings/labels (not data) with the design's phrasing where the app's current copy diverges cosmetically.
- [ ] **Step 3:** Run `cd ui && npx vitest run` (full suite — Today has no dedicated spec but is exercised indirectly by route tests) and `npx tsc --noEmit`.
- [ ] **Step 4:** Start dev server, navigate to `/`, screenshot, visually compare against the design's `today.jsx` layout (hero, stat row, briefing+sweep grid, spend-by-category, agent+upcoming grid).
- [ ] **Step 5:** Commit: `git commit -m "style: reconcile Today screen with design reference"`

### Task 4: Accounts screen conformance

**Files:**
- Modify: `ui/src/screens/Accounts.tsx`
- Reference: `components/accounts.jsx`
- Test: `ui/src/screens/Accounts.test.tsx`

- [ ] **Step 1:** Fetch `components/accounts.jsx` via MCP.
- [ ] **Step 2:** Diff account-group headers, balance figures, per-account card/row layout, chart/sparkline usage, empty state against `ui/src/screens/Accounts.tsx`. Port markup/classes/copy only — keep `useAccounts`/mutation hooks as-is.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Accounts.test.tsx` and `npx tsc --noEmit`; update only presentational assertions that changed on purpose.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Accounts screen with design reference"`

### Task 5: Categories screen conformance

**Files:**
- Modify: `ui/src/screens/Categories.tsx`
- Reference: `components/categories.jsx`
- Test: `ui/src/screens/Categories.test.tsx`

- [ ] **Step 1:** Fetch `components/categories.jsx`.
- [ ] **Step 2:** Diff grouping/list layout, `.category-item`/`.swatch`/spending-type chips against current screen (note: `Categories.tsx` and `ui/src/utils/categoryColor.ts` are mid-edit in the working tree per git status — read the current uncommitted state, don't clobber in-progress work, only add missing design conformance on top of it).
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Categories.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Categories screen with design reference"`

### Task 6: AccountTransactions styling (transactions.jsx port, no new route)

**Files:**
- Modify: `ui/src/screens/AccountTransactions.tsx`
- Reference: `components/transactions.jsx` (style/markup reference only — do NOT recreate it as a standalone route; do NOT copy its account-agnostic list logic wholesale)
- Test: `ui/src/screens/AccountTransactions.test.tsx`

- [ ] **Step 1:** Fetch `components/transactions.jsx`.
- [ ] **Step 2:** Identify presentational patterns worth porting: filter/search bar layout, row markup (`.tbl` usage, merchant/category cell layout, amount alignment), bulk-action toolbar, split/transfer badges. Apply these to `AccountTransactions.tsx` while keeping its per-account scoping and existing hooks (`useAccountTransactions` or equivalent) unchanged.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/AccountTransactions.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: port transactions design patterns into AccountTransactions"`

### Task 7: Recurring screen conformance

**Files:**
- Modify: `ui/src/screens/Recurring.tsx`
- Reference: `components/recurring.jsx`
- Test: check `Glob ui/src/screens/Recurring.test.tsx`

- [ ] **Step 1:** Fetch `components/recurring.jsx`.
- [ ] **Step 2:** Diff calendar/list view toggle, `.rule`-style recurring item cards, upcoming/frequency badges against current screen.
- [ ] **Step 3:** Run relevant vitest file (if present) and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Recurring screen with design reference"`

### Task 8: Goals screen conformance

**Files:**
- Modify: `ui/src/screens/Goals.tsx`
- Reference: `components/goals.jsx`
- Test: `ui/src/screens/Goals.test.tsx`

- [ ] **Step 1:** Fetch `components/goals.jsx`.
- [ ] **Step 2:** Diff goal-card layout, `.goal-bar`/`.goal-range` usage, compound-growth projector section (per CLAUDE.md's Financial Freedom Framework table — this section must stay wired to real projections, only its markup/classes should change).
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Goals.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Goals screen with design reference"`

### Task 9: Budget screen conformance

**Files:**
- Modify: `ui/src/screens/Budget.tsx`
- Reference: `components/budget.jsx`
- Test: `ui/src/screens/Budget.history.test.tsx`

- [ ] **Step 1:** Fetch `components/budget.jsx`.
- [ ] **Step 2:** Diff envelope rows, `.budget-inline-edit` usage, allocation donut/spending-type breakdown (Conscious Spending framework section) against current screen.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Budget.history.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Budget screen with design reference"`

### Task 10: Plan Next Month modal conformance

**Files:**
- Modify: `ui/src/screens/PlanNextMonthModal.tsx`
- Reference: `components/plan-next-month.jsx`
- Test: `ui/src/screens/PlanNextMonthModal.test.tsx`

- [ ] **Step 1:** Fetch `components/plan-next-month.jsx`.
- [ ] **Step 2:** Diff `.dialog-overlay`/`.dialog-grid` usage, step layout, envelope-adjustment rows against current modal.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/PlanNextMonthModal.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile PlanNextMonthModal with design reference"`

### Task 11: Insights screen conformance

**Files:**
- Modify: `ui/src/screens/Insights.tsx`
- Reference: `components/insights.jsx`
- Test: `ui/src/screens/Insights.operator.test.tsx`, `ui/src/screens/Insights.memory.test.tsx`

- [ ] **Step 1:** Fetch `components/insights.jsx`.
- [ ] **Step 2:** Diff insight-card layout, anomaly/opportunity/pattern grouping, `.agent-rich-*` usage against current screen.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Insights.operator.test.tsx src/screens/Insights.memory.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Insights screen with design reference"`

### Task 12: Scenarios screen conformance

**Files:**
- Modify: `ui/src/screens/Scenarios.tsx`
- Reference: `components/scenarios.jsx`
- Test: `ui/src/screens/Scenarios.test.tsx`

- [ ] **Step 1:** Fetch `components/scenarios.jsx`.
- [ ] **Step 2:** Diff `.scenario-composer`/`.scenario-input` usage, scenario result cards, comparison layout against current screen.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Scenarios.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Scenarios screen with design reference"`

### Task 13: Reports screen conformance

**Files:**
- Modify: `ui/src/screens/Reports.tsx`
- Reference: `components/reports.jsx`, `components/reports-config.jsx`, `components/reports-widgets.jsx`
- Test: `ui/src/screens/Reports.test.tsx`

- [ ] **Step 1:** Fetch all three design reports files via MCP.
- [ ] **Step 2:** Diff report-library grid, widget cards/charts, config panel layout against current `Reports.tsx`. This is the largest design surface (52KB `reports-widgets.jsx`) — focus on structural/class parity for the widget types the app actually renders; do not add widget types the app has no data for.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Reports.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Reports screen with design reference"`

### Task 14: Rules screen conformance

**Files:**
- Modify: `ui/src/screens/Rules.tsx`
- Reference: `components/rules.jsx`
- Test: `ui/src/screens/Rules.test.tsx`

- [ ] **Step 1:** Fetch `components/rules.jsx`.
- [ ] **Step 2:** Diff `.rule`/`.cond`/`.tok` condition-token rendering against current screen.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Rules.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Rules screen with design reference"`

### Task 15: Settings screen conformance

**Files:**
- Modify: `ui/src/screens/Settings.tsx`
- Reference: `components/settings.jsx`
- Test: `ui/src/screens/Settings.test.tsx`

- [ ] **Step 1:** Fetch `components/settings.jsx`.
- [ ] **Step 2:** Diff `.s-row` settings-row layout, section grouping, toggle (`.tog`) usage against current screen.
- [ ] **Step 3:** Run `cd ui && npx vitest run src/screens/Settings.test.tsx` and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Settings screen with design reference"`

### Task 16: Onboarding conformance

**Files:**
- Modify: `ui/src/screens/Onboarding.tsx`, `ui/src/screens/onboarding/StepCategories.tsx`
- Reference: `components/onboarding.jsx`
- Test: check `Glob ui/src/screens/Onboarding.test.tsx` and `ui/src/screens/onboarding/*.test.tsx`

- [ ] **Step 1:** Fetch `components/onboarding.jsx`.
- [ ] **Step 2:** Diff step-indicator, per-step card layout, CTA placement against current onboarding flow. `StepCategories.tsx` is mid-edit in the working tree — read current uncommitted state first, layer conformance changes on top.
- [ ] **Step 3:** Run relevant vitest files and `npx tsc --noEmit`.
- [ ] **Step 4:** Commit: `git commit -m "style: reconcile Onboarding screens with design reference"`

---

## Final verification (after all tasks land)

- [ ] Run full suite: `cd ui && npx vitest run` — expect 105+ tests passing (some counts may shift if presentational assertions were updated; no test should be deleted without a behavioral-equivalent replacement).
- [ ] Run `cd ui && npx tsc --noEmit` — expect 0 errors.
- [ ] Run `cargo test --workspace` to confirm no Rust-side regression (none expected — this plan touches only `ui/`).
- [ ] Start the app via `mcp__Claude_Preview__preview_start` and click through every in-scope screen once, screenshotting each, to catch any layout regression the per-screen vitest runs wouldn't.

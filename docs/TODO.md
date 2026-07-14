# FinSight — Implementation TODO

> **For new agents:** The design reference is at `design/plutus/project/components/` (JSX prototypes with full HTML/CSS/JS). The implementation is at `ui/src/screens/` (React + TypeScript + Tauri). Read the relevant design file before implementing each section. The design uses mock data (`FS.*`); the implementation must use real Tauri commands.
>
> **Stack:** Rust/Tauri 2 backend · React 18 + TypeScript + Vite frontend · SQLite/SQLCipher via rusqlite · tanstack-query hooks · sonner toasts · zod + react-hook-form · design tokens in `ui/src/styles/tokens.css` + `app.css`
>
> **Adding a Tauri command:** (1) write the function in `crates/finsight-app/src/commands/`, (2) register it in `crates/finsight-app/src/lib.rs` inside `build_specta_builder()`, (3) run `cargo run -p finsight-tauri --bin export_bindings` from the **repo root** to regenerate `ui/src/api/bindings.ts`.

---

## 📍 Live status — where the real gaps are

**Everything below this section is a historical shipping log (all `[x]`), not the
live gap list.** For what is actually incomplete, see the product audit:

> **`docs/audits/2026-07-10-finsight-product-audit.md`** — ranked findings (P0–P3)
> with root causes, acceptance criteria, and a dependency graph. This is the
> living "known gaps" document.

Audit findings resolved (git log, commits tagged `P0-*`/`P1-*`/`P2-*` +
feat(transfers)/fix(investments)/P3): **all P0–P3 items in the product audit
and all items in the completeness/cross-user-ownership roadmap are done** as
of 2026-07-13 — including cross-user ownership shares (V042–V045), the sticky
transfer verdict + review surface + bulk counterparty verdicts (V046),
investment-account correctness (market value verbatim; brokerage activity
excluded from every cashflow/nudge surface), the previously-missing
`/transactions` route the Inbox CTAs deep-link to, the P3 polish tier
(startup/import cascade transparency, merchant display naming, currency
creation defaults, history housekeeping), a real-app UI validation pass
against the compiled Tauri binary (found + fixed a live drawer-staleness bug
unit tests couldn't catch), and per-item resolution evidence now inline in
`2026-07-10-finsight-product-audit.md` itself (every P0/P1/P2/P3 heading
carries its own resolving commit hash and summary, not just this pointer).
The two audit docs in `docs/audits/` carry per-item status and acceptance
evidence; the audit probe
(`crates/finsight-app/tests/audit_probe.rs`) is the rerunnable acceptance
harness on `samples/`.

---

## ✅ Agent rich responses shipped (2026-06-28)

Detailed handoff: `docs/agent-rich-responses-handoff.md`.

- `[x]` Extend `AgentAnswer` with typed `responseBlocks` for safe markdown, tables, metric grids, callouts, and charts.
- `[x]` Add backend validation and fallback enrichment so existing prose/reasoning/alternatives render through rich blocks.
- `[x]` Add `AgentResponseRenderer` using `react-markdown`, `remark-gfm`, `rehype-sanitize`, and existing Nivo chart libraries.
- `[x]` Wire the shared renderer into the Copilot screen and Command Palette ask mode.
- `[x]` Add rich markdown/table rendering tests and unsafe-HTML sanitization coverage.
- `[x]` Regenerate TypeScript bindings and validate backend/frontend builds and targeted tests.

---

## ✅ SimpleFIN production hardening shipped (2026-06-28)

Design: `docs/superpowers/specs/2026-06-28-simplefin-production-hardening-design.md`.
Handoff: `docs/simplefin-production-hardening-handoff.md`.

- `[x]` Add migration `V026__import_reconciliation_workbench.sql` for durable import candidates, candidate matches, sync run audit rows, and source-separated account balances.
- `[x]` Harden background sync so Off pauses without killing the scheduler, On resumes it, and manual/background sync jobs cannot overlap.
- `[x]` Add SimpleFIN fetch retry/backoff for transient failures and avoid retrying auth/payment/invalid credential failures.
- `[x]` Record sync runs with success/partial/failed status and per-run added/updated/skipped/queued counts.
- `[x]` Create durable Inbox sync-error alerts when SimpleFIN sync fails after retry or fails with unretryable access/payment errors.
- `[x]` Separate `simplefin` bank balance snapshots from `ledger_recomputed` balances so drift checks compare ledger totals against the latest bank-reported snapshot only.
- `[x]` Refresh account `extra_json`/`raw_json` from the fresh SimpleFIN account payload before importing investment holdings.
- `[x]` Upgrade CSV/SimpleFIN reconciliation with amount tolerance, confidence scoring, collision handling, and pending-to-posted matching.
- `[x]` Queue ambiguous/colliding import candidates in a durable Import Review workbench instead of silently duplicating or merging them.
- `[x]` Add Inbox Import Review actions: accept recommended/alternative match, create new transaction, or dismiss candidate.
- `[x]` Regenerate TypeScript bindings and update frontend hooks for import review commands and queued-for-review sync counts.

---

## ✅ SimpleFIN + import coordination shipped (2026-06-28)

Detailed handoff: `docs/simplefin-import-coordination-handoff.md`.

- `[x]` Refresh revoked SimpleFIN credentials using a replacement setup token and store the new access URL in the OS keychain.
- `[x]` Relink existing SimpleFIN accounts to refreshed connection rows instead of creating stale duplicate connections/accounts.
- `[x]` Run local SimpleFIN sync after credential refresh; result was 7 linked accounts synced, 253 transactions added, 7 updated, 3 skipped, 0 errors.
- `[x]` Cap SimpleFIN initial sync lookback to 44 days to stay under the bridge's 45-day recommended range; keep 14-day subsequent lookback.
- `[x]` Make account display names prefer `nickname`, then `official_name`, then provider `name` so raw SimpleFIN IDs are not shown when better labels exist.
- `[x]` Add shared transaction reconciliation for CSV and SimpleFIN imports: exact `imported_id` first, then conservative fuzzy same-account amount/date/merchant matching.
- `[x]` Add per-batch collision avoidance so multiple incoming rows cannot match the same existing ledger transaction.
- `[x]` Let SimpleFIN enrich matching CSV-created transactions with provider IDs/raw sync metadata while preserving user notes and categories.
- `[x]` Make CSV imports skip matching SimpleFIN transactions and persist full transaction metadata for new CSV rows.
- `[x]` Add regression tests for CSV-before-SimpleFIN and SimpleFIN-before-CSV duplicate prevention.

---

## ✅ Wave D shipped (2026-06-16)

All Wave D features are done and pushed to main.

**Shipped in Wave D — Group 1 (splits + notifications):** Transaction splits UI (split a transaction across multiple categories via `TransactionDrawer`), in-app notification centre (bell icon in sidebar with `markNotificationRead` / `dismissAllNotifications` commands, V012 migration adds `transaction_splits` and `notifications` tables).

**Shipped in Wave D — Group 2 (real agentic features):** Replaced all simulated/hardcoded agent behaviour with live LLM-backed implementations:
- **Anomaly detection** (`crates/finsight-agent/src/anomaly.rs`): two-phase detection — IQR statistical pre-filter (Q3 + 1.5×IQR fence, min 3 historical transactions per merchant) → LLM batch confirmation. Runs automatically at end of every `run_job` scan; sets `is_anomaly = 1` on confirmed outliers; stores `agent.last_scan_at` / `agent.last_scan_categorized` to the settings KV.
- **Real agent status** (`get_agent_status` command): returns live counts — uncategorized transactions, flagged anomalies, over-budget envelopes, upcoming bills, last scan time.
- **Data-driven Insights ticker** (`AgentStatusBar` in `Insights.tsx`): replaced 6 hardcoded strings with messages built from `useAgentStatus()` data. Cycles between real facts (last scan time, uncategorized count, anomaly count, budget/bills alerts). Falls back to "All clear · no issues found".
- **Free-text LLM ask in CommandPalette**: removed 5 pre-computed canned questions; "Ask: [query]" item appears on any non-empty input and fires a real `ask_agent` LLM call with injected financial context (net worth, monthly totals, top categories, budgets, anomaly count). Prose answer rendered inline; graceful no-provider error with "Open Settings →" link.

**New commands:** `getAgentStatus`, `askAgent`. **New types:** `AgentStatus`, `AgentAnswer`. **New hooks:** `useAgentStatus` (30s refetch), `useAskAgent`. **New crate dep:** `chrono` added to `finsight-agent`. **Next migration = V013.**

---

## ✅ Wave C shipped (2026-06-15)

All Wave C features are done and pushed to main.

**Shipped in Wave C:** §2 Plan Next Month wizard (6-step modal launched from Budget), §4c Accounts CSV export, §5c Transactions CSV export, §5d Reimbursable/split flag toggles and table chips, §7c Budget 5-month spending history table, §8b Recurring price-history chip (↑↓ with old→new amount), §10a Reports scope switcher (Month/Quarter/Year/All time), §10b DonutChart (spending breakdown by category), §10c Year-over-year comparison chart, §10d Saved report tabs (localStorage-persisted, create/rename/delete), §10e Widget show/hide toggles (Customize mode per tab), §11c Agent 24h activity log in Rules sidebar, §12d Keyboard shortcuts reference in Settings.

**New hooks:** `usePlanNextMonthData`, `useApplyNextMonthPlan`, `useBudgetHistory`, `useRecentAgentActivity`. **New commands:** `getPlanNextMonthData`, `applyNextMonthPlan`, `listBudgetHistory`, `listRecentAgentActivity`, `exportTransactionsCsv`, `exportAccountCsv`, `getReportData` (now accepts `scope: String`). **New types:** `CategoryPlanRow`, `PlanData`, `PlanAssignment`, `AgentActivity`, `MonthlyActual`, `CategoryHistory`. **RecurringItem** extended with `minAmountCents`/`maxAmountCents`; **ReportData** extended with `monthlyLastYear`.

---

## ✅ Wave B shipped (2026-06-07)

All Wave B features are done and merged to main. Design + plan: `docs/superpowers/specs/2026-06-05-wave-b-all-remaining-design.md`, `docs/superpowers/plans/2026-06-05-wave-b-all-remaining.md`.

**Shipped in Wave B:** §3b Smart Sweep card, §3c upcoming recurring chips, §3d Runway stat, §6c AI insight sentence, §9b what-if Apply button, §9c Sinking funds section, §11b manual new-rule builder, §12a data export, §12b currency selector, §12c appearance section, §13a agent operator panel, §14a Ask the agent mode, §14b Run a what-if action.

**Also shipped:** dev-only `seed_dev_demo` command (`crates/finsight-core/src/sample.rs`) that seeds the "Mira & Adam" prototype dataset (6 accounts, 6 months of recurring transactions, 5 goals, 5 assets, 4 liabilities, budgets, net-worth snapshots). Exposed as a Tauri command and a DEV-only "Load demo data" button in Settings (guarded by `import.meta.env.DEV`). Run `pnpm tauri:dev`, go to Settings, click "Load demo data" to populate all screens for local testing. **Bug fixed:** recurring detection crashed with "Error detecting recurring" when uncategorised income transactions (NULL `cat_label`) were present — now handled with `Option<String>`.

---

## ✅ Backend foundations landed (2026-06-04)

The migration-heavy **backend** for items §3a, §4a, §4b, §5d, §11a, and §13b is **done and merged to main** — schema, repos, Tauri commands, live wiring, tests, and bindings. Design + plan: `docs/superpowers/specs/2026-06-04-backend-foundations-design.md`, `docs/superpowers/plans/2026-06-04-backend-foundations.md`.

**Shipped:**
- **Migrations V006–V011:** `net_worth_snapshots`, `manual_assets`, `liabilities`, `rule_proposals`, `agent_memory`, and transaction `is_reimbursable`/`is_split` columns. **V012** added `transaction_splits` and `notifications` tables. **Next migration = V013.**
- **Repos:** `net_worth`, `manual_assets`, `liabilities`, `rule_proposals`, `agent_memory` + `transactions::set_flags`.
- **Commands (all in `bindings.ts`):** `commands/assets.rs` (manual-asset & liability CRUD, `record_net_worth_snapshot`, `list_net_worth_history`); `commands/insights.rs` (`list_agent_memory`, `forget_agent_memory`); `agent.rs` (`list_rule_proposals`, `accept_rule_proposal`, `decline_rule_proposal`); `transactions::set_transaction_flags`.
- **Live wiring:** user category correction → `agent_memory` upsert; categorizer post-run → `rule_proposals` (≥3 corrections, deduped); net-worth snapshot auto-records on app start.

> Building the UIs below: the commands/types already exist — import from `ui/src/api/client.ts` and add tanstack-query hooks under `ui/src/api/hooks/`.

---

## 1. Scenarios screen (entire feature — not in nav) ✅ DONE

**Design reference:** `design/plutus/project/components/scenarios.jsx`

The Scenarios screen is a what-if / natural-language forecasting tool. It is completely absent from FinSight — no route, no nav entry, no backend.

### What to build

**Backend (new `crates/finsight-app/src/commands/scenarios.rs`):**
- `run_scenario(description: String, months: u32) -> ScenarioResult` — takes a text description and forecast horizon, returns a `ScenarioResult` with:
  - `verdict: bool` — can the user cover this scenario?
  - `runway_change_days: i64` — delta in runway days
  - `monthly_impact_cents: i64` — estimated monthly cost
  - `considerations: Vec<String>` — 3–5 bullet points of analysis
  - `baseline_monthly: Vec<i64>` — current trajectory (N months of net)
  - `scenario_monthly: Vec<i64>` — scenario trajectory (same N months, adjusted)
- `list_scenario_history() -> Vec<SavedScenario>` — previously saved scenarios
- `save_scenario(description: String, result: ScenarioResult)` — persist to a new `scenarios` table (V005 migration: `id, description, result_json, created_at`)
- `delete_scenario(id: String)`

**Frontend (`ui/src/screens/Scenarios.tsx`):**
- Header: eyebrow "Scenarios", title "What if…"
- **Input area:** large text field ("What if I take a 6-month sabbatical?") + suggested chip buttons for common scenarios ("Cut income 50%", "Eliminate dining out", "Buy a car $35k", "Add $500/mo to savings")
- **Forecast range toolbar:** `6M / 12M / 24M` — drives how many months the SVG chart shows
- **Results panel** (appears after submission):
  - Verdict card — green "Coverable" / red "Not coverable" with explanation sentence
  - Impact grid — 3 stats: runway change, monthly impact, goals affected
  - SVG line chart — two lines: current trajectory (solid) vs scenario (dashed accent), months on X axis, cumulative net on Y axis, "TODAY" vertical marker
  - Numbered considerations list
  - Action row: "Save scenario", "Discard"
- **History list** (right sidebar): past saved scenarios, each with description, verdict chip, date, and "Re-run" button
- Add `/scenarios` route to `App.tsx` and sidebar nav (use `I.Bolt` icon, between Goals and Reports)

---

## 2. Plan Next Month wizard (launched from Budget) — ✅ DONE

**Design reference:** `design/plutus/project/components/plan-next-month.jsx`

A 6-step guided modal launched by "Plan next month" from the Budget screen. Helps the user assign next month's income to envelopes before the month starts.

### What to build

**Backend (add to `crates/finsight-app/src/commands/budget.rs`):**
- `get_plan_next_month_data() -> PlanData` — returns:
  - `income_cents: i64` — average of last 3 months income from transactions
  - `existing_budgets: Vec<{category_id, label, color, this_month_cents, last_month_cents, budget_cents}>`
  - `goals: Vec<GoalDto>` — active goals with monthly_cents already set
  - `recurring_expense_cents: i64` — total detected monthly recurring outflows
- `apply_next_month_plan(assignments: Vec<{category_id, amount_cents}>)` — upserts budget rows for next month's YYYY-MM

**Frontend (`ui/src/components/PlanNextMonthModal.tsx`):**
Full-screen overlay (z-index 70) with:
- **Step 1 — Look back:** Shows last month's spending by category as a horizontal bar list. Readonly. "This is what actually happened."
- **Step 2 — The basics:** Assign amounts to Housing, Utilities, Groceries (the must-pay envelopes). Each row has an inline number input pre-filled with last month's actual. Running total of assigned vs income shown on the right.
- **Step 3 — The not-yet:** Assign remaining flexible categories (Dining, Transport, etc.). Same inline inputs.
- **Step 4 — Buffer:** Slider 0–$2000 for an unassigned buffer. Subtracts from remaining-to-assign.
- **Step 5 — Pulls:** Goal contribution sliders — for each active goal, adjust monthly contribution.
- **Step 6 — Adjust & Done:** Review all assignments in a table. "Apply to [next month]" button calls `apply_next_month_plan`. Shows a success state.

Live preview panel (right side, visible all 6 steps): stacked proportional bar of income allocation — essentials / flexible / goals / buffer / unassigned — updates in real time.

Wire the "Plan next month" button in `Budget.tsx` header to open this modal.

---

## 3. Today screen — missing pieces

**Design reference:** `design/plutus/project/components/today.jsx`

### 3a. Net-worth area chart with range selector — ✅ DONE

The design shows a SVG area chart tracing net-worth history with selectable ranges (1M / 3M / 6M / 1Y / All).

**Backend ✅ done:** `net_worth_snapshots` table (V006). Commands `recordNetWorthSnapshot()` and `listNetWorthHistory(days)` exist; a snapshot auto-records on app start. (Currently sums bank-account balances only; fold in manual assets/liabilities once §4a/§4b UIs exist.)

**Frontend (pending):** In `Today.tsx`, above the stat row, render a SVG area chart (same pattern as `Reports.tsx`'s NetLine):
- Range toolbar: 1M / 3M / 6M / 1Y / All (defaults to 6M)
- Gradient fill under the line (lime at top, transparent at bottom — see `design/plutus/project/components/today.jsx` `NetWorthChart`)
- Last point glows with a radius-14 accent circle
- Month labels on X axis in `var(--mono)` font

### 3b. Smart Sweep suggestion card ✅ DONE

When there is a positive net this month (income > expenses), show a card:
```
You have $X unallocated this month.
[Park in House Fund]  [Assign to a goal…]  [Dismiss]
```
Calls `update_goal_balance` or `set_budget` depending on action chosen.
Show only when `totals.netCents > 5000` and the user hasn't dismissed it this session (use `useState` — no persistence needed).

### 3c. Upcoming recurring items ✅ DONE

Below the category stream bar, add a compact list of recurring items due in the next 7 days:
- Call `listRecurring()`, filter where `nextExpected` is within 7 days from today
- Render as a horizontal chip row: merchant initials dot + name + amount + days-until
- "See all" link to `/recurring`

### 3d. Runway stat in the stat row ✅ DONE

Replace the "Accounts" stat card with a "Runway" stat:
- `runway_days = (total_account_balance) / avg_daily_burn`
- `avg_daily_burn` = `(expenses this month) / day_of_month`
- Display as "134 days" with sub-text "at current burn"

---

## 4. Accounts screen — missing pieces

**Design reference:** `design/plutus/project/components/accounts.jsx`

### 4a. Manual assets section — ✅ DONE

The design has a second section below bank accounts for manually tracked assets (home value, car, investment portfolio, crypto).

**Backend ✅ done:** `manual_assets` table (V007). Commands `listManualAssets`, `createManualAsset`, `updateManualAsset`, `deleteManualAsset` exist (types `ManualAsset`, `NewManualAsset`, `ManualAssetPatch`).

**Frontend (pending):** In `Accounts.tsx`, add a "Manual assets" section below the accounts table with:
- Asset cards showing: icon (house/car/chart/currency), name, value, type chip
- "Add manual asset" button opens a small form drawer
- Total assets + total accounts = net worth displayed at top

### 4b. Liabilities section — ✅ DONE

Similarly, track liabilities (mortgage, student loans, credit card balances).

**Backend ✅ done:** `liabilities` table (V008) with `limit_cents`, `apr_pct`, `payoff_date`. Commands `listLiabilities`, `createLiability`, `updateLiability`, `deleteLiability` exist (types `Liability`, `NewLiability`, `LiabilityPatch`).

**Frontend (pending):** "Liabilities" section in `Accounts.tsx`:
- Each row: name, type chip, balance, APR, progress bar (balance / original limit), payoff date
- Net worth = accounts + assets − liabilities (update the Today screen hero too)

### 4c. CSV export per account — ✅ DONE

Add an "Export CSV" button to the per-account row or account detail panel that calls:
```rust
// Already have import; export just needs a query + write-to-file
async fn export_account_csv(account_id: String) -> AppResult<String>  // returns file path
```
Use `tauri_plugin_dialog` to show a save dialog, then write CSV with: date, merchant, category, amount, notes.

---

## 5. Transactions screen — missing pieces

**Design reference:** `design/plutus/project/components/transactions.jsx`

### 5a. Search bar ✅ DONE

Add a search input above the table (full-width, debounced 300ms):
```rust
// Modify list_transactions to accept an optional search string
// SQL: WHERE lower(merchant_raw) LIKE lower('%' || ?search || '%')
//       OR lower(notes) LIKE lower('%' || ?search || '%')
```
Update `TxnFilterInput` in the backend to include `search: Option<String>`.
In `Transactions.tsx`, add a controlled `<input>` that updates the filter passed to `useTransactions`.

### 5b. Filter tabs ✅ DONE

Below the header, add a tab strip: **All · Needs review · Anomalies · No category**
- "Needs review" — filter where `ai_confidence < 0.6 AND last categorization source = 'llm'`
- "Anomalies" — filter where `is_anomaly = 1`
- "No category" — filter where `category_id IS NULL`

Update `TxnFilterInput` backend to accept `filter_preset: Option<String>` and add the WHERE clauses.

### 5c. CSV export — ✅ DONE

"Export CSV" button in the Transactions header that exports the currently filtered view:
```rust
async fn export_transactions_csv(filter: TxnFilterInput) -> AppResult<String>
```
Uses `tauri_plugin_dialog` to save, writes: date, merchant, category, amount, notes.

### 5d. Reimbursable and split flags — ✅ DONE

**Backend ✅ done:** `is_reimbursable` / `is_split` columns on `transactions` (V011); `Transaction` carries both booleans; command `setTransactionFlags(id, isReimbursable, isSplit)` exists.

**Frontend ✅ done:** Toggle buttons in `TransactionDrawer` with `aria-pressed`. Chips in the transactions table for rows where these are true.

---

## 6. Categories screen — missing pieces

**Design reference:** `design/plutus/project/components/categories.jsx`

### 6a. Year scope ✅ DONE

Add a third toolbar option "Year" to the scope toggle. When selected:
- Fetch the last 12 months of spending from `list_categories_with_spending` — but this command only returns current and last month. Add a new backend field or command for year-to-date total.
- Update `list_categories_with_spending` to also return `year_total_cents: i64` (SUM of outflow where `posted_at >= strftime('%Y-01-01', 'now')`).
- In the frontend, use `yearTotalCents` as the value when scope = "year".

### 6b. Budget column in the table ✅ DONE

The Categories table should show each category's budget alongside actual spending. This requires joining `budgets` (current month row) into the `list_categories_with_spending` query. Add `budget_cents: i64` to `CategoryWithSpending` (0 if no budget set). Render a new "Budget" column in the table, coloring it `var(--negative)` if `thisMonthCents > budgetCents`.

### 6c. AI insight sentence ✅ DONE

Below the stream bar in the summary card, render a computed sentence:
```
✦ [TopGainer] dropped $X — the biggest improvement this month.
  [TopRiser] rose by $Y.
```
This is pure frontend computation from the categories array — no new backend needed. Sort cats by `(thisMonthCents - lastMonthCents)` ascending for biggest drop, descending for biggest rise.

---

## 7. Budget screen — missing pieces

**Design reference:** `design/plutus/project/components/budget.jsx`

### 7a. "To Budget" tracker ✅ DONE

Track unassigned income: `toBudget = income_this_month - sum(all budget_cents for this month)`.
Show a pill row below the header:
```
[lime dot] To Budget · unassigned
$1,240  of $6,800 income · $5,560 assigned
[Assign to a goal…]  [Park in House Fund]
```
Requires calling `get_month_totals` (already exists) and `list_budget_envelopes` (already exists) in Budget.tsx — compute the difference client-side.

### 7b. "By activity" sort ✅ DONE

Add a 4th sort option in the toolbar: `By activity`. Sort envelopes by `txnCount DESC`.
Already available in `BudgetEnvelope.txnCount` from the backend.

### 7c. 5-month history strip — ✅ DONE

Below the main envelope grid, add a compact table showing each category's last 5 months of actual spending. Requires a new backend query:
```rust
// list_budget_history(months: u32) -> Vec<{category_id, label, color, monthly_actuals: Vec<{month, cents}>}>
```
SQL: group transactions by `(category_id, strftime('%Y-%m', posted_at))` for the last N months.
Render as a table: category name | Jan | Feb | Mar | Apr | May (with color intensity scaled to max).

---

## 8. Recurring screen — missing pieces

**Design reference:** `design/plutus/project/components/recurring.jsx`

### 8a. Day-detail panel ✅ DONE

When a calendar cell is clicked in `CalendarView`, animate a detail panel below the grid showing that day's expected movements:
- Day number (large), weekday name, "TODAY" badge if applicable
- Right side: net total card (color-coded positive/negative)
- List of recurring items for that day: logo tile, name, status chips, amount, "···" menu

Implementation: add `selectedDay: number | null` state to `CalendarView`. When non-null, render a `.rcal-detail` div (CSS already exists in `app.css`) below the grid. Items come from `dayMap[selectedDay]`.

### 8b. Price-history chip per subscription — ✅ DONE

In the Subscriptions view, if an item's `lastAmountCents` differs from a previous occurrence (requires comparing across `occurrences`), show a chip: `price-up: $19.99 → $22.99`. This requires updating the `list_recurring` SQL to also return `min_amount_cents` alongside `max_amount_cents` (already fetched as `last_amount`). If `max_amount_cents != min_amount_cents`, flag as price-changed.

---

## 9. Goals screen — missing pieces

**Design reference:** `design/plutus/project/components/goals.jsx`

### 9a. Pace status chip on each goal card ✅ DONE

Compute pace from: `monthsRemaining = ceil((target - current) / monthly)` vs `monthsExpected = (targetDate - today) in months`.
- Ahead: `monthsRemaining < monthsExpected * 0.85`
- On track: within 15%
- Needs attention: `monthsRemaining > monthsExpected * 1.15` or no monthly contribution

Show as a `chip` in the goal card header: `chip.positive` (Ahead), default chip (On track), `chip.warning` (Needs attention).

### 9b. Apply scenario from what-if slider ✅ DONE

The "Apply" button in the what-if panel currently just resets the slider. Wire it to actually persist:
- Call `update_goal(id, { monthly_cents: goal.monthly_cents + extra })` — needs a new `update_goal` command that patches the monthly_cents field. Add to `commands/budget.rs`.
- Show a toast: "Applied +$X/mo to [Goal name] · ETA now [new date]"

### 9c. Sinking funds section ✅ DONE

The design has a separate "Sinking funds" concept — short-term savings buckets with due dates (car registration, Christmas gifts, annual subscriptions). These are basically goals with `goal_type = 'save-by-date'` and a near-term date. The Goals table already supports this type.

**Frontend only:** Add a "Sinking funds" section below the main goal list that shows `goals.filter(g => g.goalType === 'save-by-date' && withinOneYear(g.targetDate))` in a 2-column compact card grid (tighter than the main goal cards).

---

## 10. Reports screen — missing pieces

**Design reference:** `design/plutus/project/components/reports.jsx`, `reports-widgets.jsx`, `reports-config.jsx`

The current Reports screen is a fixed layout. The design is a fully customizable widget dashboard. This is the largest remaining feature gap.

### 10a. Scope switcher — ✅ DONE

Add a scope toolbar to Reports: **Month / Quarter / Year / All-time**. This drives the time window for all charts and tables. Pass the selected scope into the `get_report_data` backend (update it to accept `scope: String`), which adjusts the SQL date filters accordingly.

### 10b. Donut / breakdown chart — ✅ DONE

Add a donut chart widget showing spending breakdown by category. Pure SVG — no library needed:
- Compute arc paths from category percentages using SVG `path` with `A` arc commands
- Center shows total spend
- Legend below with category name + percentage + amount

### 10c. Year-over-year comparison chart — ✅ DONE

Add a YoY line chart — two SVG polylines, this year vs last year, month by month. Requires the backend to also return last year's monthly totals. Update `get_report_data` to return `monthly_last_year: Vec<MonthSummary>`.

### 10d. Multiple saved report tabs (stretch) — ✅ DONE

Allow users to create named report tabs (like "Monthly overview", "Wealth", "Spending deep dive"), each storing a JSON config of which widgets are visible and their order. Persist to `settings` KV store as `report_tabs: [...] `. Add tab strip above the charts, "+" button to create, inline rename (double-click).

### 10e. Widget show/hide toggles — ✅ DONE

Short of full drag-and-drop, add a simple "Customize" mode (pencil icon in top-right) that toggles visibility of each widget (bar chart / line chart / category table / merchant table / donut). Persist visibility state to localStorage.

---

## 11. Rules screen — missing pieces

**Design reference:** `design/plutus/project/components/rules.jsx`

### 11a. Agent proposals section — ✅ DONE

The design shows a dashed-border card "Agent proposals" below the active rules list with 3–5 agent-suggested rules. Users can Accept or Decline each.

**Backend ✅ done:** `rule_proposals` table (V009, with `pattern` + `category_id` so accept can materialize a real rule). Commands `listRuleProposals` (pending only), `acceptRuleProposal(id)` (creates a `source:"agent"` rule + marks accepted), `declineRuleProposal(id)` exist (type `RuleProposal`). The categorizer already emits proposals as a post-run step when a merchant has ≥3 manual user categorizations to the same category (deduped against existing rules/pending proposals).

**Frontend (pending):** In `Rules.tsx`, add the "Agent proposals" card (dashed border, accent color) below the rules list. Each proposal row: context eyebrow (`whenLabel`), description text, "Accept" (btn.primary) and "Decline" (btn.ghost.sm) buttons.

### 11b. New rule manual builder ✅ DONE

Add a "New rule" button in the Rules header. Opens a small inline form or modal:
- Pattern input: text field with `%` wildcards, live preview matching against recent merchants
- Category picker: reuse `<CategoryPicker>` component
- Submit calls `create_rule(pattern, category_id)` (already exists)

### 11c. Agent 24h activity log — ✅ DONE

Add a "Agent · last 24h" card in the right sidebar of Rules (the design already has the two-column layout). Query: fetch the last 10 `categorizations` rows joined with transaction/category info, group into activity log entries. New command: `list_recent_agent_activity(limit: u32) -> Vec<AgentActivity>` where `AgentActivity = { text, sub, minutes_ago }`.

---

## 12. Settings screen — missing pieces

**Design reference:** `design/plutus/project/components/settings.jsx`

### 12a. Data export ✅ DONE

"Export all data" section with two buttons:
- **Export CSV** — exports transactions, accounts, categories as a ZIP of CSVs
- **Export JSON** — full data dump as JSON

```rust
async fn export_all_data_json() -> AppResult<String>  // returns temp file path
async fn export_all_data_csv() -> AppResult<String>   // returns temp file path
```
Use `tauri_plugin_dialog` to trigger a save dialog.

### 12b. Currency setting ✅ DONE

Add a "Default currency" setting (stored in the settings KV table):
```rust
async fn get_currency() -> AppResult<String>  // "USD"
async fn set_currency(currency: String) -> AppResult<()>
```
Show a select dropdown in Settings. Used as the fallback currency in `formatMoney` calls.

### 12c. Appearance section ✅ DONE

Move the theme/density/accent controls from the hidden `tweaks` store into a visible Settings section:
- Theme toggle (Light / Dark)
- Density toggle (Cozy / Compact)  
- Accent color picker (6 color swatches — already in `useTweaks`)
- These already work via `useTweaks()`; just render them in Settings UI

### 12d. Keyboard shortcuts reference — ✅ DONE

Add a static "Keyboard shortcuts" section listing:
- `⌘K` — Command palette
- `⌘.` — Toggle privacy mode
- (Future: `⌘,` — Settings, `⌘/` — Jump to today)

---

## 13. Insights screen — missing pieces

**Design reference:** `design/plutus/project/components/insights.jsx`

### 13a. Agent operator panel ✅ DONE

Add a "status bar" at the top of Insights:
```
[pulse dot] Agent · running locally     [cycling ticker: "Watching: Joint Checking balance · stable"]
                                         [Re-run scan button]
```
The cycling ticker is a `useEffect` interval that rotates through 5–6 "currently watching" messages every 2.4 seconds. The "Re-run scan" button calls `trigger_categorize()` (already exists), shows a brief "Scanning…" state, then a "Scan complete" toast.

### 13b. Agent memory section — ✅ DONE

Below the insight cards, add a "What the agent has learned" section.

**Backend ✅ done:** `agent_memory` table (V010, deduped by `merchant_key` via unique upsert). Commands `listAgentMemory`, `forgetAgentMemory(id)` exist (type `AgentMemory`). A `kind:"correction"` memory is upserted automatically whenever the user sets a transaction's category (wired into `transactions::update`).

**Frontend (pending):** Render as a list below insights: each row shows `description` + "Forget" button that calls `forgetAgentMemory` with an undo toast.

---

## 14. Command palette — missing pieces

**Design reference:** `design/plutus/project/components/command-palette.jsx`

### 14a. "Ask the agent" mode ✅ DONE

Add a third section in the command palette: **Ask the agent** (above "Jump to"). Pre-load 5 canned questions with answers:
```typescript
const AGENT_ASKS = [
  {
    label: "What did we spend on groceries this month vs last?",
    answer: { prose: "...", kind: "compareBars", data: { ... } }
  },
  // ... 4 more
]
```
When an Ask item is selected, switch the palette into "answer mode":
- The palette expands wider (`min(760px, 94vw)`)
- Show the prose answer text
- Below the prose, render a visualization based on `kind`:
  - `compareBars` — two horizontal bars with labels and amounts (pure CSS/div, no SVG needed)
  - `bigNumber` — large centered figure
  - `progress` — progress bar with ETA
- Show 1–2 action buttons ("Open [screen]") that navigate and close the palette
- "Back" button (top-right) returns to the list mode

The canned questions should be computed from real data at mount time (e.g., actually query `get_month_totals` and `list_categories_with_spending` to fill in the real numbers in the prose/data).

### 14b. Additional action items ✅ DONE

Add 3 more actions to the "Actions" section:
- "Export this month as CSV" — calls the transaction CSV export (see item 5c)
- "Run a what-if" — navigates to `/scenarios`
- "Plan next month" — opens the Plan Next Month modal

---

## 15. Sidebar — missing pieces

### 15a. Scenarios in nav ✅ DONE

Add Scenarios to `NAV_MAIN` in `Sidebar.tsx` and to `ROUTES` in `routes.ts`:
```typescript
{ id: "scenarios", path: "/scenarios", label: "Scenarios", Icon: I.Bolt }
```
Position: between Goals and Reports.

### 15b. Live transaction count badge ✅ DONE

Show a live count badge next to "Transactions" in the nav. Add a `get_transaction_count()` command that returns the total count. Display as a formatted badge (e.g., "1.2k"). Refetch every 60s.

### 15c. "Run setup again" footer item ✅ DONE

The design has a footer nav item that re-launches the onboarding flow. Already partially present in the codebase (`reset_onboarding_completion` command exists). Add a nav item to the sidebar footer:
```tsx
<div className="nav-item" onClick={() => { resetOnboarding.mutateAsync(); navigate("/onboarding"); }}>
  <I.Sparkle className="ico" />
  <span>Run setup again</span>
</div>
```

---

## Priority order

> Items marked ✅ are shipped. Remaining items re-ranked by value.

| Priority | Item | Effort | Value | Status |
|----------|------|--------|-------|--------|
| — | Transaction search + filter tabs (§5a, §5b) | Low | High | ✅ Done |
| — | Categories: year scope + budget column (§6a, §6b) | Low | Medium | ✅ Done |
| — | Budget: "To Budget" tracker + activity sort (§7a, §7b) | Low | Medium | ✅ Done |
| — | Recurring: day-detail panel (§8a) | Low | Medium | ✅ Done |
| — | Goals: pace chip (§9a) | Low | Medium | ✅ Done |
| — | Sidebar: count badge + run setup (§15b, §15c) | Low | Medium | ✅ Done |
| — | Scenarios screen (§1) | High | High | ✅ Done |
| — | Rules: agent proposals + manual builder (§11a, §11b) | Medium | High | ✅ Done |
| — | Command palette: Ask the agent + what-if action (§14a, §14b) | Medium | High | ✅ Done |
| — | Today: net-worth chart + recurring + Sweep + Runway (§3a–3d) | Medium | High | ✅ Done |
| — | Accounts: manual assets + liabilities (§4a, §4b) | Medium | Medium | ✅ Done |
| — | Settings: export + currency + appearance (§12a–12c) | Low | Medium | ✅ Done |
| — | Goals: apply what-if + sinking funds (§9b, §9c) | Low | Medium | ✅ Done |
| — | Insights: agent operator panel + memory (§13a, §13b) | Medium | Medium | ✅ Done |
| — | Categories: AI insight sentence (§6c) | Low | Medium | ✅ Done |
| — | Plan Next Month wizard (§2) | High | Medium | ✅ Done |
| — | Reports: scope switcher + donut + YoY (§10a, §10b, §10c) | Medium | Medium | ✅ Done |
| — | Budget: 5-month history strip (§7c) | Medium | Low | ✅ Done |
| — | Transactions: CSV export (§5c) | Low | Low | ✅ Done |
| — | Accounts: CSV export (§4c) | Low | Low | ✅ Done |
| — | Recurring: price-history chip (§8b) | Low | Low | ✅ Done |
| — | Rules: agent activity log (§11c) | Medium | Low | ✅ Done |
| — | Settings: keyboard shortcuts reference (§12d) | Low | Low | ✅ Done |
| — | Reports: saved tabs + widget customization (§10d, §10e) | Very High | Low (MVP) | ✅ Done |
| — | Transactions: reimbursable/split flags UI (§5d) | Low | Low | ✅ Done |
| — | Transaction splits UI + notification centre (Wave D Group 1) | Medium | High | ✅ Done |
| — | Real anomaly detection (IQR + LLM, `anomaly.rs`) | Medium | High | ✅ Done |
| — | Real agent status + data-driven Insights ticker | Low | Medium | ✅ Done |
| — | Free-text LLM ask in CommandPalette (`ask_agent`) | Low | High | ✅ Done |

---

## Technical notes for new agents

- **Run `cargo run -p finsight-tauri --bin export_bindings` from the repo root** (not from `ui/`) after adding any Tauri command, or bindings won't update.
- **CSS variables:** use `var(--ink)`, `var(--ink-mute)`, `var(--ink-faint)`, `var(--line)`, `var(--elevated)` etc. — all defined in `ui/src/styles/tokens.css`. Do NOT use hardcoded colors.
- **CSS component classes** like `.card`, `.chip`, `.btn`, `.tbl`, `.stat`, `.eyebrow`, `.toolbar`, `.stream`, `.goal-bar`, `.tog`, `.rule`, `.cond`, `.tok` are all in `ui/src/styles/app.css`.
- **Icons:** import from `ui/src/components/Icons.tsx`. If you need a new icon not in the file, add it there following the existing `icon()` factory pattern.
- **Toasts:** use `import { toast } from "sonner"` — use `toast.success()`, `toast.error()`, `toast("text", { description: "...", action: { label: "Undo", onClick: () => {} } })`.
- **Drawers:** reuse `ui/src/components/Drawer.tsx` for any slide-in panels.
- **All Rust commands** must have `#[tauri::command]` and `#[specta::specta]` attributes and `pub async fn` signature to be picked up by specta.
- **Migrations:** add new `.sql` files to `crates/finsight-core/migrations/` as `V00N__description.sql`. Refinery auto-discovers them by filename prefix ordering. Next = `V013__description.sql`.
- **Dev demo data:** `seed_dev_demo()` in `crates/finsight-core/src/sample.rs` loads the full "Mira & Adam" dataset (6 accounts, ~142 transactions, 5 goals, assets, liabilities, budgets, net-worth history). Exposed via the "Load demo data" button in Settings (visible only in `import.meta.env.DEV`). Idempotent — clears `source='sample'` data before re-seeding. Does NOT touch non-sample accounts.
- **Tests:** run `cd ui && npx vitest run` and `cargo test --workspace` before committing. 105 frontend tests and 103 Rust tests must stay green (0 TypeScript errors via `cd ui && npx tsc --noEmit`).

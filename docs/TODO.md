# FinSight — Implementation TODO

> **For new agents:** The design reference is at `design/plutus/project/components/` (JSX prototypes with full HTML/CSS/JS). The implementation is at `ui/src/screens/` (React + TypeScript + Tauri). Read the relevant design file before implementing each section. The design uses mock data (`FS.*`); the implementation must use real Tauri commands.
>
> **Stack:** Rust/Tauri 2 backend · React 18 + TypeScript + Vite frontend · SQLite/SQLCipher via rusqlite · tanstack-query hooks · sonner toasts · zod + react-hook-form · design tokens in `ui/src/styles/tokens.css` + `app.css`
>
> **Adding a Tauri command:** (1) write the function in `crates/finsight-app/src/commands/`, (2) register it in `crates/finsight-app/src/lib.rs` inside `build_specta_builder()`, (3) run `cargo run -p finsight-tauri --bin export_bindings` from the **repo root** to regenerate `ui/src/api/bindings.ts`.

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

## 2. Plan Next Month wizard (launched from Budget)

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

### 3a. Net-worth area chart with range selector

The design shows a SVG area chart tracing net-worth history with selectable ranges (1M / 3M / 6M / 1Y / All).

**Backend:** Add a V005 (or V006) migration for a `net_worth_snapshots` table:
```sql
CREATE TABLE net_worth_snapshots (
  id TEXT PRIMARY KEY,
  date TEXT NOT NULL UNIQUE,        -- ISO date 'YYYY-MM-DD'
  total_cents INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
```
Add a `record_net_worth_snapshot()` command that sums all account balances and inserts/updates today's row. Call it from `configure_app` setup or on-demand.
Add `list_net_worth_history(days: u32) -> Vec<{date, total_cents}>` command.

**Frontend:** In `Today.tsx`, above the stat row, render a SVG area chart (same pattern as `Reports.tsx`'s NetLine):
- Range toolbar: 1M / 3M / 6M / 1Y / All (defaults to 6M)
- Gradient fill under the line (lime at top, transparent at bottom — see `design/plutus/project/components/today.jsx` `NetWorthChart`)
- Last point glows with a radius-14 accent circle
- Month labels on X axis in `var(--mono)` font

### 3b. Smart Sweep suggestion card

When there is a positive net this month (income > expenses), show a card:
```
You have $X unallocated this month.
[Park in House Fund]  [Assign to a goal…]  [Dismiss]
```
Calls `update_goal_balance` or `set_budget` depending on action chosen.
Show only when `totals.netCents > 5000` and the user hasn't dismissed it this session (use `useState` — no persistence needed).

### 3c. Upcoming recurring items

Below the category stream bar, add a compact list of recurring items due in the next 7 days:
- Call `listRecurring()`, filter where `nextExpected` is within 7 days from today
- Render as a horizontal chip row: merchant initials dot + name + amount + days-until
- "See all" link to `/recurring`

### 3d. Runway stat in the stat row

Replace the "Accounts" stat card with a "Runway" stat:
- `runway_days = (total_account_balance) / avg_daily_burn`
- `avg_daily_burn` = `(expenses this month) / day_of_month`
- Display as "134 days" with sub-text "at current burn"

---

## 4. Accounts screen — missing pieces

**Design reference:** `design/plutus/project/components/accounts.jsx`

### 4a. Manual assets section

The design has a second section below bank accounts for manually tracked assets (home value, car, investment portfolio, crypto).

**Backend:** Add a `manual_assets` table (V005 migration):
```sql
CREATE TABLE manual_assets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  asset_type TEXT NOT NULL,  -- 'property' | 'vehicle' | 'investment' | 'crypto' | 'other'
  value_cents INTEGER NOT NULL DEFAULT 0,
  currency TEXT NOT NULL DEFAULT 'USD',
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```
Commands: `list_manual_assets`, `create_manual_asset`, `update_manual_asset`, `delete_manual_asset`.

**Frontend:** In `Accounts.tsx`, add a "Manual assets" section below the accounts table with:
- Asset cards showing: icon (house/car/chart/currency), name, value, type chip
- "Add manual asset" button opens a small form drawer
- Total assets + total accounts = net worth displayed at top

### 4b. Liabilities section

Similarly, track liabilities (mortgage, student loans, credit card balances).

**Backend:** Add `liabilities` table (same migration): `id, name, liability_type, balance_cents, limit_cents, apr_pct, payoff_date, currency, created_at`. Commands: `list_liabilities`, `create_liability`, `update_liability`, `delete_liability`.

**Frontend:** "Liabilities" section in `Accounts.tsx`:
- Each row: name, type chip, balance, APR, progress bar (balance / original limit), payoff date
- Net worth = accounts + assets − liabilities (update the Today screen hero too)

### 4c. CSV export per account

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

### 5c. CSV export

"Export CSV" button in the Transactions header that exports the currently filtered view:
```rust
async fn export_transactions_csv(filter: TxnFilterInput) -> AppResult<String>
```
Uses `tauri_plugin_dialog` to save, writes: date, merchant, category, amount, notes.

### 5d. Reimbursable and split flags (future/stretch)

Add `is_reimbursable BOOLEAN DEFAULT 0` and `is_split BOOLEAN DEFAULT 0` columns to `transactions` (V005 migration). Add toggle buttons in `TransactionDrawer`. Show chips in the transactions table for rows where these are true.

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

### 6c. AI insight sentence

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

### 7c. 5-month history strip

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

### 8b. Price-history chip per subscription

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

### 9b. Apply scenario from what-if slider

The "Apply" button in the what-if panel currently just resets the slider. Wire it to actually persist:
- Call `update_goal(id, { monthly_cents: goal.monthly_cents + extra })` — needs a new `update_goal` command that patches the monthly_cents field. Add to `commands/budget.rs`.
- Show a toast: "Applied +$X/mo to [Goal name] · ETA now [new date]"

### 9c. Sinking funds section

The design has a separate "Sinking funds" concept — short-term savings buckets with due dates (car registration, Christmas gifts, annual subscriptions). These are basically goals with `goal_type = 'save-by-date'` and a near-term date. The Goals table already supports this type.

**Frontend only:** Add a "Sinking funds" section below the main goal list that shows `goals.filter(g => g.goalType === 'save-by-date' && withinOneYear(g.targetDate))` in a 2-column compact card grid (tighter than the main goal cards).

---

## 10. Reports screen — missing pieces

**Design reference:** `design/plutus/project/components/reports.jsx`, `reports-widgets.jsx`, `reports-config.jsx`

The current Reports screen is a fixed layout. The design is a fully customizable widget dashboard. This is the largest remaining feature gap.

### 10a. Scope switcher

Add a scope toolbar to Reports: **Month / Quarter / Year / All-time**. This drives the time window for all charts and tables. Pass the selected scope into the `get_report_data` backend (update it to accept `scope: String`), which adjusts the SQL date filters accordingly.

### 10b. Donut / breakdown chart

Add a donut chart widget showing spending breakdown by category. Pure SVG — no library needed:
- Compute arc paths from category percentages using SVG `path` with `A` arc commands
- Center shows total spend
- Legend below with category name + percentage + amount

### 10c. Year-over-year comparison chart

Add a YoY line chart — two SVG polylines, this year vs last year, month by month. Requires the backend to also return last year's monthly totals. Update `get_report_data` to return `monthly_last_year: Vec<MonthSummary>`.

### 10d. Multiple saved report tabs (stretch)

Allow users to create named report tabs (like "Monthly overview", "Wealth", "Spending deep dive"), each storing a JSON config of which widgets are visible and their order. Persist to `settings` KV store as `report_tabs: [...] `. Add tab strip above the charts, "+" button to create, inline rename (double-click).

### 10e. Widget show/hide toggles

Short of full drag-and-drop, add a simple "Customize" mode (pencil icon in top-right) that toggles visibility of each widget (bar chart / line chart / category table / merchant table / donut). Persist visibility state to localStorage.

---

## 11. Rules screen — missing pieces

**Design reference:** `design/plutus/project/components/rules.jsx`

### 11a. Agent proposals section

The design shows a dashed-border card "Agent proposals" below the active rules list with 3–5 agent-suggested rules. Users can Accept or Decline each.

**Backend:** Add a `rule_proposals` table (migration):
```sql
CREATE TABLE rule_proposals (
  id TEXT PRIMARY KEY,
  when_label TEXT NOT NULL,   -- context label e.g. "Recurring"
  description TEXT NOT NULL,  -- human-readable rule description
  status TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'accepted' | 'declined'
  created_at TEXT NOT NULL
);
```
Commands: `list_rule_proposals`, `accept_rule_proposal(id)` (converts to a real rule), `decline_rule_proposal(id)`.

The agent categorizer (`crates/finsight-agent/src/categorizer.rs`) should emit proposals when it detects a clear pattern (e.g., the same merchant has been manually recategorized 3+ times). This can be a post-run step in the `CategorizeAll` job.

**Frontend:** In `Rules.tsx`, add the "Agent proposals" card (dashed border, accent color) below the rules list. Each proposal row: context eyebrow, description text, "Accept" (btn.primary) and "Decline" (btn.ghost.sm) buttons.

### 11b. New rule manual builder

Add a "New rule" button in the Rules header. Opens a small inline form or modal:
- Pattern input: text field with `%` wildcards, live preview matching against recent merchants
- Category picker: reuse `<CategoryPicker>` component
- Submit calls `create_rule(pattern, category_id)` (already exists)

### 11c. Agent 24h activity log

Add a "Agent · last 24h" card in the right sidebar of Rules (the design already has the two-column layout). Query: fetch the last 10 `categorizations` rows joined with transaction/category info, group into activity log entries. New command: `list_recent_agent_activity(limit: u32) -> Vec<AgentActivity>` where `AgentActivity = { text, sub, minutes_ago }`.

---

## 12. Settings screen — missing pieces

**Design reference:** `design/plutus/project/components/settings.jsx`

### 12a. Data export

"Export all data" section with two buttons:
- **Export CSV** — exports transactions, accounts, categories as a ZIP of CSVs
- **Export JSON** — full data dump as JSON

```rust
async fn export_all_data_json() -> AppResult<String>  // returns temp file path
async fn export_all_data_csv() -> AppResult<String>   // returns temp file path
```
Use `tauri_plugin_dialog` to trigger a save dialog.

### 12b. Currency setting

Add a "Default currency" setting (stored in the settings KV table):
```rust
async fn get_currency() -> AppResult<String>  // "USD"
async fn set_currency(currency: String) -> AppResult<()>
```
Show a select dropdown in Settings. Used as the fallback currency in `formatMoney` calls.

### 12c. Appearance section

Move the theme/density/accent controls from the hidden `tweaks` store into a visible Settings section:
- Theme toggle (Light / Dark)
- Density toggle (Cozy / Compact)  
- Accent color picker (6 color swatches — already in `useTweaks`)
- These already work via `useTweaks()`; just render them in Settings UI

### 12d. Keyboard shortcuts reference

Add a static "Keyboard shortcuts" section listing:
- `⌘K` — Command palette
- `⌘.` — Toggle privacy mode
- (Future: `⌘,` — Settings, `⌘/` — Jump to today)

---

## 13. Insights screen — missing pieces

**Design reference:** `design/plutus/project/components/insights.jsx`

### 13a. Agent operator panel

Add a "status bar" at the top of Insights:
```
[pulse dot] Agent · running locally     [cycling ticker: "Watching: Joint Checking balance · stable"]
                                         [Re-run scan button]
```
The cycling ticker is a `useEffect` interval that rotates through 5–6 "currently watching" messages every 2.4 seconds. The "Re-run scan" button calls `trigger_categorize()` (already exists), shows a brief "Scanning…" state, then a "Scan complete" toast.

### 13b. Agent memory section

Below the insight cards, add a "What the agent has learned" section. This requires a `agent_memory` table:
```sql
CREATE TABLE agent_memory (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,  -- 'preference' | 'pattern' | 'correction'
  description TEXT NOT NULL,  -- e.g. "AMZN MKTPL should be Shopping (you corrected 3 times)"
  created_at TEXT NOT NULL
);
```
When the user corrects a categorization in `TransactionDrawer`, insert a memory entry. Commands: `list_agent_memory`, `forget_agent_memory(id)`.

Render as a list below insights: each row shows the description + "Forget" button that calls `forget_agent_memory` with undo toast.

---

## 14. Command palette — missing pieces

**Design reference:** `design/plutus/project/components/command-palette.jsx`

### 14a. "Ask the agent" mode

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

### 14b. Additional action items

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
| — | Scenarios screen (§1) | High | High — design centrepiece | ✅ Done |
| 2 | Rules: agent proposals + manual new-rule builder (§11a, §11b) | Medium | High | |
| 3 | Command palette: Ask the agent mode (§14a) | Medium | High — design showpiece | |
| 4 | Today: net-worth chart + upcoming recurring (§3a, §3c) | Medium | High | |
| 5 | Accounts: manual assets + liabilities (§4a, §4b) | Medium | Medium | |
| 6 | Settings: data export + appearance section (§12a, §12c) | Low | Medium | |
| 7 | Goals: apply what-if (§9b) | Low | Medium | |
| 8 | Insights: agent operator panel + memory (§13a, §13b) | Medium | Medium | |
| 9 | Reports: scope switcher + donut + YoY (§10a, §10b, §10c) | Medium | Medium | |
| 10 | Today: Smart Sweep card + Runway stat (§3b, §3d) | Low | Medium | |
| 11 | Command palette: additional actions (§14b) | Low | Low | |
| 12 | Plan Next Month wizard (§2) | High | Medium | |
| 13 | Transactions: CSV export (§5c) | Low | Low | |
| 14 | Accounts: CSV export (§4c) | Low | Low | |
| 15 | Rules: agent activity log (§11c) | Medium | Low | |
| 16 | Reports: saved tabs + widget customization (§10d, §10e) | Very High | Low (MVP) | |

---

## Technical notes for new agents

- **Run `cargo run -p finsight-tauri --bin export_bindings` from the repo root** (not from `ui/`) after adding any Tauri command, or bindings won't update.
- **CSS variables:** use `var(--ink)`, `var(--ink-mute)`, `var(--ink-faint)`, `var(--line)`, `var(--elevated)` etc. — all defined in `ui/src/styles/tokens.css`. Do NOT use hardcoded colors.
- **CSS component classes** like `.card`, `.chip`, `.btn`, `.tbl`, `.stat`, `.eyebrow`, `.toolbar`, `.stream`, `.goal-bar`, `.tog`, `.rule`, `.cond`, `.tok` are all in `ui/src/styles/app.css`.
- **Icons:** import from `ui/src/components/Icons.tsx`. If you need a new icon not in the file, add it there following the existing `icon()` factory pattern.
- **Toasts:** use `import { toast } from "sonner"` — use `toast.success()`, `toast.error()`, `toast("text", { description: "...", action: { label: "Undo", onClick: () => {} } })`.
- **Drawers:** reuse `ui/src/components/Drawer.tsx` for any slide-in panels.
- **All Rust commands** must have `#[tauri::command]` and `#[specta::specta]` attributes and `pub async fn` signature to be picked up by specta.
- **Migrations:** add new `.sql` files to `crates/finsight-core/migrations/` as `V00N__description.sql`. Refinery auto-discovers them by filename prefix ordering.
- **Tests:** run `cd ui && npx vitest run` and `cargo test --workspace` before committing. 51 frontend tests and all Rust tests must stay green.

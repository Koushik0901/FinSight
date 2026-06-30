# UX & Usefulness Improvements Handoff

Date: 2026-06-29

Plan source: `docs/superpowers/plans/2026-06-28-ux-usefulness-improvements.md`

This handoff documents the high-impact UX and usefulness improvements shipped across all 11 plan areas (A1–F2). The work covers critical data-access gaps, AI/Copilot depth, financial health scoring, debt payoff planning, monthly review workflow, and savings/milestone tracking.

---

## Migrations added

| File | Purpose |
|---|---|
| `crates/finsight-core/migrations/V027__perf_indexes.sql` | Performance indexes: `idx_transactions_posted_at`, `idx_transactions_account_posted` |
| `crates/finsight-core/migrations/V028__monthly_reviews_milestones.sql` | `monthly_reviews` table + `net_worth_milestones` table |

---

## A. Transaction Data Access

### A1 — Date Range + Account Filter

**Backend changes:**
- `crates/finsight-core/src/repos/transactions.rs` — `TxnFilter` gained `start_date: Option<String>` and `end_date: Option<String>`; `list()` adds `t.posted_at >= ?` / `t.posted_at <= ?` conditions when set.
- `crates/finsight-app/src/commands/transactions.rs` — `TxnFilterInput` gained `start_date`/`end_date`; both `list_transactions` and `export_transactions_csv` pass them through.

**Frontend changes (`ui/src/screens/Transactions.tsx`):**
- Added `<input type="date">` pickers for start/end date wired into the filter.
- Added account dropdown using `useAccounts()` that filters by `accountId`.

### A2 — Bulk Transaction Categorization

**Frontend changes (`ui/src/screens/Transactions.tsx`):**
- Per-row checkboxes + "Select all" header checkbox; shift-click for range selection.
- Bulk action toolbar appears when ≥1 transactions selected: category picker + "Categorize as…" button, "Dismiss review flag" button, selected count badge.
- Calls `commands.updateTransaction()` in a loop; shows toast with count on completion.

### A3 — Import Reconciliation Workbench UI

**New screen (`ui/src/screens/ImportReview.tsx`):**
- Lists all pending `import_candidates` via `useImportReviewCandidates()`.
- Each candidate shows: date, merchant, amount, account, confidence score, reason.
- Candidates with matches show a recommended match side-by-side with "Accept match" CTA; additional matches listed below.
- Three actions per candidate: **Accept match** (`useAcceptImportCandidateMatch`), **Create new transaction** (`useCreateImportCandidateTransaction`), **Dismiss** (`useDismissImportCandidate`).
- Empty state when queue is clear.

**Route:** `<Route path="/import-review" element={<ImportReview />} />` added to `App.tsx`.

**Sidebar badge:** `ui/src/components/Sidebar.tsx` reads `useImportReviewCandidates()` and shows a numeric badge on the Inbox nav item when pending candidates exist.

---

## B. AI / Copilot Depth

### B1 — Persistent Copilot Session List

**Backend changes (`crates/finsight-app/src/commands/copilot.rs`):**
- `list_action_bundles` now accepts `session_id: Option<String>` to filter bundles by session.
- `crates/finsight-core/src/repos/copilot_actions.rs` — `list_bundles` gained `session_id` parameter.

**Frontend changes (`ui/src/screens/Copilot.tsx`):**
- Left sidebar panel listing past sessions from `list_agent_sessions()` with title + timestamp.
- Clicking a session loads its bundles in a read-only history view.
- "New chat" button clears current conversation and creates a fresh session.
- Active session highlighted in the panel.

### B2 — Context-Aware Copilot Quick-Access Button

**New component (`ui/src/components/CopilotQuickAsk.tsx`):**
- Fixed floating button (bottom-right, 44×44px circle) that opens a `Drawer` slide-in panel.
- Drawer contains a pre-filled textarea with a context-rich prompt + "Ask Copilot" button.
- Calls `invoke("ask_agent", { question, mode: "deep" })` and renders the response using `AgentResponseRenderer`.
- Props: `prompt: string`, `label?: string`.

**Added to screens:**
- `Today.tsx`: "Based on my spending this month…"
- `Budget.tsx`: "My [X] category is over budget…"
- `Goals.tsx`: "My goal [X] is [Y]% complete…"
- `Transactions.tsx`: "Analyze my recent transactions…"

### B3 — Copilot Memory Panel

**Frontend changes (`ui/src/screens/Copilot.tsx`):**
- "Memory" tab added alongside the main chat area (tabs: **Chat** | **Memory** | **History**).
- Memory tab lists `agent_memory` rows via `useAgentMemory()` as readable cards: kind badge, content, confidence, date, "Forget" button wired to `useForgetAgentMemory()`.
- History tab shows past bundles (moved from the inline `PastBundlesSection`).

---

## C. Financial Health Score

### C1 — Backend (`crates/finsight-app/src/commands/insights.rs`)

New command `get_financial_health_score` returns `HealthScore`:

| Component | Weight | Logic |
|---|---|---|
| Savings rate | 25 pts | ≥10% = 25, 5–10% = 15, <5% = 0 |
| Emergency fund | 25 pts | ≥6 months = 25, 3–6 = 15, 1–3 = 8, <1 = 0 |
| Debt-to-income ratio | 20 pts | 0 debt = 20, <20% DTI = 15, 20–40% = 8, >40% = 0 |
| Goal progress | 15 pts | avg pct_complete across active goals, scaled to 0–15 |
| Budget adherence | 15 pts | 0 overages = 15, ≤2 = 8, ≤4 = 4, >4 = 0 |

Score 0–100 → grade A (85+) / B (70+) / C (55+) / D (40+) / F.
Returns top-3 improvement tips from weakest components.

Registered as `commands::insights::get_financial_health_score`.

### C2 — Frontend (`ui/src/screens/Today.tsx`)

- `useHealthScore()` hook added to `ui/src/api/hooks/insights.ts`.
- `HealthScoreCard` component: CSS conic-gradient circular gauge, letter grade overlay, 3 improvement tips as a bullet list.
- Inserted between the stat row and the spending category stream on the Today screen.

---

## D. Debt Payoff Planner

### D1 — Backend (`crates/finsight-app/src/commands/assets.rs`)

New command `compute_debt_payoff(extra_monthly_cents: i64)` returns `Vec<DebtPayoffResult>` for both strategies:

- **Snowball**: pays smallest balance first, rolls freed payment to next debt.
- **Avalanche**: pays highest APR first, rolls freed payment to next debt.

Each result includes: `total_interest_cents`, `total_months`, `payoff_date_label`, and per-liability `DebtPayoffSummary` (interest paid, months to payoff).

Registered as `commands::assets::compute_debt_payoff`.

### D2 — Frontend (`ui/src/screens/Goals.tsx`)

- "Debt Payoff" tab added to the goal type filter toolbar.
- Extra monthly payment input (slider + number input).
- Side-by-side Snowball vs Avalanche comparison cards: total interest saved, payoff date, months to debt-free.
- Per-liability breakdown table: current balance, APR, min payment, estimated payoff date.
- "Create Debt Payoff Goal" CTA pre-fills a `debt-payoff` goal.

---

## E. Monthly Financial Review

### E1 — Backend (`crates/finsight-app/src/commands/reports.rs`)

New commands:
- `create_monthly_review(input: CreateMonthlyReviewInput)` — computes a denormalized snapshot (income, expenses, savings rate, over-budget categories) and persists to `monthly_reviews`.
- `list_monthly_reviews()` — returns reviews ordered by year/month DESC.

`MonthlyReviewSnapshot` is stored as JSON in `snapshot_json` column and deserialized on read.

Registered as `commands::reports::create_monthly_review` and `commands::reports::list_monthly_reviews`.

### E2 — Frontend (`ui/src/screens/Today.tsx`)

- On days 28–31 of the month, a "Month in Review" card appears prompting the user to save their review.
- Clicking "Save review" calls `useCreateMonthlyReview()` with current year/month + optional notes textarea.
- Saved reviews appear in `Reports` screen under a new "Review History" tab (via `useListMonthlyReviews()`).

---

## F. Savings Rate Trend & Net Worth Milestones

### F1 — Savings Rate Sparkline

**Backend (`crates/finsight-app/src/commands/reports.rs`):**
- `get_savings_rate_history()` returns `Vec<SavingsRatePoint>` for the last 12 months; each point: `month`, `savings_rate_pct`, `income_cents`, `expense_cents`.

**Frontend (`ui/src/screens/Today.tsx`):**
- `useSavingsRateHistory()` hook wired to the health score card.
- Sparkline rendered as a mini SVG polyline below the savings rate pill; green if trending up, red if down, gray if flat.

### F2 — Net Worth Milestone Celebrations

**Backend (`crates/finsight-app/src/commands/assets.rs`):**
- `get_uncelebrated_milestones()` checks current net worth against thresholds ($10k / $25k / $50k / $100k / $250k / $500k / $1M), records any newly crossed thresholds in `net_worth_milestones`, and returns the list of newly achieved ones.

**Frontend (`ui/src/screens/Today.tsx`):**
- `useUncelebratedMilestones()` called on Today load.
- If any new milestones exist, a congratulatory card renders: "🎉 Net worth crossed $X!" with a dismiss button.

---

## New hooks added

| Hook | File | Wraps command |
|---|---|---|
| `useHealthScore` | `ui/src/api/hooks/insights.ts` | `get_financial_health_score` |
| `useSavingsRateHistory` | `ui/src/api/hooks/index.ts` | `get_savings_rate_history` |
| `useCreateMonthlyReview` | `ui/src/api/hooks/index.ts` | `create_monthly_review` |
| `useListMonthlyReviews` | `ui/src/api/hooks/index.ts` | `list_monthly_reviews` |
| `useComputeDebtPayoff` | `ui/src/api/hooks/budget.ts` | `compute_debt_payoff` |
| `useUncelebratedMilestones` | `ui/src/api/hooks/assets.ts` | `get_uncelebrated_milestones` |

---

## Verification results

| Check | Result |
|---|---|
| `cargo build --workspace` | ✅ 0 errors |
| `cargo test --workspace` | ✅ all tests pass |
| `cargo run -p finsight-tauri --bin export_bindings` | ✅ bindings regenerated |
| `cd ui && npx tsc --noEmit` | ✅ 0 TypeScript errors |
| `cd ui && npx vitest run` | ✅ 163 tests / 36 files passing |

---

## State after this session

- **Current latest migration:** V028 (`monthly_reviews_milestones`). Next = `V029__…`.
- **Rust test count:** ~215 (all workspace crates)
- **Frontend test count:** 163 tests / 36 files
- No breaking changes to existing commands or data shapes.
- All monetary amounts in new UI components use `className="money"` for privacy mode.
- No hardcoded colors — all new components use design tokens.

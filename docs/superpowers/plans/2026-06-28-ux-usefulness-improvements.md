# FinSight — High-Impact UX & Usefulness Improvements

## Problem Statement

FinSight has a solid foundation — 14 screens, AI categorization, goals, scenarios, and a Financial Freedom Framework. However, several critical gaps reduce its genuine usefulness: transaction data access is limited (no date range filter), the Copilot loses all conversation history, the app lacks a "North Star" financial health metric, and key financial tools (debt payoff planning, monthly reviews) are missing despite the data being available in the DB.

Focus: **data features · AI/Copilot depth · financial planning tools** | All tiers of effort | Built for personal use now, distributable eventually.

---

## Improvement Areas

### Area A — Transaction Data Access (Critical Gap)

**A1. Date Range + Account Filter on Transactions Screen**
Currently the transaction list is locked to `limit: null` with no date range or account filtering. Users cannot answer "how much did I spend on food in Q3?" or "show me only my checking account."
- Add `startDate`, `endDate`, `accountId` to `TxnFilterInput` Rust struct + `list_transactions` command
- Add V027 migration: add index on `transactions(posted_at, account_id)` for performance
- Add date range picker + account dropdown to Transactions screen header
- Run export_bindings after Rust changes

**A2. Bulk Transaction Categorization**
The "Needs review" tab shows uncategorized transactions, but each requires individual click → drawer → category pick → save. Power users with 50+ uncategorized transactions are stuck.
- Add multi-select checkboxes to transaction table rows (shift-click range select)
- Add bulk action toolbar: "Categorize selected as…" (CategoryPicker), "Dismiss review flag"
- Wire to existing `set_transaction_category` command called in a loop with toast progress

**A3. Import Reconciliation Workbench UI**
V026 migration (`import_reconciliation_workbench`) exists with full DB schema, but there is zero UI for it. This means import conflict resolution has no workflow.
- Add `/import-review` route accessible from Inbox (after SimpleFIN sync or CSV import)
- Show pending candidates with proposed matches side-by-side (existing `import_review_candidates` table)
- Wire accept/create/dismiss to existing hooks (`useAcceptImportCandidateMatch`, `useCreateImportCandidateTransaction`, `useDismissImportCandidate`) — these hooks already exist in `ui/src/api/hooks/simplefin.ts`
- Add an "Import review" badge count to Inbox in the sidebar

---

### Area B — AI/Copilot Depth (Highest ROI)

**B1. Persistent Copilot Session List**
The `copilot_sessions` table exists and `persist_plan` saves each conversation turn, but the Copilot UI shows no history — every visit loses previous AI conversations.
- Add `list_copilot_sessions` Rust command (list sessions by created_at DESC, with last message preview)
- Add `get_copilot_session` command to retrieve full turn history for a session
- Add a left panel on the Copilot screen: session list with timestamps + first message preview
- Clicking a past session restores it as read-only context; new sessions are created fresh
- Add "New chat" button to start a fresh session

**B2. Context-Aware Copilot Quick-Access Button**
Currently, to ask the Copilot about something you see on screen, you must: navigate to Copilot → type a prompt from scratch. This 3-step friction kills the most natural use of AI.
- Add a `<CopilotQuickAsk>` floating button to each major screen (Today, Budget, Goals, Transactions, Insights)
- On click: open a slide-in panel (Drawer) with a pre-filled, context-rich prompt + message input
- Context injected per screen: Today → "Based on my spending this month…", Budget → "My [X] category is [Y]% used…", Goals → "My [goal] is [Z]% complete…"
- Submits via existing `invoke("ask_copilot")` Tauri command; response shown in-drawer

**B3. Copilot Memory Panel on Copilot Screen**
Agent memories (stored via `agent_memory` table) are only visible buried in Insights. The Copilot should show what it "knows" about you.
- Add a "Memory" tab to the Copilot screen alongside the main chat
- Display memory items as readable cards with confidence + date + forget button
- This reuses the existing `useAgentMemory` and `useForgetAgentMemory` hooks — mostly UI work

---

### Area C — Financial Health Score (North Star Metric)

**C1. Financial Health Score — Backend**
No single metric exists that tells a user "how financially healthy am I right now?". This is the single most impactful addition for user engagement.

Algorithm (0–100 score, weighted):
- Savings rate (25 pts): 10%+ = 25, 5-10% = 15, <5% = 0
- Emergency fund (25 pts): 6+ months = 25, 3-6 = 15, 1-3 = 8, <1 = 0
- Debt-to-income ratio (20 pts): 0 debt = 20, <20% = 15, 20-40% = 8, >40% = 0
- Goal progress (15 pts): avg % progress across active goals, scaled
- Budget adherence (15 pts): % of envelopes within budget, scaled

Implementation:
- Add `health_score` function in `crates/finsight-agent/src/context.rs` (it already computes all sub-metrics via `wellness_context()`)
- Add `get_financial_health_score` Tauri command in `crates/finsight-app/src/commands/`
- Return `HealthScore { total: u8, breakdown: HealthScoreBreakdown }` with component scores + explanations
- Run export_bindings

**C2. Financial Health Score — UI**
- Add a prominent "Health Score" card to the **Today** screen (between net worth chart and stat row)
- Show score as a circular gauge (0–100) with letter grade (A/B/C/D/F)
- Show 3 highest-priority improvement tips (from breakdown components that are below target)
- Add a small health score sparkline to **Journey** screen next to milestone progress
- Hook updates whenever `month-totals`, `goals`, or `liabilities` query keys change

---

### Area D — Debt Payoff Planner

**D1. Debt Payoff Planner — Backend**
The `liabilities` table has `balance_cents`, `interest_rate`, `min_payment_cents`. The Goals screen links goals to liabilities. Yet there's no computation of payoff schedules.
- Add `compute_debt_payoff` Tauri command: takes extra monthly payment amount, returns both Snowball and Avalanche schedules
- Schedule = array of `{ month, liability_name, payment_cents, remaining_balance, interest_paid }` per debt per month
- Return: total interest saved (avalanche vs snowball), payoff date for each, total months to debt-free
- This is pure computation — no new DB migration needed

**D2. Debt Payoff Planner — UI (new section in Goals screen)**
- Add "Debt Payoff" tab to Goals screen (alongside existing goal type tabs)
- Show each liability with current balance, rate, min payment, and estimated payoff date at min payment
- Input: "Extra monthly payment" slider/input → recalculate both strategies
- Chart: dual-line (snowball vs avalanche) cumulative balance over time
- CTA: "Create a Debt Payoff Goal" → pre-fill a `debt-payoff` goal with the computed schedule

---

### Area E — Monthly Financial Review Workflow

**E1. Monthly Review — Backend**
No guided monthly review exists. This creates accountability and historical record.
- Add V027 migration: `monthly_reviews` table — `(id, year, month, notes, snapshot_json, created_at)`
- `snapshot_json` stores a denormalized summary (income, expenses, savings_rate, over_budget_categories, goal_progress)
- Add `create_monthly_review` and `list_monthly_reviews` Tauri commands
- The snapshot is computed at review-creation time from current month's data

**E2. Monthly Review — UI**
- Add "Month in Review" modal (triggered from Today screen on the last 3 days of month or via Reports)
- Step 1 — Income & Expenses: show monthly summary vs last month with % change
- Step 2 — Budget: highlight over/under budget categories
- Step 3 — Goals: show each goal's progress and monthly contribution
- Step 4 — Notes: free text field for personal reflection
- Step 5 — Summary: compute and show health score, congratulate wins, flag concerns
- On save: persist snapshot to DB. Saved reviews appear in a new "Review History" tab in Reports screen.

---

### Area F — Savings Rate Trend & Net Worth Milestones

**F1. Savings Rate Sparkline on Today Screen**
The `net_worth_snapshots` table tracks monthly net worth, and `month_totals` gives income/expenses. We can compute savings rate per month historically.
- Add `get_savings_rate_history` command: returns array of `{ month, savings_rate_pct }` for last 12 months
- Add a small sparkline chart to the Today screen beneath the savings rate pill
- Color: green if trending up, red if trending down, gray if flat

**F2. Net Worth Milestone Celebrations**
Crossing $10k, $25k, $50k, $100k, $250k, $500k, $1M net worth for the first time should be celebrated — it drives long-term engagement.
- Add V028 migration: `net_worth_milestones` table — `(threshold_cents, achieved_at)`, unique per threshold
- In the net worth snapshot job (runs after each sync/calculation), check if any new thresholds are crossed; insert if so and generate an Inbox action item
- On Today screen load: check for newly crossed milestones and surface a congratulatory card (dismissable)
- Integrate milestone achievements into the Journey screen as bonus "sub-milestones"

---

## Implementation Order (Priority Queue)

| Priority | ID | Area | Impact | Effort |
|---|---|---|---|---|
| P1 | A3 | Import Reconciliation Workbench UI | Unblocks existing V026 backend | Low (hooks exist) |
| P1 | B1 | Persistent Copilot Session List | Critical AI memory gap | Medium |
| P1 | C1+C2 | Financial Health Score | North Star metric | Medium |
| P1 | A1 | Transaction Date Range + Account Filter | Core data access | Medium |
| P2 | B2 | Context-Aware Copilot Quick-Access | AI friction reduction | Low-Medium |
| P2 | D1+D2 | Debt Payoff Planner | Fulfills Ramsey framework | Medium |
| P2 | A2 | Bulk Transaction Categorization | Power user feature | Medium |
| P2 | E1+E2 | Monthly Financial Review | Accountability loop | High |
| P3 | B3 | Copilot Memory Panel | AI transparency | Low |
| P3 | F1 | Savings Rate Sparkline | Visual trend | Low |
| P3 | F2 | Net Worth Milestones | Engagement/celebration | Medium |

---

## Notes & Constraints

- Next migration file: `V027__...sql` (current latest is V026)
- All new Tauri commands must be `async`, decorated with `#[tauri::command]` and `#[specta::specta]`
- After any Rust command change: `cargo run -p finsight-tauri --bin export_bindings`
- All monetary amounts in UI must use `className="money"` for privacy mode compatibility
- No hardcoded colors — use design tokens from `tokens.css`
- Import from `ui/src/api/client.ts`, never directly from `bindings.ts`
- Test baseline to maintain: 214 Rust tests, 161 frontend tests, 0 TypeScript errors

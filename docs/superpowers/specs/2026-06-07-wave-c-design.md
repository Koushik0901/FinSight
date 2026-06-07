# Wave C — Design Spec
**Date:** 2026-06-07  
**Scope:** 8 remaining features from `docs/TODO.md`, priority order 1–8. Wave D (§10d–10e full widget dashboard) is explicitly deferred.

---

## Items in this wave

| # | Item | Section | Effort |
|---|------|---------|--------|
| 1 | Plan Next Month wizard | §2 | High |
| 2 | Reports: scope switcher + donut + YoY | §10a–10c | Medium |
| 3 | Budget: 5-month history strip | §7c | Medium |
| 4 | Rules: agent activity log | §11c | Medium |
| 5 | Transactions: CSV export | §5c | Low |
| 6 | Accounts: CSV export | §4c | Low |
| 7 | Recurring: price-history chip | §8b | Low |
| 8 | Settings: keyboard shortcuts | §12d | Very low |

---

## 1. Plan Next Month Wizard (§2)

### Backend — `crates/finsight-app/src/commands/budget.rs`

Two new commands.

#### Types

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPlanRow {
    pub category_id: String,
    pub label: String,
    pub color: String,
    pub group_label: String,
    pub budget_cents: i64,   // current month's budget (0 = none set)
    pub m0_cents: i64,       // this month's outflow
    pub m1_cents: i64,       // last month's outflow
    pub m2_cents: i64,       // two months ago outflow
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanData {
    pub income_cents: i64,                // avg of last 3 months income
    pub categories: Vec<CategoryPlanRow>, // all non-archived categories
    pub goals: Vec<GoalDto>,              // active goals (current < target)
    pub recurring_expense_cents: i64,     // sum of detected monthly recurring outflows
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanAssignment {
    pub category_id: String,
    pub amount_cents: i64,
}
```

#### `get_plan_next_month_data() -> AppResult<PlanData>`

- **income_cents**: `SELECT AVG(monthly_income) FROM (SELECT SUM(amount_cents) FROM transactions WHERE amount_cents > 0 GROUP BY strftime('%Y-%m', posted_at) ORDER BY strftime('%Y-%m', posted_at) DESC LIMIT 3)`
- **categories**: All non-archived categories joined with spending for current month (m0), last month (m1), and two months ago (m2) via three left-join subqueries or a pivot on `strftime('%Y-%m', posted_at)`. Budget pulled from `budgets` table for current month.
- **goals**: `goals::list` filtered to `current_cents < target_cents` and not archived.
- **recurring_expense_cents**: Inline query — same merchant-grouping logic as `list_recurring` but just sums `ABS(last_amount_cents)` for items with `avg_gap_days < 45` (monthly-or-shorter cadence).

#### `apply_next_month_plan(assignments: Vec<PlanAssignment>) -> AppResult<()>`

- Computes next month as `Utc::now() + 1 month` formatted as `YYYY-MM`.
- For each `PlanAssignment`: calls `budgets::set(conn, &category_id, &next_month, amount_cents)`.
- Skips assignments with `amount_cents == 0`.

### Frontend — `ui/src/screens/PlanNextMonthModal.tsx` (new file)

Full-screen overlay, `z-index: 70`. Reuses the `onb-shell` / `onb-top` / `onb-body` / `onb-foot` / `onb-left` / `onb-right` CSS classes from `app.css` (same structure as Onboarding).

#### Step state

```typescript
type StepId = "review" | "basics" | "notyet" | "buffer" | "pulls" | "adjust";
const STEPS: { id: StepId; label: string }[] = [
  { id: "review",  label: "Look back" },
  { id: "basics",  label: "The basics" },
  { id: "notyet",  label: "The not-yet" },
  { id: "buffer",  label: "Breathing room" },
  { id: "pulls",   label: "The pulls" },
  { id: "adjust",  label: "Adjust & Done" },
];
```

State:
- `step: number` — current step index (0–5)
- `assignments: Record<string, number>` — `{ [categoryId]: amountCents }` — updated by steps 2, 3, 5, 6
- `buffer: number` — buffer slider value in cents (default 80_000)
- `goalContribs: Record<string, number>` — `{ [goalId]: monthlyCents }` pre-filled from `goal.monthlyCents`
- `toggledSuggestions: Set<string>` — category IDs whose suggestions are toggled on

#### Step 1 — Look back

Readonly bar list of last month's category spending (sort by `m1Cents` desc). Each row: color swatch + label + formatted amount + proportional inline bar (width = `m1Cents / maxM1Cents * 100%`). Caption eyebrow above: "How last month actually played out."

#### Step 2 — The basics

Categories where `groupLabel.toLowerCase().includes("fixed")`. If no categories match, skip this step and show all categories in step 3 instead. Each row: color swatch + label + "3-month avg $X" subtext + inline `<input type="number">` pre-filled with `budgetCents || m1Cents`. Input change updates `assignments[categoryId]`. Running total chip at bottom.

#### Step 3 — The not-yet

Same layout as step 2 but for remaining categories (non-fixed groups). Inline number inputs update `assignments[categoryId]`.

#### Step 4 — Buffer

Single range slider, `min=0 max=200000 step=5000` (represents cents). Display value as formatted dollars. Descriptive sentence updates with slider: *"At $X, you start [next month] with about Y days of typical spend already covered."* where `Y = buffer / avgDailyBurn` (avgDailyBurn from plan data).

#### Step 5 — Pulls

One range slider per active goal (`goalType !== "spending-cap"`). `min=0 max=200000 step=5000`. Pre-filled from `goal.monthlyCents`. Each row: goal name + "current / target · ETA" subtext + formatted amount label. Updates `goalContribs[goalId]`. Chip at bottom: "Toward goals: $X/mo".

#### Step 6 — Adjust & Done

**Suggestion computation (pure client-side):**

```typescript
function computeSuggestions(categories: CategoryPlanRow[]): BudgetSuggestion[] {
  return categories.flatMap(c => {
    if (c.budgetCents === 0) return [];
    const months = [c.m0Cents, c.m1Cents, c.m2Cents];
    const avg = months.reduce((s, v) => s + v, 0) / 3;
    // Hit budget (≥90%) in 2+ of last 3 months → suggest raising
    if (months.filter(m => m >= c.budgetCents * 0.9).length >= 2) {
      const suggested = Math.ceil((avg * 1.1) / 5000) * 5000; // round up to $50
      if (suggested > c.budgetCents) return [{ categoryId: c.categoryId, label: c.label,
        color: c.color, current: c.budgetCents, suggested,
        why: `Hit budget ${months.filter(m => m >= c.budgetCents * 0.9).length}/3 months — lift to match reality.` }];
    }
    // Averaged <50% of budget in 2+ months → suggest lowering
    if (months.filter(m => m < c.budgetCents * 0.5).length >= 2) {
      const suggested = Math.max(Math.ceil((avg * 1.1) / 5000) * 5000, 0);
      if (suggested < c.budgetCents) return [{ categoryId: c.categoryId, label: c.label,
        color: c.color, current: c.budgetCents, suggested,
        why: `Averaged ${Math.round((avg / c.budgetCents) * 100)}% of budget. Free up the headroom.` }];
    }
    return [];
  }).slice(0, 4);
}
```

Each suggestion renders as a toggleable card (dashed border, accent on selected): color swatch + label + "was $X → now $Y" + why text + toggle. Toggling a suggestion on writes `suggested` into `assignments[categoryId]`.

Below suggestions: review table with all assigned categories (label, amount). Below table: "Apply to [next month name]" primary button → calls both:
1. `applyNextMonthPlan({ assignments: flattenedCategoryAssignments })` — upserts budget rows
2. `updateGoalMonthly(id, monthlyCents)` for each goal whose `goalContribs[id]` differs from the original `goal.monthlyCents`

Success state: "Plan applied — {N} categories budgeted for [next month]."

The buffer is informational only and is not persisted (no table for it).

#### Right preview panel (all steps)

```
Live preview · [next month name]
──────────────────────────────
Income              $X,XXX

[stacked proportional bar]

  ● Fixed            $X,XXX
  ● Set-asides       $X,XXX
  ● Buffer             $XXX
  ● Goals            $X,XXX
  ● Daily life       $X,XXX
  ──────────────────────────
  Unassigned         $X,XXX   ← accent color, or negative color if over
```

Below preview: step checklist (6 dots, filled/active/future states matching current step).

Stacked bar segments activate progressively as steps are completed. Segment widths = `value / incomeCents * 100%`. If `unassigned < 0`, the overflow is shown as a negative-colored overflow extension.

#### Wiring

In `Budget.tsx`, add "Plan next month" button in the header (right of the toolbar). State: `const [planOpen, setPlanOpen] = useState(false)`. Render `<PlanNextMonthModal open={planOpen} onClose={() => setPlanOpen(false)} />` at bottom of the screen.

#### New hook

`usePlanNextMonthData()` in `ui/src/api/hooks/budget.ts` — standard tanstack-query wrapping `getPlanNextMonthData`.  
`useApplyNextMonthPlan()` — mutation wrapping `applyNextMonthPlan`, invalidates `["budget-envelopes"]` on success.

---

## 2. Reports: Scope + Donut + YoY (§10a–10c)

### Backend — `crates/finsight-app/src/commands/reports.rs`

#### Changes to `ReportData`

Add one field:
```rust
pub struct ReportData {
    pub monthly: Vec<MonthSummary>,
    pub monthly_last_year: Vec<MonthSummary>, // NEW — same months, prior year
    pub top_categories: Vec<CategoryTotal>,
    pub top_merchants: Vec<MerchantTotal>,
}
```

#### Changes to `get_report_data`

Add `scope: String` parameter (default `"year"` if empty/invalid). Scope controls:

| scope | `monthly` coverage | KPI date range |
|---|---|---|
| `"month"` | current month only (1 entry) | current month |
| `"quarter"` | last 3 months | last 3 months |
| `"year"` | YTD months (Jan → current) | YTD |
| `"all"` | all available months, capped at 24 | all time |

`monthly_last_year`: same month count as `monthly`, but each month string offset back by 12 months (e.g., if `monthly` covers Jun 2025–May 2026, `monthly_last_year` covers Jun 2024–May 2025).

`top_categories` and `top_merchants`: filter by the scope's date range (not always 12 months).

### Frontend — `ui/src/screens/Reports.tsx`

#### §10a Scope toolbar

Replace the existing `6M / 12M` toolbar with:
```
Month | Quarter | Year | All-time
```
State: `const [scope, setScope] = useState<"month"|"quarter"|"year"|"all">("year")`.  
Query key changes to `["report-data", scope]`. Pass `scope` to `getReportData(scope)`.

#### §10b Donut chart — `DonutChart` component (inline in Reports.tsx)

```typescript
function DonutChart({ categories, totalCents }: {
  categories: CategoryTotal[];
  totalCents: number;
})
```

SVG ring (cx=50, cy=50, inner r=32, outer r=48, viewBox="0 0 100 100"):
- Use all entries from `topCategories` (already top 10 from backend)
- Compute cumulative angles from each category's share of sum-of-returned-totals
- For each slice: SVG `path` with `M`, `L`, `A` (large-arc flag when share > 50%), back to center, `Z`
- Gap between slices: 1° of arc
- Center: `<text>` showing formatted total spend for scope

Legend below SVG: flex-wrap row of `color swatch + label + pct%` chips.

#### §10c YoY chart — `YoYChart` component (inline in Reports.tsx)

```typescript
function YoYChart({ thisYear, lastYear }: {
  thisYear: MonthSummary[];
  lastYear: MonthSummary[];
})
```

SVG polylines, `viewBox="0 0 100 50"`, `preserveAspectRatio="none"`:
- Both arrays share the same X axis (index-based)
- Y axis: scale to max of both datasets
- This year: solid `var(--accent)` line, weight 1.5
- Last year: dashed `var(--ink-mute)` line, weight 1 (`strokeDasharray="2 1"`)
- Month labels: X axis below, showing `thisYear[i].label`
- Legend: two color samples + "This year" / "Last year" labels

#### Layout update

Below existing bar+net-line 2-col grid, add a new 2-col grid row:
- Left: `<DonutChart>` (top categories for scope)
- Right: `<YoYChart>` (expense line, `thisYear = data.monthly`, `lastYear = data.monthlyLastYear`)

---

## 3. Budget: 5-Month History Strip (§7c)

### Backend — `crates/finsight-app/src/commands/budget.rs`

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyActual {
    pub month: String,  // "YYYY-MM"
    pub label: String,  // "Jan"
    pub cents: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryHistory {
    pub category_id: String,
    pub label: String,
    pub color: String,
    pub monthly: Vec<MonthlyActual>,  // N entries, oldest first, zeros for empty months
}

pub async fn list_budget_history(
    state: tauri::State<'_, AppState>,
    months: u32,
) -> AppResult<Vec<CategoryHistory>>
```

Implementation:
1. Rust generates the list of `months` month strings (`YYYY-MM`), oldest first.
2. SQL query: `SELECT category_id, strftime('%Y-%m', posted_at) as mo, SUM(-amount_cents) FROM transactions WHERE amount_cents < 0 AND strftime('%Y-%m', posted_at) >= ?cutoff GROUP BY category_id, mo`
3. Fetch all non-archived categories.
4. Assemble result: for each category that has at least one non-zero month in the period, create a `CategoryHistory` with `monthly` filled from the query results (zeros for missing months).
5. Sort by total spend across the period descending.

### Frontend — `ui/src/screens/Budget.tsx`

New hook `useBudgetHistory(months: number)` in `ui/src/api/hooks/budget.ts`.

Add a "Past 5 months" section at the bottom of `Budget.tsx`, below the envelope grid. Only render when `history.length > 0`.

**Table structure:**
```
Category    | Jan  | Feb  | Mar  | Apr  | May
────────────────────────────────────────────
● Groceries | $820 | $740 | $890 | $680 | $810
● Dining    | $340 | $290 |  —   | $410 | $380
```

- Amounts: compact format — `$1.2k` for ≥$1000, `$820` otherwise
- Zero/null months: display `—` in `var(--ink-faint)` color
- Cell background heatmap: `background: var(--accent)` at `opacity = (cents / columnMax) * 0.18`
- Column max per month: `Math.max(...history.map(c => c.monthly[i].cents))`
- Table uses `.tbl` class with an extra sticky first column

---

## 4. Rules: Agent Activity Log (§11c)

### Backend — `crates/finsight-app/src/commands/agent.rs`

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivity {
    pub text: String,       // "'{merchant}' → {category}"
    pub sub: String,        // "rule · 100% conf" or "llm · 82% conf"
    pub minutes_ago: i64,
}

pub async fn list_recent_agent_activity(
    state: tauri::State<'_, AppState>,
    limit: u32,
) -> AppResult<Vec<AgentActivity>>
```

SQL:
```sql
SELECT t.merchant_raw,
       COALESCE(c.label, 'Uncategorized'),
       cat.source,
       ROUND(cat.confidence * 100) as pct,
       CAST((julianday('now') - julianday(cat.at)) * 1440 AS INTEGER) as mins_ago
FROM categorizations cat
JOIN transactions t ON t.id = cat.txn_id
LEFT JOIN categories c ON c.id = cat.category_id
WHERE cat.at >= datetime('now', '-24 hours')
ORDER BY cat.at DESC
LIMIT ?1
```

Format `text` as `"'{merchant_raw}' → {category_label}"`. Format `sub` as `"{source} · {pct}% conf"`.

### Frontend — `ui/src/screens/Rules.tsx`

New hook `useRecentAgentActivity(limit: number)` in `ui/src/api/hooks/insights.ts`.

Add a third card to the right sidebar column (after "Trust dial"):

```tsx
<div className="card tight">
  <div className="eyebrow" style={{ marginBottom: 10 }}>
    <span className="dot" style={{ background: "var(--accent)" }} />
    Agent · last 24h
  </div>
  {activity.length === 0
    ? <p className="muted" style={{ fontSize: 13 }}>Nothing yet — import transactions to see activity.</p>
    : activity.map((a, i) => (
        <div key={i} style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 8,
          padding: "8px 0", borderBottom: i < activity.length - 1 ? "1px solid var(--line)" : "none" }}>
          <div>
            <div style={{ fontSize: 13 }}>{a.text}</div>
            <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{a.sub}</div>
          </div>
          <span style={{ fontSize: 11.5, color: "var(--ink-faint)", fontFamily: "var(--mono)",
            alignSelf: "center", whiteSpace: "nowrap" }}>
            {a.minutesAgo < 60 ? `${a.minutesAgo}m` : `${Math.floor(a.minutesAgo / 60)}h`}
          </span>
        </div>
      ))
  }
</div>
```

---

## 5. Transactions: CSV Export (§5c)

### Backend — `crates/finsight-app/src/commands/transactions.rs`

```rust
pub async fn export_transactions_csv(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    filter: TxnFilterInput,
) -> AppResult<String>
```

1. Apply same filter as `list_transactions` (reuse the WHERE clause building logic) with no limit/offset.
2. Show save dialog via `tauri_plugin_dialog::FileDialogBuilder::new().add_filter("CSV", &["csv"]).save_file()`.
3. If cancelled, return `Ok(String::new())`.
4. Write CSV rows: `date,merchant,category,amount,notes` header + one row per transaction. `amount` = `amount_cents as f64 / 100.0` (negative = expense).
5. Return the file path string.

### Frontend — `ui/src/screens/Transactions.tsx`

Add "Export CSV" button to the screen header (right side, after the tab strip row):

```tsx
<button className="btn ghost sm" onClick={async () => {
  const result = await commands.exportTransactionsCsv(filter);
  if (result.status === "ok" && result.data) {
    toast.success("Exported", { description: result.data });
  }
}}>
  ↓ Export CSV
</button>
```

---

## 6. Accounts: CSV Export (§4c)

### Backend — `crates/finsight-app/src/commands/accounts.rs`

```rust
pub async fn export_account_csv(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    account_id: String,
) -> AppResult<String>
```

Same pattern as §5c: query transactions for the account ordered by `posted_at DESC`, save dialog, write CSV, return path.

### Frontend — `ui/src/screens/Accounts.tsx`

Add a small "CSV ↓" ghost button to each account row's action area. On click: calls `exportAccountCsv(account.id)`. Toast on success.

---

## 7. Recurring: Price-History Chip (§8b)

### Backend — `crates/finsight-app/src/commands/recurring.rs`

Add two fields to `RecurringItem`:
```rust
pub min_amount_cents: i64,  // most negative historical value (highest price for expenses)
pub max_amount_cents: i64,  // least negative historical value (lowest price for expenses)
```

In the SQL CTE, add `MIN(t.amount_cents)` and `MAX(t.amount_cents)` to the per-merchant aggregate alongside the existing `last_amount_cents` computation.

### Frontend — `ui/src/screens/Recurring.tsx`

In the subscriptions list, after the amount display for each item:

```typescript
const minAbs = Math.abs(item.minAmountCents);  // highest historical price
const maxAbs = Math.abs(item.maxAmountCents);  // lowest historical price
const curAbs = Math.abs(item.lastAmountCents); // current price

const priceChanged = minAbs !== maxAbs;
const priceUp   = priceChanged && curAbs >= minAbs;  // at or near the high
const priceDown = priceChanged && curAbs <= maxAbs;  // at or near the low
```

Render chip:
- Price up: `<span className="chip warning">↑ {fmt(maxAbs)} → {fmt(curAbs)}</span>`
- Price down: `<span className="chip positive">↓ {fmt(minAbs)} → {fmt(curAbs)}</span>`
- No change: render nothing

---

## 8. Settings: Keyboard Shortcuts (§12d)

### Frontend — `ui/src/screens/Settings.tsx`

Add a "Keyboard shortcuts" section at the bottom of the Settings screen, after the appearance section:

```tsx
<div className="section">
  <div className="eyebrow" style={{ marginBottom: 14 }}>Keyboard shortcuts</div>
  <div className="card tight">
    {[
      { key: "⌘K", label: "Open command palette" },
      { key: "⌘.", label: "Toggle privacy mode" },
    ].map(({ key, label }, i, arr) => (
      <div key={key} style={{
        display: "flex", alignItems: "center", gap: 16, padding: "10px 0",
        borderBottom: i < arr.length - 1 ? "1px solid var(--line)" : "none",
      }}>
        <kbd style={{
          fontFamily: "var(--mono)", fontSize: 13, padding: "3px 8px",
          background: "var(--surface-2)", border: "1px solid var(--line)",
          borderRadius: 5, color: "var(--ink)", minWidth: 36, textAlign: "center",
        }}>
          {key}
        </kbd>
        <span style={{ fontSize: 14 }}>{label}</span>
      </div>
    ))}
  </div>
</div>
```

---

## Migration

No new migrations required. All backend changes add new commands or extend existing ones. No schema changes.

---

## New bindings required

After all Rust changes, run `cargo run -p finsight-tauri --bin export_bindings` from repo root. New exports:

- `commands.getPlanNextMonthData()`
- `commands.applyNextMonthPlan(assignments)`
- `commands.getReportData(scope)` ← signature change (was no-arg)
- `commands.listBudgetHistory(months)`
- `commands.listRecentAgentActivity(limit)`
- `commands.exportTransactionsCsv(filter)`
- `commands.exportAccountCsv(accountId)`
- Updated `RecurringItem` type (adds `minAmountCents`, `maxAmountCents`)
- Updated `ReportData` type (adds `monthlyLastYear`)
- Updated `CategoryPlanRow`, `PlanData`, `PlanAssignment`, `CategoryHistory`, `MonthlyActual`, `AgentActivity` types

---

## Test bar

All existing tests must remain green: 103 Rust tests, 90 frontend tests, 0 TypeScript errors.

New frontend tests to add (one file per complex screen change):
- `PlanNextMonthModal.test.tsx` — step navigation, suggestion computation, apply mutation
- `Reports.test.tsx` (extend) — scope switcher state, donut/YoY render with mock data
- `Budget.history.test.tsx` — history table renders cells, heatmap opacity logic

No new Rust unit tests required (commands are thin wrappers over existing repo functions).

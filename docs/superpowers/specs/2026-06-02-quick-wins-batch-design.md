# Quick-Wins Batch — Design Spec

**Date:** 2026-06-02  
**Items:** §5a, §5b, §6a, §6b, §7a, §7b, §8a, §9a, §15b, §15c from TODO.md  
**Effort:** Low — no new migrations, no new screen files  

---

## Scope

Six groups of enhancements to existing screens, all additive with no breaking changes to existing behavior.

---

## 1. Transaction search + filter tabs (§5a, §5b)

### Backend

**File:** `crates/finsight-app/src/commands/transactions.rs`

Extend `TxnFilterInput`:
```rust
pub struct TxnFilterInput {
    pub account_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub search: Option<String>,          // NEW
    pub filter_preset: Option<String>,   // NEW: "needs_review" | "anomalies" | "no_category"
}
```

**File:** `crates/finsight-core/src/repos/transactions.rs`

Extend `TxnFilter`:
```rust
pub struct TxnFilter {
    pub account_id: Option<String>,
    pub limit: i64,
    pub offset: i64,
    pub search: Option<String>,         // NEW
    pub filter_preset: Option<String>,  // NEW
}
```

Update `list()` to dynamically build WHERE clauses:
- `search`: `AND (lower(t.merchant_raw) LIKE lower('%'||?||'%') OR lower(t.notes) LIKE lower('%'||?||'%'))`
- `filter_preset = "needs_review"`: `AND t.ai_confidence IS NOT NULL AND t.ai_confidence < 0.6`
- `filter_preset = "anomalies"`: `AND t.is_anomaly = 1`
- `filter_preset = "no_category"`: `AND t.category_id IS NULL`

Multiple filters compose with AND. Regenerate bindings after this change.

### Frontend

**File:** `ui/src/screens/Transactions.tsx`

- Add `search` state (`string`, debounced 300ms via `useEffect` + `setTimeout`)
- Add `preset` state (`"" | "needs_review" | "anomalies" | "no_category"`, default `""`)
- Compose into `TxnFilterInput` passed to `useTransactions(filter)`
- UI: full-width search `<input>` above the table with placeholder "Search transactions…"
- Tab strip below header: `All · Needs review · Anomalies · No category` — active tab gets `.active` class, updates `preset` state

---

## 2. Categories: Year scope + Budget column (§6a, §6b)

### Backend

**File:** `crates/finsight-app/src/commands/transactions.rs` (`list_categories_with_spending`)

Add two new fields to `CategoryWithSpending`:
```rust
pub year_total_cents: i64,   // SUM of outflow this calendar year
pub budget_cents: i64,       // budget for current month (0 if none set)
```

SQL changes:
- `year_total_cents`: `COALESCE(SUM(CASE WHEN t.posted_at >= strftime('%Y-01-01','now') AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0)`
- `budget_cents`: LEFT JOIN `budgets b ON b.category_id = c.id AND b.month = strftime('%Y-%m','now')`, select `COALESCE(b.amount_cents, 0)`

Regenerate bindings after this change.

### Frontend

**File:** `ui/src/screens/Categories.tsx`

- Add `"Year"` option to the existing scope toolbar (3 options: Month / Last / Year)
- When scope = `"year"`, use `cat.yearTotalCents` as the displayed value
- Add a "Budget" column to the category table:
  - Shows `cat.budgetCents` formatted as money (or `—` if 0)
  - Text color: `var(--negative)` if `cat.thisMonthCents > cat.budgetCents && cat.budgetCents > 0`

---

## 3. Budget: "To Budget" tracker + "By activity" sort (§7a, §7b)

### Frontend only

**File:** `ui/src/screens/Budget.tsx`

**"To Budget" tracker:**
- Compute `toBudget = totals.incomeCents - envelopes.reduce((s, e) => s + e.budgetCents, 0)`
- Render a pill row immediately below the header:
  ```
  [lime dot] To Budget · unassigned
  $X,XXX  of $Y,YYY income · $Z,ZZZ assigned
  ```
- Use existing CSS `.chip` / `.stat` classes from app.css

**"By activity" sort:**
- Add `"activity"` as a 4th sort option to the existing sort toolbar
- Sort logic: `[...envelopes].sort((a, b) => b.txnCount - a.txnCount)`
- The `txnCount` field already exists on `BudgetEnvelope` from the backend

---

## 4. Recurring: Day-detail panel (§8a)

### Frontend only

**File:** `ui/src/screens/Recurring.tsx` (inside the `CalendarView` sub-component)

- Add `selectedDay: number | null` state, default `null`
- Calendar cell `onClick` sets `selectedDay` to that day number (toggle: click same day again deselects)
- When `selectedDay !== null`, render a `.rcal-detail` div below the calendar grid containing:
  - Left: large day number (`fontSize: 40`), weekday name, "TODAY" badge if `selectedDay === today`
  - Right: net total card (sum of amounts for the day, color-coded positive/negative)
  - List of recurring items for that day from `dayMap[selectedDay]`: merchant logo tile, name, status chip, amount, `···` menu button
- Add `.rcal-detail` CSS to `app.css` (it does not exist yet): a panel below the calendar grid with padding, border-top `1px solid var(--line)`, background `var(--elevated)`, border-radius `0 0 12px 12px`

---

## 5. Goals: Pace chip (§9a)

### Frontend only

**File:** `ui/src/screens/Goals.tsx`

Per goal card, compute pace status:
```typescript
const monthsRemaining = Math.ceil((goal.targetCents - goal.currentCents) / Math.max(goal.monthlyCents, 1));
const monthsExpected = differenceInMonths(parseISO(goal.targetDate), new Date());
let pace: "ahead" | "on_track" | "needs_attention";
if (monthsRemaining < monthsExpected * 0.85) pace = "ahead";
else if (monthsRemaining > monthsExpected * 1.15 || goal.monthlyCents === 0) pace = "needs_attention";
else pace = "on_track";
```

Render as a `.chip` in the goal card header:
- `"ahead"` → `className="chip positive"`, label "Ahead"
- `"on_track"` → `className="chip"`, label "On track"
- `"needs_attention"` → `className="chip warning"`, label "Needs attention"

Handle edge case: if `targetDate` is null/past or `targetCents` is 0, skip the chip.

---

## 6. Sidebar additions (§15b, §15c)

### Backend

**File:** `crates/finsight-app/src/commands/meta.rs` (or a new `sidebar.rs`)

New command:
```rust
#[tauri::command]
#[specta::specta]
pub async fn get_transaction_count(state: tauri::State<'_, AppState>) -> AppResult<i64>
```

SQL: `SELECT COUNT(*) FROM transactions`

Register in `lib.rs` and regenerate bindings.

### Frontend

**File:** `ui/src/components/Sidebar.tsx`

**Transaction count badge (§15b):**
- Add `useQuery` for `get_transaction_count` with `staleTime: 60_000` (refetch every 60s)
- Format: `count >= 1000 ? (count / 1000).toFixed(1) + 'k' : count.toString()`
- Render as `<span className="badge">` next to the "Transactions" nav item

**"Run setup again" footer (§15c):**
- Add mutation calling `resetOnboardingCompletion()` 
- On success, `navigate("/onboarding")`
- Render in sidebar footer:
  ```tsx
  <div className="nav-item" onClick={handleReset}>
    <I.Sparkle className="ico" />
    <span>Run setup again</span>
  </div>
  ```

---

## Implementation order

1. Backend changes (transactions filter + categories with spending + transaction count) — regenerate bindings once after all backend changes
2. Frontend: Transactions.tsx (search + filter tabs)
3. Frontend: Categories.tsx (year scope + budget column)
4. Frontend: Budget.tsx (To Budget tracker + activity sort)
5. Frontend: Recurring.tsx (day-detail panel)
6. Frontend: Goals.tsx (pace chip)
7. Frontend: Sidebar.tsx (count badge + run setup again)

## Testing

- Run `npx vitest run` from `ui/` — 51 existing tests must stay green
- Run `cargo test --workspace` — all Rust tests must stay green
- Run `cargo run -p finsight-tauri --bin export_bindings` from repo root after all backend changes

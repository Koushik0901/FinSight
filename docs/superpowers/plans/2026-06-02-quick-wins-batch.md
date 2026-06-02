# Quick-Wins Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add transaction search + filter tabs, categories year scope + budget column, budget "To Budget" tracker + activity sort, recurring day-detail panel, goals pace chip, and sidebar transaction count badge + "Run setup again" footer.

**Architecture:** All changes are additive enhancements to existing screens. Backend changes extend two existing commands (`list_transactions`, `list_categories_with_spending`) and add one new command (`get_transaction_count`). No new migrations or screen files needed.

**Tech Stack:** Rust/Tauri 2 backend · React 18 + TypeScript · tanstack-query · SQLite via rusqlite · CSS variables from `ui/src/styles/tokens.css`

---

## File Map

**Modify (backend):**
- `crates/finsight-core/src/repos/transactions.rs` — extend `TxnFilter`, update `list()` SQL
- `crates/finsight-app/src/commands/transactions.rs` — extend `TxnFilterInput`, extend `CategoryWithSpending`, update `list_categories_with_spending` SQL, add `get_transaction_count`
- `crates/finsight-app/src/lib.rs` — register `get_transaction_count`

**Modify (frontend):**
- `ui/src/screens/Transactions.tsx` — search input + filter tabs
- `ui/src/screens/Categories.tsx` — "Year" scope option + Budget column
- `ui/src/screens/Budget.tsx` — "To Budget" pill + "By activity" sort
- `ui/src/styles/app.css` — add `.rcal-detail` CSS
- `ui/src/screens/Recurring.tsx` — day-detail panel in `CalendarView`
- `ui/src/screens/Goals.tsx` — pace chip per goal card
- `ui/src/components/Sidebar.tsx` — transaction count badge + "Run setup again"

---

## Task 1: Extend TxnFilter in the core repo

**Files:**
- Modify: `crates/finsight-core/src/repos/transactions.rs`

- [ ] **Step 1: Add `search` and `filter_preset` fields to `TxnFilter`**

Replace the existing `TxnFilter` struct and its `Default` impl (lines 49–63):

```rust
pub struct TxnFilter {
    pub account_id: Option<String>,
    pub limit: i64,
    pub offset: i64,
    pub search: Option<String>,
    pub filter_preset: Option<String>,
}

impl Default for TxnFilter {
    fn default() -> Self {
        Self {
            account_id: None,
            limit: 100,
            offset: 0,
            search: None,
            filter_preset: None,
        }
    }
}
```

- [ ] **Step 2: Update `list()` to use the new filter fields**

Replace the WHERE-clause block in `list()` — the section that currently reads:
```rust
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(aid) = filter.account_id.as_ref() {
        sql.push_str("WHERE t.account_id = ? ");
        params.push(Box::new(aid.clone()));
    }
    sql.push_str("ORDER BY t.posted_at DESC LIMIT ? OFFSET ?");
```

Replace with:
```rust
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();

    if let Some(aid) = filter.account_id.as_ref() {
        conditions.push("t.account_id = ?".to_string());
        params.push(Box::new(aid.clone()));
    }
    if let Some(search) = filter.search.as_ref() {
        conditions.push(
            "(lower(t.merchant_raw) LIKE lower(?) OR lower(COALESCE(t.notes,'')) LIKE lower(?))".to_string(),
        );
        let pattern = format!("%{}%", search);
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }
    match filter.filter_preset.as_deref() {
        Some("needs_review") => {
            conditions.push("t.ai_confidence IS NOT NULL AND t.ai_confidence < 0.6".to_string());
        }
        Some("anomalies") => {
            conditions.push("t.is_anomaly = 1".to_string());
        }
        Some("no_category") => {
            conditions.push("t.category_id IS NULL".to_string());
        }
        _ => {}
    }
    if !conditions.is_empty() {
        sql.push_str("WHERE ");
        sql.push_str(&conditions.join(" AND "));
        sql.push(' ');
    }
    sql.push_str("ORDER BY t.posted_at DESC LIMIT ? OFFSET ?");
    params.push(Box::new(filter.limit));
    params.push(Box::new(filter.offset));
```

- [ ] **Step 3: Run Rust tests to confirm existing tests still pass**

```
cargo test -p finsight-core --lib repos::transactions
```

Expected: all 4 transaction tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/repos/transactions.rs
git commit -m "feat(core): extend TxnFilter with search and filter_preset"
```

---

## Task 2: Extend TxnFilterInput in the commands layer

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs`

- [ ] **Step 1: Add fields to `TxnFilterInput`**

Find the `TxnFilterInput` struct definition (currently has `account_id`, `limit`, `offset`) and add two new fields:

```rust
#[derive(Debug, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct TxnFilterInput {
    pub account_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub search: Option<String>,
    pub filter_preset: Option<String>,
}
```

- [ ] **Step 2: Pass new fields through in `list_transactions`**

Find the `transactions::TxnFilter { ... }` construction inside `list_transactions` and add the two new fields:

```rust
transactions::TxnFilter {
    account_id: filter.account_id,
    limit: filter.limit.unwrap_or(100),
    offset: filter.offset.unwrap_or(0),
    search: filter.search,
    filter_preset: filter.filter_preset,
}
```

- [ ] **Step 3: Run Rust tests**

```
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs
git commit -m "feat(app): extend TxnFilterInput with search and filter_preset"
```

---

## Task 3: Extend CategoryWithSpending with year + budget fields

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs`

- [ ] **Step 1: Add `year_total_cents` and `budget_cents` to `CategoryWithSpending`**

Find the `CategoryWithSpending` struct and add two fields at the end:

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryWithSpending {
    pub id: String,
    pub label: String,
    pub color: String,
    pub group_id: String,
    pub group_label: String,
    pub this_month_cents: i64,
    pub last_month_cents: i64,
    pub txn_count: i64,
    pub year_total_cents: i64,   // NEW
    pub budget_cents: i64,       // NEW
}
```

- [ ] **Step 2: Update `list_categories_with_spending` with new SQL and params**

Replace the entire `list_categories_with_spending` command body. The new version adds `year_start` and `current_month` params and a LEFT JOIN on `budgets`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_categories_with_spending(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryWithSpending>> {
    let db = (*state.db).clone();
    let now = Utc::now();
    let this_month_start = now.format("%Y-%m-01").to_string();
    let last_month_start = {
        let m = now.month0();
        if m == 0 {
            format!("{}-12-01", now.year() - 1)
        } else {
            format!("{}-{:02}-01", now.year(), m)
        }
    };
    let year_start = format!("{}-01-01", now.year());
    let current_month = now.format("%Y-%m").to_string();

    run(&db, move |conn| {
        let mut stmt = conn.prepare(
            "SELECT \
               c.id, c.label, COALESCE(c.color,''), c.group_id, COALESCE(g.label,''), \
               COALESCE(SUM(CASE WHEN t.posted_at >= ?1 AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0), \
               COALESCE(SUM(CASE WHEN t.posted_at >= ?2 AND t.posted_at < ?1 AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0), \
               COUNT(CASE WHEN t.posted_at >= ?1 THEN 1 END), \
               COALESCE(SUM(CASE WHEN t.posted_at >= ?3 AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0), \
               COALESCE(MAX(b.amount_cents), 0) \
             FROM categories c \
             LEFT JOIN category_groups g ON g.id = c.group_id \
             LEFT JOIN transactions t ON t.category_id = c.id \
             LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?4 \
             WHERE c.archived_at IS NULL \
             GROUP BY c.id, c.label, c.color, c.group_id, g.label \
             ORDER BY 6 DESC, g.sort_order, c.sort_order",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![this_month_start, last_month_start, year_start, current_month],
            |r| {
                Ok(CategoryWithSpending {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    color: r.get(2)?,
                    group_id: r.get(3)?,
                    group_label: r.get(4)?,
                    this_month_cents: r.get(5)?,
                    last_month_cents: r.get(6)?,
                    txn_count: r.get(7)?,
                    year_total_cents: r.get(8)?,
                    budget_cents: r.get(9)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
```

Note: `now.month0()` and `now.year()` require the `Datelike` trait — it's already imported at the top of the file via `use chrono::{Datelike, Utc};`.

- [ ] **Step 3: Run Rust tests**

```
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs
git commit -m "feat(app): add yearTotalCents and budgetCents to CategoryWithSpending"
```

---

## Task 4: Add `get_transaction_count` command

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs`
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Add the command to `transactions.rs`**

Append at the end of `crates/finsight-app/src/commands/transactions.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn get_transaction_count(
    state: tauri::State<'_, AppState>,
) -> AppResult<i64> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
    })
    .await
    .map_err(AppError::from)
}
```

- [ ] **Step 2: Register the command in `lib.rs`**

In `crates/finsight-app/src/lib.rs`, inside `build_specta_builder()`, add after `commands::reports::get_month_totals`:

```rust
        commands::transactions::get_transaction_count,
```

- [ ] **Step 3: Run Rust tests**

```
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs crates/finsight-app/src/lib.rs
git commit -m "feat(app): add get_transaction_count command"
```

---

## Task 5: Regenerate TypeScript bindings

**Files:**
- Auto-generated: `ui/src/api/bindings.ts`

- [ ] **Step 1: Run the bindings export from the repo root**

From the root of the repository (not from `ui/`):

```
cargo run -p finsight-tauri --bin export_bindings
```

Expected output ends with something like: `Bindings written to ui/src/api/bindings.ts`

- [ ] **Step 2: Verify bindings contain the new fields**

```
grep -n "search\|filterPreset\|yearTotalCents\|budgetCents\|getTransactionCount" ui/src/api/bindings.ts
```

Expected: lines containing each of those identifiers.

- [ ] **Step 3: Run frontend type-check to catch any breaking changes**

```
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/api/bindings.ts
git commit -m "chore: regenerate bindings — search, filterPreset, yearTotalCents, budgetCents, getTransactionCount"
```

---

## Task 6: Transaction search + filter tabs

**Files:**
- Modify: `ui/src/screens/Transactions.tsx`

- [ ] **Step 1: Replace `Transactions.tsx` with the version that has search + tabs**

The existing file uses `const { data, isLoading, error } = useTransactions();` with no filter. Replace the entire file with:

```tsx
import { useState, useEffect, useRef } from "react";
import { useTransactions } from "../api/hooks/transactions";
import TransactionDrawer from "../components/TransactionDrawer";
import FilePicker from "../components/FilePicker";
import ImportMappingDialog from "./onboarding/ImportMappingDialog";
import type { Transaction, TxnFilterInput } from "../api/client";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

type Preset = "" | "needs_review" | "anomalies" | "no_category";

const TABS: { key: Preset; label: string }[] = [
  { key: "", label: "All" },
  { key: "needs_review", label: "Needs review" },
  { key: "anomalies", label: "Anomalies" },
  { key: "no_category", label: "No category" },
];

export default function Transactions() {
  const [addOpen, setAddOpen] = useState(false);
  const [editTxn, setEditTxn] = useState<Transaction | null>(null);
  const [csvPath, setCsvPath] = useState<string | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [preset, setPreset] = useState<Preset>("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => setSearch(searchInput), 300);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [searchInput]);

  const filter: TxnFilterInput = {
    accountId: null,
    limit: null,
    offset: null,
    search: search || null,
    filterPreset: preset || null,
  };

  const { data, isLoading, error } = useTransactions(filter);

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;

  return (
    <div className="screen-transactions">
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <h1 style={{ fontSize: 32, fontWeight: 600, margin: 0 }}>Transactions</h1>
        <div className="actions" style={{ display: "flex", gap: 8 }}>
          <FilePicker onPicked={setCsvPath} label="Import CSV" />
          <button className="primary" onClick={() => setAddOpen(true)}>+ Add transaction</button>
        </div>
      </header>

      {/* Search */}
      <input
        type="search"
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        placeholder="Search transactions…"
        style={{
          width: "100%",
          background: "var(--surface-2)",
          border: "1px solid var(--line)",
          borderRadius: 8,
          padding: "8px 14px",
          fontSize: 14,
          color: "var(--ink)",
          outline: "none",
          marginBottom: 12,
          boxSizing: "border-box",
        }}
      />

      {/* Filter tabs */}
      <div className="toolbar" style={{ marginBottom: 20, display: "inline-flex" }}>
        {TABS.map((tab) => (
          <button
            key={tab.key}
            className={preset === tab.key ? "on" : ""}
            onClick={() => setPreset(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {(!data || data.length === 0) ? (
        <div className="stub">No transactions match your filters.</div>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Date</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Merchant</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Category</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Amount</th>
            </tr>
          </thead>
          <tbody>
            {data.map((t: Transaction) => (
              <tr
                key={t.id}
                style={{ borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                onClick={() => setEditTxn(t)}
                aria-label={`Edit transaction ${t.merchant_raw}`}
              >
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{formatDate(t.posted_at)}</td>
                <td style={{ padding: "12px 0" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    <span
                      aria-label={`${t.merchant_label ?? t.merchant_raw} merchant tile`}
                      style={{
                        width: 28, height: 28, borderRadius: 6,
                        background: t.merchant_color ?? "var(--surface-2)",
                        color: "var(--accent-ink)",
                        fontSize: 11, fontWeight: 600,
                        display: "grid", placeItems: "center",
                      }}
                    >
                      {t.merchant_initials ?? "?"}
                    </span>
                    <span>{t.merchant_label ?? t.merchant_raw}</span>
                  </div>
                </td>
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>
                  {t.category_label ?? "Uncategorized"}
                </td>
                <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                  <span className="money">{formatMoney(t.amount_cents)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <TransactionDrawer
        open={editTxn !== null}
        onClose={() => setEditTxn(null)}
        transaction={editTxn ?? undefined}
      />
      {csvPath && (
        <ImportMappingDialog
          path={csvPath}
          onClose={() => setCsvPath(null)}
          onImported={() => setCsvPath(null)}
        />
      )}
    </div>
  );
}
```

Note: The `Transaction` type in `bindings.ts` uses **snake_case** field names (not camelCase) because the Rust struct doesn't use `rename_all = "camelCase"`. Use `t.merchant_raw`, `t.posted_at`, `t.amount_cents`, `t.merchant_label`, `t.merchant_color`, `t.merchant_initials`, `t.category_label` — exactly as in the original file.

- [ ] **Step 2: Run frontend tests**

```
cd ui && npx vitest run
```

Expected: 51 tests pass (or more if new ones were added). Zero failures.

- [ ] **Step 3: Run TypeScript type-check**

```
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/Transactions.tsx
git commit -m "feat(ui): transaction search bar + filter tabs (needs review / anomalies / no category)"
```

---

## Task 7: Categories — year scope + budget column

**Files:**
- Modify: `ui/src/screens/Categories.tsx`

- [ ] **Step 1: Add `"year"` scope type and update the scope toolbar**

Change the `scope` state type from `"month" | "avg"` to `"month" | "avg" | "year"`:

```tsx
const [scope, setScope] = useState<"month" | "avg" | "year">("month");
```

Add a `"Year"` button to the existing toolbar section (the `<div className="toolbar">` that currently has two buttons):

```tsx
<div className="toolbar">
  <button className={scope === "month" ? "on" : ""} onClick={() => setScope("month")}>
    This month
  </button>
  <button className={scope === "avg" ? "on" : ""} onClick={() => setScope("avg")}>
    vs. last month
  </button>
  <button className={scope === "year" ? "on" : ""} onClick={() => setScope("year")}>
    Year to date
  </button>
</div>
```

- [ ] **Step 2: Update `valueFor` to handle year scope**

The existing `valueFor` function handles `"month"` and `"avg"`. Extend it:

```tsx
const valueFor = (c: CategoryWithSpending) => {
  if (scope === "avg") return Math.round((c.thisMonthCents + c.lastMonthCents) / 2);
  if (scope === "year") return c.yearTotalCents;
  return c.thisMonthCents;
};
```

`yearTotalCents` is the new field added to `CategoryWithSpending` in Task 3.

- [ ] **Step 3: Add a "Budget" column header and cell to the table**

In the `<thead>` of the category table, add a new `<th>` after the "Transactions" column:

```tsx
<th className="right">Budget</th>
```

In each `<tr>` in `<tbody>`, add a new `<td>` after the transactions count cell:

```tsx
<td className="right num tabular" style={{ color: c.budgetCents > 0 && c.thisMonthCents > c.budgetCents ? "var(--negative)" : "var(--ink-mute)" }}>
  {c.budgetCents > 0 ? fmt(c.budgetCents) : "—"}
</td>
```

`budgetCents` is the new field from Task 3.

- [ ] **Step 4: Run TypeScript type-check**

```
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Categories.tsx
git commit -m "feat(ui): categories year-to-date scope + budget column in table"
```

---

## Task 8: Budget — "To Budget" pill + "By activity" sort

**Files:**
- Modify: `ui/src/screens/Budget.tsx`

- [ ] **Step 1: Add month totals query to `Budget.tsx`**

Add the import for `commands` and `MonthTotals` at the top (alongside the existing imports), and add `useQuery` from `@tanstack/react-query`:

```tsx
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { useBudgetEnvelopes, useSetBudget } from "../api/hooks/budget";
import { commands, type BudgetEnvelope, type MonthTotals } from "../api/client";
import * as I from "../components/Icons";
```

Inside the `Budget` component, add a query for month totals right after `useBudgetEnvelopes()`:

```tsx
const { data: totals } = useQuery<MonthTotals>({
  queryKey: ["today-summary"],
  queryFn: async () => {
    const result = await commands.getMonthTotals();
    if (result.status === "error") throw new Error(result.error.message);
    return result.data;
  },
  staleTime: 60_000,
});
```

- [ ] **Step 2: Compute `toBudget` and update the `SortKey` type**

Add the `toBudget` computation after the `sorted` declaration:

```tsx
const totalBudgetSet = envelopes.reduce((s, e) => s + e.budgetCents, 0);
const toBudget = (totals?.incomeCents ?? 0) - totalBudgetSet;
```

Change the `SortKey` type to include `"activity"`:

```tsx
type SortKey = "group" | "stress" | "size" | "activity";
```

- [ ] **Step 3: Add "By activity" sort logic**

In the `sorted` computation, add the new sort case:

```tsx
const sorted = [...envelopes].sort((a, b) => {
  if (sort === "stress")   return envelopeStatus(b).severity - envelopeStatus(a).severity || b.spentCents - a.spentCents;
  if (sort === "size")     return b.budgetCents - a.budgetCents;
  if (sort === "activity") return b.txnCount - a.txnCount;
  return (a.groupLabel || "").localeCompare(b.groupLabel || "") || a.categoryLabel.localeCompare(b.categoryLabel);
});
```

- [ ] **Step 4: Add the "By activity" button to the toolbar**

In the `<div className="toolbar">` containing the sort buttons, add:

```tsx
<button className={sort === "activity" ? "on" : ""} onClick={() => setSort("activity")}>By activity</button>
```

- [ ] **Step 5: Add the "To Budget" pill row below the screen header**

In the JSX, directly after the `</div>` that closes the `screen-header` div and before the `{noData ? ...}` block, add:

```tsx
{/* To Budget tracker */}
{totals && (
  <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "10px 16px", background: "var(--surface-2)", borderRadius: 10, marginBottom: 20, fontSize: 13 }}>
    <span style={{ width: 8, height: 8, borderRadius: 999, background: "var(--accent)", boxShadow: "0 0 6px var(--accent)", flexShrink: 0 }} />
    <span style={{ color: "var(--ink-mute)" }}>To Budget · unassigned</span>
    <span className="num money" style={{ fontSize: 18, fontWeight: 600, color: toBudget >= 0 ? "var(--accent)" : "var(--negative)" }}>
      {fmt(Math.abs(toBudget))}
      {toBudget < 0 ? " over" : ""}
    </span>
    <span style={{ color: "var(--ink-faint)", marginLeft: "auto", fontSize: 12 }}>
      of {fmt(totals.incomeCents)} income · {fmt(totalBudgetSet)} assigned
    </span>
  </div>
)}
```

- [ ] **Step 6: Run TypeScript type-check + tests**

```
cd ui && npx tsc --noEmit && npx vitest run
```

Expected: 0 type errors, 51+ tests pass.

- [ ] **Step 7: Commit**

```bash
git add ui/src/screens/Budget.tsx
git commit -m "feat(ui): budget To Budget tracker pill + By activity sort option"
```

---

## Task 9: Recurring — day-detail panel CSS + implementation

**Files:**
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/screens/Recurring.tsx`

- [ ] **Step 1: Add `.rcal-detail` CSS to `app.css`**

Append to the end of `ui/src/styles/app.css`:

```css
/* ── Recurring calendar day-detail panel ───────────────────────────────── */
.rcal-detail {
  border-top: 1px solid var(--line);
  background: var(--elevated);
  border-radius: 0 0 12px 12px;
  padding: 20px 24px;
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 24px;
  animation: fadeIn .15s ease;
}
.rcal-detail-day {
  font-size: 48px;
  font-weight: 700;
  line-height: 1;
  color: var(--ink);
  font-family: var(--mono);
}
.rcal-detail-weekday {
  font-size: 13px;
  color: var(--ink-mute);
  margin-top: 4px;
}
.rcal-detail-today-badge {
  display: inline-block;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: .06em;
  padding: 2px 7px;
  border-radius: 999px;
  background: var(--accent);
  color: var(--accent-ink);
  margin-top: 6px;
}
.rcal-detail-items {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.rcal-detail-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  background: var(--surface-2);
  border-radius: 8px;
}
.rcal-detail-net {
  font-size: 13px;
  font-weight: 600;
  font-family: var(--mono);
  padding: 6px 14px;
  border-radius: 8px;
  background: var(--surface-2);
  margin-bottom: 10px;
  display: inline-block;
}
.rcal-detail-net.pos { color: var(--accent); }
.rcal-detail-net.neg { color: var(--negative); }
```

- [ ] **Step 2: Update `CalendarView` in `Recurring.tsx` to track selected day and render the panel**

Find the `CalendarView` function component. Add `selectedDay` state immediately after the existing `offset` state:

```tsx
const [selectedDay, setSelectedDay] = useState<number | null>(null);
```

Update each calendar day cell `<div>` to be clickable. Find the cell `<div key={day} className={[...].join(" ")} ...>` opening tag and add an `onClick` and `cursor: pointer` style:

```tsx
<div
  key={day}
  className={[
    "rcal-cell",
    isToday ? "today" : "",
    isPast  ? "past"  : "",
    isWeekend && !isToday ? "weekend" : "",
    netCents > 0 ? "pos" : "",
    selectedDay === day ? "selected" : "",
  ].filter(Boolean).join(" ")}
  style={{ "--load": `${loadPct}%`, cursor: dayItems.length > 0 ? "pointer" : undefined } as React.CSSProperties}
  onClick={() => setSelectedDay(selectedDay === day ? null : day)}
>
```

After the closing `</div>` of the `<div className="rcal-grid">`, add the day-detail panel:

```tsx
{selectedDay !== null && (dayMap[selectedDay] ?? []).length > 0 && (() => {
  const detailItems = dayMap[selectedDay] ?? [];
  const netCents = detailItems.reduce((s, r) => s + r.lastAmountCents, 0);
  const dayDate = new Date(year, month, selectedDay);
  const weekday = dayDate.toLocaleString("default", { weekday: "long" });
  return (
    <div className="rcal-detail">
      <div>
        <div className="rcal-detail-day">{selectedDay}</div>
        <div className="rcal-detail-weekday">{weekday}</div>
        {isCurrentMonth && selectedDay === today && (
          <div className="rcal-detail-today-badge">TODAY</div>
        )}
      </div>
      <div>
        <div className={`rcal-detail-net ${netCents > 0 ? "pos" : "neg"}`}>
          {fmt(netCents)} net
        </div>
        <div className="rcal-detail-items">
          {detailItems.map((item) => {
            const color = item.lastAmountCents > 0 ? "var(--accent)" : (item.categoryColor || colorFromStr(item.merchantRaw));
            return (
              <div key={item.merchantRaw} className="rcal-detail-item">
                <div style={{ width: 30, height: 30, borderRadius: 7, background: color, color: "#fff", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 700, flexShrink: 0 }}>
                  {initials(item.merchantRaw)}
                </div>
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 13.5, fontWeight: 500 }}>{item.merchantRaw}</div>
                  <div className="muted" style={{ fontSize: 12 }}>{item.categoryLabel || "Uncategorized"}</div>
                </div>
                <span className={`chip ${item.isSubscription ? "positive" : ""}`} style={{ fontSize: 11 }}>
                  {item.cadence}
                </span>
                <span className={`num tabular money ${item.lastAmountCents > 0 ? "pos" : ""}`} style={{ fontSize: 14, fontFamily: "var(--mono)" }}>
                  {fmt(item.lastAmountCents)}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
})()}
```

Also reset `selectedDay` to `null` when the month offset changes. Add this effect inside `CalendarView` after the `offset` state:

```tsx
useEffect(() => { setSelectedDay(null); }, [offset]);
```

This requires adding `useEffect` to the import at the top of the file. The file currently imports `{ useState, useMemo }` — change to `{ useState, useMemo, useEffect }`.

- [ ] **Step 3: Run TypeScript type-check + tests**

```
cd ui && npx tsc --noEmit && npx vitest run
```

Expected: 0 type errors, 51+ tests pass.

- [ ] **Step 4: Commit**

```bash
git add ui/src/styles/app.css ui/src/screens/Recurring.tsx
git commit -m "feat(ui): recurring calendar day-detail panel"
```

---

## Task 10: Goals — pace chip per card

**Files:**
- Modify: `ui/src/screens/Goals.tsx`

- [ ] **Step 1: Add the `paceStatus` helper function**

After the existing `etaLabel` function (around line 27), add:

```tsx
type PaceStatus = "ahead" | "on_track" | "needs_attention";

function paceStatus(goal: GoalDto): PaceStatus | null {
  if (!goal.targetDate || goal.targetCents === 0) return null;
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return null; // already reached
  if (goal.monthlyCents <= 0) return "needs_attention";
  const monthsRemaining = Math.ceil(remaining / goal.monthlyCents);
  const target = new Date(goal.targetDate);
  const now = new Date();
  const monthsExpected =
    (target.getFullYear() - now.getFullYear()) * 12 +
    (target.getMonth() - now.getMonth());
  if (monthsExpected <= 0) return "needs_attention";
  if (monthsRemaining < monthsExpected * 0.85) return "ahead";
  if (monthsRemaining > monthsExpected * 1.15) return "needs_attention";
  return "on_track";
}

const PACE_LABELS: Record<PaceStatus, { label: string; cls: string }> = {
  ahead: { label: "Ahead", cls: "positive" },
  on_track: { label: "On track", cls: "" },
  needs_attention: { label: "Needs attention", cls: "warning" },
};
```

- [ ] **Step 2: Render the pace chip in `GoalCard`**

Inside `GoalCard`, find the header section that currently renders:
```tsx
<div style={{ fontSize: 15.5, fontWeight: 600, marginBottom: 3 }}>{goal.name}</div>
<span className="chip" style={{ fontSize: 11 }}>{TYPE_LABELS[goal.goalType] || goal.goalType}</span>
```

Add the pace chip after the type chip:

```tsx
<div style={{ fontSize: 15.5, fontWeight: 600, marginBottom: 3 }}>{goal.name}</div>
<div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginTop: 4 }}>
  <span className="chip" style={{ fontSize: 11 }}>{TYPE_LABELS[goal.goalType] || goal.goalType}</span>
  {(() => {
    const pace = paceStatus(goal);
    if (!pace) return null;
    const { label, cls } = PACE_LABELS[pace];
    return <span className={`chip ${cls}`} style={{ fontSize: 11 }}>{label}</span>;
  })()}
</div>
```

- [ ] **Step 3: Run TypeScript type-check + tests**

```
cd ui && npx tsc --noEmit && npx vitest run
```

Expected: 0 type errors, 51+ tests pass.

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/Goals.tsx
git commit -m "feat(ui): pace chip on goal cards (Ahead / On track / Needs attention)"
```

---

## Task 11: Sidebar — transaction count badge + "Run setup again"

**Files:**
- Modify: `ui/src/components/Sidebar.tsx`

- [ ] **Step 1: Add the transaction count query**

In `Sidebar.tsx`, add imports for `useQuery` and `commands`:

```tsx
import { useQuery } from "@tanstack/react-query";
import { commands } from "../api/client";
```

Inside the `Sidebar` component, after the existing `useNeedsReviewCount` call, add:

```tsx
const { data: txnCount = 0 } = useQuery<number>({
  queryKey: ["transaction-count"],
  queryFn: async () => {
    const result = await commands.getTransactionCount();
    if (result.status === "error") throw new Error(result.error.message);
    return result.data;
  },
  staleTime: 60_000,
  refetchInterval: 60_000,
});

const formattedTxnCount =
  txnCount >= 1000 ? `${(txnCount / 1000).toFixed(1)}k` : String(txnCount);
```

- [ ] **Step 2: Add the transaction count badge to the nav item**

Find the `NAV_MAIN` array and update the `transactions` entry to include a badge indicator. The badge will be rendered dynamically instead (since it's live data), so leave the `NAV_MAIN` array as-is and instead modify the JSX rendering loop.

In the `NAV_MAIN.map(...)` render loop, after the existing pulse badge logic, add a transaction count badge for the `"transactions"` item:

```tsx
{NAV_MAIN.map((n) => (
  <NavLink
    key={n.id}
    to={n.path}
    end={n.path === "/"}
    className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
  >
    <n.Icon className="ico" />
    <span>{n.label}</span>
    {n.id === "rules" && needsReview > 0 && (
      <span className="pulse" title={`${needsReview} need review`} />
    )}
    {n.id === "transactions" && txnCount > 0 && (
      <span className="badge" style={{ marginLeft: "auto", fontSize: 11 }}>
        {formattedTxnCount}
      </span>
    )}
    {n.badge && <span className="badge">{n.badge}</span>}
  </NavLink>
))}
```

- [ ] **Step 3: Add "Run setup again" to the sidebar footer**

Import `useResetOnboarding` and `useNavigate`:

```tsx
import { NavLink, useNavigate } from "react-router-dom";
import { useResetOnboarding } from "../api/hooks/onboarding";
```

Inside the `Sidebar` component, add:

```tsx
const navigate = useNavigate();
const resetOnboarding = useResetOnboarding();

const handleRunSetup = async () => {
  await resetOnboarding.mutateAsync();
  navigate("/onboarding");
};
```

Find the sidebar footer section (the `</aside>` closing, or any footer `<div>` inside the sidebar). Add a footer nav item before the closing `</aside>`:

```tsx
{/* Footer */}
<div style={{ marginTop: "auto", padding: "8px 12px", borderTop: "1px solid var(--line)" }}>
  <button
    className="nav-item"
    style={{ width: "100%", textAlign: "left", background: "none", border: "none", cursor: "pointer" }}
    onClick={() => void handleRunSetup()}
  >
    <I.Sparkle className="ico" />
    <span>Run setup again</span>
  </button>
</div>
```

- [ ] **Step 4: Run TypeScript type-check + tests**

```
cd ui && npx tsc --noEmit && npx vitest run
```

Expected: 0 type errors, 51+ tests pass.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/Sidebar.tsx
git commit -m "feat(ui): sidebar transaction count badge + Run setup again footer item"
```

---

## Task 12: Final integration check

- [ ] **Step 1: Run all Rust tests**

```
cargo test --workspace
```

Expected: all pass.

- [ ] **Step 2: Run all frontend tests**

```
cd ui && npx vitest run
```

Expected: 51+ tests, 0 failures.

- [ ] **Step 3: Run TypeScript type-check**

```
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 4: Start the app and smoke-test each feature**

```
cd ui && npm run dev
```

Open the app and verify:
1. **Transactions:** Search bar filters by merchant name, filter tabs switch between All/Needs review/Anomalies/No category
2. **Categories:** "Year to date" tab shows `yearTotalCents`, "Budget" column shows budget amounts with red when over
3. **Budget:** "To Budget" pill shows income minus assigned, "By activity" sort orders by transaction count
4. **Recurring → Calendar view:** Clicking a day with items shows the detail panel below the grid; clicking again hides it
5. **Goals:** Each goal with a target date shows a pace chip (Ahead/On track/Needs attention)
6. **Sidebar:** Transaction count badge appears next to "Transactions" nav item; "Run setup again" button in footer navigates to onboarding

# Account Page Transaction Filter

**Date:** 2026-06-30  
**Status:** Implemented  
**Scope:** Make the "Filter" button on the Accounts detail page functional by adding inline search, date range, and preset chips. Extract a reusable `TransactionFilter` component from the existing Transactions screen.

## Implementation

Implemented on 2026-06-30 via subagent-driven execution of `docs/superpowers/plans/2026-06-30-account-transaction-filter.md`.

- `193f8dc` — feat: add reusable TransactionFilter component
- `cf20b0a` — feat: wire TransactionFilter into Accounts page
- `67bca37` — refactor: Transactions page uses shared TransactionFilter

Verification: `cargo check --workspace` passed; `TransactionFilter`, `Accounts`, and `Transactions` tests passed; `tsc --noEmit` passed. The full suite has one pre-existing failure in `src/components/copilot/TauriRuntime.test.tsx` unrelated to this work.

## Context

The Accounts page (`ui/src/screens/Accounts.tsx`) shows recent activity for the selected account in a table. A **Filter** button appears above the table but currently has no `onClick` handler. The page already fetches transactions for the selected account via `useTransactions({ accountId: selectedAccount.id, ... })`, and the backend's `TxnFilterInput` supports `search`, `startDate`, `endDate`, and `filterPreset`.

The Transactions page (`ui/src/screens/Transactions.tsx`) already contains a search input and preset chips (All / Needs review / Anomalies) with local client-side filtering. It does not currently expose date-range filters. This design creates a reusable `TransactionFilter` component that adds date-range inputs and shares all three controls (search, dates, presets) across both screens.

## Goals

1. Make the Account detail page **Filter** button toggle a filter bar.
2. Provide controls for **search text**, **start/end dates**, and **preset chips**.
3. Reuse the same filter UI on the Transactions page to keep UX consistent.
4. Keep the implementation simple: no URL persistence, no backend changes.

## Non-goals

- URL query-string persistence for filter state.
- New backend endpoints or filter fields.
- Refactoring the entire Transactions page beyond the filter controls.

## Design

### Component: `TransactionFilter`

**Location:** `ui/src/components/TransactionFilter.tsx`

**Props:**

| Prop | Type | Description |
|------|------|-------------|
| `value` | `TxnFilterInput` | Current filter values. |
| `onChange` | `(filter: TxnFilterInput) => void` | Called whenever any input changes. |
| `counts` | `{ review: number; anomalies: number }` | Optional badge counts for preset chips. |
| `className` | `string` | Optional extra CSS class for the container. |

**Rendered UI:**

1. **Search input** with a magnifying-glass icon. Placeholder: "Search by merchant, note, amount, or category…"
2. **Start date** input (`type="date"`).
3. **End date** input (`type="date"`).
4. **Preset chips** using the existing `.toolbar` pattern:
   - `All`
   - `Needs review` (badge shows `counts.review` if > 0)
   - `Anomalies` (badge shows `counts.anomalies` if > 0)

**Behavior:**

- Changing the search input calls `onChange` with `search` updated.
- Changing a date input calls `onChange` with `startDate` or `endDate` updated (ISO `YYYY-MM-DD` or `null`).
- Clicking a preset chip calls `onChange` with `filterPreset` set to:
  - `null` for All
  - `"needs_review"` for Needs review
  - `"anomalies"` for Anomalies
- Preset chips reflect the current `value.filterPreset` with an `on` class.

### Integration: `Accounts.tsx`

1. Add local state for filter visibility and values:
   - `filterOpen: boolean`
   - `search: string`
   - `startDate: string | null`
   - `endDate: string | null`
   - `preset: "all" | "needs_review" | "anomalies"`
2. When the user selects a different account, reset the filter state to defaults.
3. Extend the existing `txFilter` memo to include the new fields.
4. Wire the **Filter** button to toggle `filterOpen`.
5. Render `TransactionFilter` conditionally between the section header and the transactions table.
6. Show an empty state in the table body when `recentTransactions.length === 0`.

### Integration: `Transactions.tsx`

1. Replace the inline search input and preset chips with `<TransactionFilter value={...} onChange={...} counts={...} />`.
2. Use the date-range values from the component in the existing client-side filtering logic (`filtered` memo) so date filters also apply on this page.
3. Maintain existing behavior: search filters across merchant, label, notes, category, account, and formatted amount.

### Data Flow

```
Accounts.tsx
  ├─ selectedAccount
  ├─ filter state (open, search, dates, preset)
  ├─ txFilter = { accountId, search, startDate, endDate, filterPreset }
  ├─ useTransactions(txFilter) → refetches on change
  └─ renders TransactionFilter when filterOpen
```

### Error Handling

- Empty filtered results display a friendly row in the table: "No transactions match your filters."
- Invalid date range (start > end) is left to the backend; the UI does not block submission.

### Styling

- Reuse existing classes: `.toolbar`, `.chip`, `.btn`, `.tbl`, and inline styles used in Transactions.tsx.
- Match spacing, font sizes, and colors to the Transactions page filter bar.

### Testing

1. **Unit test `TransactionFilter.test.tsx`:**
   - Changing the search input calls `onChange` with updated `search`.
   - Changing a date calls `onChange` with updated `startDate`/`endDate`.
   - Clicking preset chips calls `onChange` with the correct `filterPreset`.
2. **Update `Accounts.test.tsx`:**
   - Clicking the **Filter** button renders the filter bar.
   - Changing search updates the `useTransactions` query key and refetches.
3. **Update `Transactions.test.tsx` (if needed):**
   - Ensure existing search and preset behavior still passes after extracting the component.

## Risks

- The Transactions page currently filters client-side; `TransactionFilter` only supplies values. Keeping this split means `Accounts.tsx` uses backend filtering while `Transactions.tsx` still filters locally. This is acceptable because the backend already supports the same fields.
- Date input styling varies across browsers, but the app already uses native `type="date"` inputs elsewhere.

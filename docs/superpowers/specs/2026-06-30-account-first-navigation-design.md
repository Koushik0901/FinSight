# Account-First Navigation Redesign

**Date:** 2026-06-30  
**Status:** Approved  
**Scope:** Remove the global Transactions page from the app navigation and replace it with per-account transaction registers. Clicking an account on the Accounts list navigates to a dedicated account register page.

## Context

FinSight currently has two overlapping ways to browse transactions:

1. **Accounts page (`/accounts`)** — lists accounts and shows a detail card with the selected account's recent transactions.
2. **Transactions page (`/transactions`)** — a global, searchable register of every transaction across all accounts, reachable from the sidebar.

Budgeting apps like YNAB and Actual use an account-first model: there is no global transaction ledger in the primary navigation; instead, each account has its own register. This aligns with how users mentally organize their money and keeps each screen focused on one account at a time.

## Goals

1. Remove the **Transactions** item from the sidebar navigation.
2. Remove the `/transactions` route (or repurpose it; see Non-goals).
3. Make the **Accounts** list page the only gateway to transaction registers.
4. Clicking an account row navigates to `/accounts/:id/transactions` showing the full register for that account.
5. Add a clear **Back to accounts** path from the register.
6. Reuse the existing `TransactionFilter` component and transaction table UI.
7. Free the right-hand detail card on `/accounts` for future, more meaningful content.

## Non-goals

- Do not redesign the Accounts list page beyond removing the recent-transactions card.
- Do not add new backend endpoints or transaction fields.
- Do not implement any replacement content for the freed real estate on `/accounts`; leave it empty/clean for a future iteration.
- Do not preserve the global `/transactions` route as a public page. It may be removed entirely or redirected.

## Design

### Navigation & Routing

| Route | Purpose |
|-------|---------|
| `/accounts` | Account list only. |
| `/accounts/:id/transactions` | Full transaction register for the account with `id`. |
| `/transactions` | **Removed** from sidebar and routing. |

### Sidebar Changes

Remove this entry from `NAV_MAIN` in `ui/src/components/Sidebar.tsx`:

```ts
{ id: "transactions", path: "/transactions", label: "Transactions", Icon: I.Flow },
```

Also remove the transaction-count badge logic associated with `id === "transactions"`.

### Accounts List Page (`/accounts`)

- Keep the existing account list with sparklines, balances, and account metadata.
- Remove the right-hand detail card entirely (the sticky card showing recent activity, balance chart, etc.).
- Each account row becomes a navigational link to `/accounts/:id/transactions`.
- The page no longer fetches `useTransactions`, `useAccountBalanceHistory`, or `useAccountBalanceSparklines` for the detail card. Sparklines may still be shown inline in the list if desired; fetching them for the list is acceptable but should be re-evaluated if performance becomes an issue.

### Account Register Page (`/accounts/:id/transactions`)

**New screen:** `ui/src/screens/AccountTransactions.tsx`

**Responsibilities:**
- Resolve the account from `useParams().id`.
- Show a header with:
  - Back button linking to `/accounts`
  - Account display name (`getAccountDisplayName`)
  - Account type / bank metadata
  - Current balance
  - Sync status / "Sync now" button (for SimpleFin accounts)
- Render the transaction filter bar (`TransactionFilter`).
- Render the full transaction table with all transactions for the account.
- Support inline editing via `TransactionDrawer`.
- Support exporting the filtered register to CSV.
- Support adding a manual transaction to this account.

**Data flow:**
```
AccountTransactions.tsx
  ├─ useParams().id
  ├─ useAccounts() → find account by id
  ├─ local filter state (search, dates, preset)
  ├─ useTransactions({ accountId: id, search, startDate, endDate, filterPreset })
  └─ renders TransactionFilter + transaction table
```

**Header layout (inspired by YNAB/Actual):**

```
┌─────────────────────────────────────────────────────────┐
│ ← Back to accounts        Chase Checking      $3,240.00 │
│ Chase · checking · ••••          [Sync now]  [Export]   │
├─────────────────────────────────────────────────────────┤
│ [Search...]  [Start] [End]  All | Needs review | Anom.. │
├─────────────────────────────────────────────────────────┤
│ DATE    MERCHANT    CATEGORY    AMOUNT                  │
│ ...                                                     │
└─────────────────────────────────────────────────────────┘
```

### Reuse & Refactoring

- Extract the transaction table row rendering into a reusable component if it isn't already. The current `Transactions.tsx` table can be copied or extracted.
- Use the existing `TransactionFilter` component for filters.
- Reuse `TransactionDrawer` for editing.
- Reuse the export CSV command (`commands.exportTransactionsCsv`) with `accountId` set.

### Error Handling

- If the account id does not exist, show a stub: "Account not found" with a back link.
- Loading state shows the standard page loader.
- Empty filtered results show "No transactions match your filters."

### Styling

- Follow existing `.screen`, `.day-hdr`, `.tbl`, `.toolbar`, `.btn`, `.chip`, `.money` classes.
- Use the account's type color (`getAccountTypeColor`) for accent elements if appropriate.

### Testing

1. **Unit tests for `AccountTransactions.tsx`**:
   - Renders account header with name and balance.
   - Renders transactions for the account.
   - Clicking a transaction row opens `TransactionDrawer`.
   - Back button navigates to `/accounts`.
   - Filter changes call `useTransactions` with updated filter.
2. **Update `Accounts.test.tsx`**:
   - Remove tests for the removed detail card.
   - Add test: clicking an account row navigates to `/accounts/:id/transactions`.
3. **Update `Sidebar.test.tsx`** (if it checks nav items):
   - Remove Transactions from expected nav items.
4. **Update `App.test.tsx`** (if it checks routes):
   - Remove `/transactions` route expectation.

## Risks

- Users currently relying on the global Transactions page will need to adjust. Mitigated by the fact that every transaction still belongs to an account and is reachable from the Accounts list.
- The right-hand card on `/accounts` is removed; its useful features (sync button, export, balance chart) move to the new register page.
- Removing `/transactions` may break bookmarks. We can add a redirect from `/transactions` to `/accounts` if desired.

## Future Work

- Use the freed real estate on `/accounts` for a net-worth mini-chart, account groups, or quick-action widgets.
- Add account-specific settings or reconciliation UI to the register page.

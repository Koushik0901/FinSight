# Account-First Navigation Redesign Implementation Plan

> **Status:** Pending implementation
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the global Transactions page and replace it with per-account transaction registers accessible from the Accounts list.

**Architecture:** Add a new route `/accounts/:id/transactions` served by a new `AccountTransactions` screen. The screen resolves the account from the URL, fetches its transactions, and renders an account header plus the shared `TransactionFilter` and transaction table. Remove the `/transactions` route from `App.tsx` and the sidebar, and simplify `Accounts.tsx` to a pure account list.

**Tech Stack:** React, React Router, TypeScript, TanStack Query, vitest, @testing-library/react.

## Global Constraints

- Remove the **Transactions** item from `Sidebar.tsx` navigation.
- Remove the `/transactions` route from `App.tsx`.
- `/accounts` becomes a pure account list; remove the detail card and recent-transactions table.
- New route: `/accounts/:id/transactions`.
- Reuse `TransactionFilter`, `TransactionDrawer`, and existing transaction table patterns.
- No backend changes; reuse existing `TxnFilterInput` and `listTransactions`.
- Run `cd ui && npx tsc --noEmit` and relevant tests before each commit.

---

### Task 1: Remove global Transactions from navigation and routing

**Files:**
- Modify: `ui/src/components/Sidebar.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/test/App.test.tsx` (if it asserts on routes)

**Interfaces:**
- Consumes: Existing `NavEntry` type and `NAV_MAIN` array.
- Produces: Sidebar no longer renders a Transactions link; `App.tsx` no longer registers `/transactions`.

- [ ] **Step 1: Write/update the failing test**

  In `ui/src/test/App.test.tsx` (or create if it does not exist), ensure the test no longer expects `/transactions` in the sidebar and does not route to `/transactions`.

  If the file already contains route assertions, update them:
  ```tsx
  // Remove or update any assertion like:
  // expect(screen.getByRole('link', { name: /transactions/i })).toBeInTheDocument();
  ```

- [ ] **Step 2: Remove Transactions from Sidebar**

  In `ui/src/components/Sidebar.tsx`, remove this entry from `NAV_MAIN`:
  ```ts
  { id: "transactions", path: "/transactions", label: "Transactions", Icon: I.Flow },
  ```

  Remove the transaction-count badge logic in `renderBadge`:
  ```ts
  if (id === "transactions" && txnCount > 0) return <span className="badge">{formattedTxnCount}</span>;
  ```

  Remove unused code that becomes orphaned:
  - `const { data: txnCount = 0 } = useQuery<number>({ ... })` block (lines 51–60)
  - `const formattedTxnCount = ...` line

- [ ] **Step 3: Remove /transactions route from App.tsx**

  In `ui/src/App.tsx`:
  - Remove the lazy import: `const Transactions = lazy(() => import("./screens/Transactions"));`
  - Remove the route: `<Route path="/transactions" element={<Transactions />} />`

- [ ] **Step 4: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/test/App.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/components/Sidebar.tsx ui/src/App.tsx ui/src/test/App.test.tsx
  git commit -m "refactor: remove global Transactions from nav and routing"
  ```

---

### Task 2: Simplify Accounts page to a pure account list

**Files:**
- Modify: `ui/src/screens/Accounts.tsx`
- Modify: `ui/src/screens/Accounts.test.tsx`
- Delete: `ui/src/components/AccountBalanceChart.tsx` (optional, if no longer used)
- Delete: `ui/src/components/AccountSparkline.tsx` (optional, if no longer used)

**Interfaces:**
- Consumes: `useAccounts`, `useManualAssets`, `useLiabilities`, `useNetWorth`, account drawer components.
- Produces: `Accounts` renders a list of clickable account rows using React Router `useNavigate`.

- [ ] **Step 1: Update Accounts.tsx to navigate on row click**

  Add import:
  ```tsx
  import { useNavigate } from "react-router-dom";
  ```

  Inside the component, add:
  ```tsx
  const navigate = useNavigate();
  ```

  Remove all state and hooks related to the detail card:
  - `selectedId`, `setSelectedId`
  - `useAccountBalanceHistory`
  - `useAccountBalanceSparklines`
  - `useTransactions` and `txFilter`
  - `filterOpen`, `search`, `startDate`, `endDate`, `preset`
  - `TransactionFilter` import
  - `AccountSparkline` and `AccountBalanceChart` imports (if removing sparklines)
  - `formatStamp` helper (if only used by detail card)
  - `syncAccount` mutation (if only used by detail card)

  Keep:
  - `useAccounts`
  - `useManualAssets`, `useLiabilities`, `useNetWorth`
  - `useSyncAllSimpleFinAccounts` (for the "Connect bank" button)
  - Account/Asset/Liability drawer state and components

  Change each account row from a `<button>` to a clickable element that navigates:
  ```tsx
  <button
    key={account.id}
    type="button"
    onClick={() => navigate(`/accounts/${account.id}/transactions`)}
    style={{
      width: "100%",
      textAlign: "left",
      display: "grid",
      gridTemplateColumns: "12px 1fr 120px",
      gap: 14,
      alignItems: "center",
      padding: "14px 16px",
      borderBottom: "1px solid var(--hairline)",
      background: "transparent",
      cursor: "pointer",
    }}
  >
    <span className="cswatch" style={{ background: account.balance_cents >= 0 ? "var(--positive)" : "var(--negative)" }} />
    <div>
      <div>{getAccountDisplayName(account)}</div>
      <div className="muted" style={{ fontSize: 12 }}>{account.bank} · {account.type}</div>
    </div>
    <div className="figure money" style={{ fontSize: 16, textAlign: "right", color: account.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(account.balance_cents, { currency: account.currency || "USD", decimals: 2 })}</div>
  </button>
  ```

  Remove the entire right-column detail card JSX (the sticky card with recent activity, balance chart, filters, and table).

  Update the grid layout to a single column or keep the two-column layout but use the right side as empty placeholder for future widgets:
  ```tsx
  <div className="section">
    <div className="card flush">
      {accounts.map((account) => (
        // clickable row from above
      ))}
    </div>
  </div>
  ```

- [ ] **Step 2: Update Accounts.test.tsx**

  Remove tests for the detail card and filters.
  Add a navigation test:
  ```tsx
  import { render, screen } from "@testing-library/react";
  import userEvent from "@testing-library/user-event";
  import { MemoryRouter, Routes, Route } from "react-router-dom";
  import Accounts from "../screens/Accounts";

  const mockNavigate = vi.fn();
  vi.mock("react-router-dom", async () => {
    const actual = await vi.importActual("react-router-dom");
    return { ...actual, useNavigate: () => mockNavigate };
  });

  it("navigates to the account register when an account row is clicked", async () => {
    render(<Accounts />);
    const row = await screen.findByText("Chase Checking");
    await userEvent.click(row.closest("button")!);
    expect(mockNavigate).toHaveBeenCalledWith("/accounts/acc-1/transactions");
  });
  ```

- [ ] **Step 3: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/screens/Accounts.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [ ] **Step 4: Commit**

  ```bash
  git add ui/src/screens/Accounts.tsx ui/src/screens/Accounts.test.tsx
  git commit -m "refactor: simplify Accounts page to pure list"
  ```

---

### Task 3: Create AccountTransactions screen

**Files:**
- Create: `ui/src/screens/AccountTransactions.tsx`
- Create: `ui/src/screens/AccountTransactions.test.tsx`
- Modify: `ui/src/App.tsx`

**Interfaces:**
- Consumes: `useParams`, `useNavigate`, `useAccounts`, `useTransactions`, `TransactionFilter`, `TransactionDrawer`, `commands.exportTransactionsCsv`.
- Produces: `AccountTransactions` screen rendered at `/accounts/:id/transactions`.

- [ ] **Step 1: Write the failing test**

  In `ui/src/screens/AccountTransactions.test.tsx`:
  ```tsx
  import { describe, it, expect, vi } from "vitest";
  import { render, screen, fireEvent } from "@testing-library/react";
  import userEvent from "@testing-library/user-event";
  import { MemoryRouter, Routes, Route } from "react-router-dom";
  import AccountTransactions from "./AccountTransactions";

  vi.mock("../api/hooks/accounts", () => ({
    useAccounts: () => ({
      data: [
        {
          id: "acc-1",
          name: "Chase Checking",
          bank: "Chase",
          type: "checking",
          balance_cents: 324000,
          currency: "USD",
          color: "#0f0",
          last_synced_at: "2026-06-30T10:00:00Z",
        },
      ],
    }),
  }));

  vi.mock("../api/hooks/transactions", () => ({
    useTransactions: () => ({
      data: [
        {
          id: "txn-1",
          account_id: "acc-1",
          posted_at: "2026-06-28T00:00:00Z",
          merchant_raw: "Whole Foods",
          merchant_label: "Whole Foods",
          amount_cents: -8432,
          category_label: "Groceries",
          category_color: "#4caf50",
        },
      ],
    }),
    useCategoriesWithSpending: () => ({ data: [] }),
  }));

  vi.mock("../api/hooks/agent", () => ({
    useNeedsReviewCount: () => ({ data: 0 }),
    useAgentStatus: () => ({ data: {} }),
  }));

  vi.mock("../api/hooks/simplefin", () => ({
    useSyncSimpleFinAccount: () => ({ mutateAsync: vi.fn() }),
  }));

  vi.mock("../api/client", async () => {
    const actual = await vi.importActual("../api/client");
    return {
      ...actual,
      commands: {
        exportTransactionsCsv: vi.fn(() => Promise.resolve({ status: "ok", data: "/path/to/export.csv" })),
      },
    };
  });

  describe("AccountTransactions", () => {
    it("renders account header and transactions", async () => {
      render(
        <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
          <Routes>
            <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
          </Routes>
        </MemoryRouter>
      );
      expect(await screen.findByText("Chase Checking")).toBeInTheDocument();
      expect(screen.getByText("Whole Foods")).toBeInTheDocument();
      expect(screen.getByText(/-84\.32/)).toBeInTheDocument();
    });

    it("navigates back to accounts when back button is clicked", async () => {
      const user = userEvent.setup();
      render(
        <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
          <Routes>
            <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
            <Route path="/accounts" element={<div>Accounts list</div>} />
          </Routes>
        </MemoryRouter>
      );
      const back = await screen.findByRole("button", { name: /Back to accounts/i });
      await user.click(back);
      expect(screen.getByText("Accounts list")).toBeInTheDocument();
    });
  });
  ```

- [ ] **Step 2: Run test to verify it fails**

  Run: `cd ui && npx vitest run src/screens/AccountTransactions.test.tsx`
  Expected: FAIL — module not found.

- [ ] **Step 3: Implement AccountTransactions.tsx**

  ```tsx
  import { useMemo, useState } from "react";
  import { useParams, useNavigate, Link } from "react-router-dom";
  import { toast } from "sonner";
  import { useAccounts } from "../api/hooks/accounts";
  import { useTransactions, useCategoriesWithSpending } from "../api/hooks/transactions";
  import { useNeedsReviewCount, useAgentStatus } from "../api/hooks/agent";
  import { useSyncSimpleFinAccount } from "../api/hooks/simplefin";
  import { commands } from "../api/client";
  import type { TxnFilterInput } from "../api/client";
  import TransactionFilter from "../components/TransactionFilter";
  import TransactionDrawer from "../components/TransactionDrawer";
  import { getAccountDisplayName, getAccountTypeColor } from "../utils/accounts";
  import { money } from "../utils/format";
  import { userErrorMessage } from "../utils/runtime";

  function formatStamp(value: string | null | undefined) {
    if (!value) return "Never synced";
    return new Date(value).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
  }

  function avatarColor(name: string) {
    let hash = 0;
    for (let i = 0; i < name.length; i += 1) hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
    const colors = ["var(--c-housing)", "var(--c-groceries)", "var(--c-dining)", "var(--c-transport)", "var(--c-travel)", "var(--c-shopping)"];
    return colors[Math.abs(hash) % colors.length] || "var(--accent)";
  }

  function avatarText(name: string) {
    return name.replace(/[^A-Za-z0-9]/g, "").slice(0, 2).toUpperCase() || "TX";
  }

  function formatDate(iso: string) {
    return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
  }

  export default function AccountTransactions() {
    const { id } = useParams<{ id: string }>();
    const navigate = useNavigate();
    const { data: accounts = [] } = useAccounts();
    const { data: categories = [] } = useCategoriesWithSpending();
    const { data: needsReviewCount = 0 } = useNeedsReviewCount();
    const { data: agentStatus } = useAgentStatus();
    const syncAccount = useSyncSimpleFinAccount();

    const [search, setSearch] = useState("");
    const [startDate, setStartDate] = useState<string | null>(null);
    const [endDate, setEndDate] = useState<string | null>(null);
    const [preset, setPreset] = useState<"all" | "needs_review" | "anomalies">("all");
    const [editTxnId, setEditTxnId] = useState<string | null>(null);
    const [addOpen, setAddOpen] = useState(false);

    const account = accounts.find((a) => a.id === id);

    const filterValue: TxnFilterInput = useMemo(
      () => ({
        accountId: id ?? null,
        limit: null,
        offset: null,
        search: search || null,
        filterPreset: preset === "all" ? null : preset,
        startDate,
        endDate,
      }),
      [id, search, preset, startDate, endDate]
    );

    const { data: transactions = [], isLoading, error } = useTransactions(filterValue);

    const categoryById = useMemo(
      () => Object.fromEntries(categories.map((c) => [c.id, c])),
      [categories]
    );

    const handleFilterChange = (next: TxnFilterInput) => {
      setSearch(next.search ?? "");
      setStartDate(next.startDate ?? null);
      setEndDate(next.endDate ?? null);
      setPreset((next.filterPreset as "all" | "needs_review" | "anomalies") ?? "all");
    };

    const handleExport = async () => {
      if (!account) return;
      try {
        const result = await commands.exportTransactionsCsv(filterValue);
        if (result.status === "ok" && result.data) toast.success("Exported", { description: result.data });
      } catch (exportError) {
        toast.error("Export failed", { description: userErrorMessage(exportError, "Try again.") });
      }
    };

    if (isLoading) return <div className="stub">Loading transactions…</div>;
    if (error) return <div className="stub" role="alert">Error loading transactions.</div>;
    if (!account) {
      return (
        <div className="stub" role="alert">
          Account not found.
          <br />
          <Link to="/accounts" className="btn primary sm" style={{ marginTop: 12 }}>Back to accounts</Link>
        </div>
      );
    }

    const editTxn = transactions.find((t) => t.id === editTxnId) ?? null;

    return (
      <div className="screen screen-account-transactions">
        <div className="day-hdr">
          <div>
            <button className="btn ghost sm" type="button" onClick={() => navigate("/accounts")}>← Back to accounts</button>
            <div className="eyebrow" style={{ marginTop: 10 }}><span className="dot" />{account.bank.toUpperCase()} · {account.type.toUpperCase()}</div>
            <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>{getAccountDisplayName(account)}</h1>
          </div>
          <div style={{ textAlign: "right" }}>
            <div className="figure money" style={{ fontSize: 34, color: account.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(account.balance_cents, { currency: account.currency || "USD", decimals: 2 })}</div>
            <div className="row row-sm wrap" style={{ justifyContent: "flex-end", marginTop: 10 }}>
              <span className="chip">Updated {formatStamp(account.last_synced_at)}</span>
              {account.simplefin_account_id && (
                <button className="btn ghost sm" type="button" onClick={async () => {
                  try {
                    const result = await syncAccount.mutateAsync(account.id);
                    toast.success(`Synced ${result.added} new transaction${result.added === 1 ? "" : "s"}`);
                  } catch (syncError) {
                    toast.error("Sync failed", { description: userErrorMessage(syncError, "Check your bank connection and try again.") });
                  }
                }}>Sync now</button>
              )}
              <button className="btn outline sm" type="button" onClick={handleExport}>Export</button>
              <button className="btn primary sm" type="button" onClick={() => setAddOpen(true)}>Add manual</button>
            </div>
          </div>
        </div>

        <div style={{ marginTop: 14 }}>
          <TransactionFilter
            value={filterValue}
            onChange={handleFilterChange}
            counts={{ review: needsReviewCount, anomalies: agentStatus?.anomalyCount ?? 0 }}
          />
        </div>

        <div className="section">
          <div className="card flush">
            <table className="tbl">
              <thead>
                <tr>
                  <th>DATE</th>
                  <th>MERCHANT</th>
                  <th>CATEGORY</th>
                  <th className="right">AMOUNT</th>
                </tr>
              </thead>
              <tbody>
                {transactions.length === 0 ? (
                  <tr>
                    <td colSpan={4} className="muted" style={{ padding: 24, textAlign: "center" }}>
                      No transactions match your filters.
                    </td>
                  </tr>
                ) : (
                  transactions.map((transaction) => {
                    const category = transaction.category_id ? categoryById[transaction.category_id] : undefined;
                    const merchantName = transaction.merchant_label ?? transaction.merchant_raw;
                    const avatarBg = transaction.merchant_color || avatarColor(merchantName);
                    return (
                      <tr key={transaction.id} onClick={() => setEditTxnId(transaction.id)} style={{ cursor: "pointer" }}>
                        <td style={{ width: 76 }}><span className="mono faint">{formatDate(transaction.posted_at)}</span></td>
                        <td>
                          <div className="row row-sm" style={{ alignItems: "center" }}>
                            <div aria-hidden="true" style={{ width: 26, height: 26, borderRadius: 7, background: avatarBg, color: "var(--accent-ink)", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 700, flexShrink: 0 }}>{avatarText(merchantName)}</div>
                            <div>
                              <div className="row row-sm wrap" style={{ alignItems: "center" }}>
                                <span>{merchantName}</span>
                                {transaction.ai_confidence !== null && transaction.ai_confidence < 0.6 && <span className="chip warning">Needs review</span>}
                                {transaction.is_split && <span className="chip">Split</span>}
                                {transaction.is_reimbursable && <span className="chip accent">Reimbursable</span>}
                              </div>
                              {transaction.notes && <div className="muted" style={{ fontSize: 12 }}>{transaction.notes}</div>}
                            </div>
                          </div>
                        </td>
                        <td><div className="row row-sm"><span className="cswatch" style={{ background: transaction.category_color || category?.color || "var(--ink-faint)" }} /><span>{transaction.category_label || category?.label || "Uncategorized"}</span></div></td>
                        <td className="right"><span className={`figure money ${transaction.amount_cents > 0 ? "pos" : ""}`} style={{ fontSize: 16 }}>{money(transaction.amount_cents, { currency: account.currency || "USD", decimals: 2 })}</span></td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>

        <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} accountId={account.id} />
        <TransactionDrawer open={editTxnId !== null} onClose={() => setEditTxnId(null)} transaction={editTxn ?? undefined} accountId={account.id} />
      </div>
    );
  }
  ```

  Note: `TransactionDrawer` may not accept `accountId` currently. If it does not, omit that prop and verify the drawer works without it. Adjust the prop interface if needed in Task 4.

- [ ] **Step 4: Register the route in App.tsx**

  In `ui/src/App.tsx`:
  - Add lazy import: `const AccountTransactions = lazy(() => import("./screens/AccountTransactions"));`
  - Add route: `<Route path="/accounts/:id/transactions" element={<AccountTransactions />} />`

- [ ] **Step 5: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/screens/AccountTransactions.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [ ] **Step 6: Commit**

  ```bash
  git add ui/src/screens/AccountTransactions.tsx ui/src/screens/AccountTransactions.test.tsx ui/src/App.tsx
  git commit -m "feat: add AccountTransactions screen and route"
  ```

---

### Task 4: Adapt TransactionDrawer for account-scoped creation

**Files:**
- Modify: `ui/src/components/TransactionDrawer.tsx` (if needed)

**Interfaces:**
- Consumes: Optional `accountId` prop.
- Produces: When `accountId` is provided, new transactions default to that account.

- [ ] **Step 1: Inspect TransactionDrawer props**

  Read `ui/src/components/TransactionDrawer.tsx` to determine whether it accepts an `accountId` prop and how `NewTransaction` is built.

- [ ] **Step 2: Add accountId prop if missing**

  If the drawer does not accept `accountId`, add it:
  ```tsx
  interface TransactionDrawerProps {
    open: boolean;
    onClose: () => void;
    transaction?: Transaction;
    accountId?: string;
  }
  ```

  When creating a new transaction, set `account_id` to `accountId` if provided:
  ```tsx
  const defaultAccountId = accountId ?? accounts[0]?.id ?? "";
  ```

- [ ] **Step 3: Update AccountTransactions.tsx if prop name differs**

  Ensure `AccountTransactions` passes the correct prop name.

- [ ] **Step 4: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/components/TransactionDrawer.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/components/TransactionDrawer.tsx ui/src/components/TransactionDrawer.test.tsx
  git commit -m "feat: support accountId prop in TransactionDrawer"
  ```

---

### Task 5: Clean up unused components and update remaining tests

**Files:**
- Delete: `ui/src/screens/Transactions.tsx`
- Delete: `ui/src/test/Transactions.test.tsx`
- Delete: `ui/src/components/AccountBalanceChart.tsx` (if no longer used)
- Delete: `ui/src/components/AccountSparkline.tsx` (if no longer used)

**Interfaces:**
- Consumes: Files identified as unused after Tasks 1–3.
- Produces: Smaller codebase with no dead components.

- [ ] **Step 1: Confirm components are unused**

  Search for imports of `Transactions`, `AccountBalanceChart`, and `AccountSparkline`:
  ```bash
  cd ui && rg "from \"../screens/Transactions\"|from \"./Transactions\"|AccountBalanceChart|AccountSparkline" src
  ```
  Expected: Only imports in the files themselves and their tests.

- [ ] **Step 2: Delete unused files**

  ```bash
  git rm ui/src/screens/Transactions.tsx
  git rm ui/src/test/Transactions.test.tsx
  git rm ui/src/components/AccountBalanceChart.tsx
  git rm ui/src/components/AccountSparkline.tsx
  ```

  If any file is still imported, do not delete it yet; update the importing file first.

- [ ] **Step 3: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run
  cd ui && npx tsc --noEmit
  ```
  Expected: All tests pass (except pre-existing failures), no type errors.

- [ ] **Step 4: Commit**

  ```bash
  git commit -m "chore: remove unused Transactions screen and account chart components"
  ```

---

### Task 6: Final verification

**Files:**
- Modify: None (verification only).

- [ ] **Step 1: Run full frontend test suite**

  Run: `cd ui && npx vitest run`
  Expected: All new tests pass; pre-existing failures remain unchanged.

- [ ] **Step 2: Run Rust check**

  Run: `cargo check --workspace`
  Expected: No errors.

- [ ] **Step 3: Commit any final fixes**

  If verification surfaced issues, commit fixes; otherwise this task is verification only.

---

## Self-Review

**Spec coverage:**
- ✅ Remove Transactions from sidebar → Task 1
- ✅ Remove `/transactions` route → Task 1
- ✅ `/accounts` becomes pure list → Task 2
- ✅ New `/accounts/:id/transactions` route and screen → Task 3
- ✅ Back navigation → Task 3
- ✅ Reuse `TransactionFilter` and transaction table → Task 3
- ✅ Tests → Tasks 1–3
- ✅ Cleanup unused files → Task 5

**Placeholder scan:**
- ✅ No TBD/TODO placeholders
- ✅ All code blocks contain concrete implementation
- ✅ Exact commands provided

**Type consistency:**
- ✅ `TxnFilterInput` fields used consistently
- ✅ Route params typed as `string`
- ✅ Prop names consistent across `AccountTransactions` and `TransactionDrawer`

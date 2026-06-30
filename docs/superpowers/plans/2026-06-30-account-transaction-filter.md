# Account Page Transaction Filter Implementation Plan

> **Status:** ✅ Completed 2026-06-30
>
> Implemented via `superpowers:subagent-driven-development`. Commits:
> - `193f8dc` — feat: add reusable TransactionFilter component
> - `cf20b0a` — feat: wire TransactionFilter into Accounts page
> - `67bca37` — refactor: Transactions page uses shared TransactionFilter
>
> Original note for agentic workers: REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add functional inline transaction filters (search, date range, presets) to the Accounts page and extract a reusable `TransactionFilter` component shared with the Transactions page.

**Architecture:** Create a controlled `TransactionFilter` component that accepts a `TxnFilterInput` value and an `onChange` callback. Use it in `Accounts.tsx` to toggle filter controls and feed the existing `useTransactions` query. Update `Transactions.tsx` to use the same component while preserving its client-side filtering behavior.

**Tech Stack:** React, TypeScript, TanStack Query, vitest, @testing-library/react, Tauri/Specta bindings.

## Global Constraints

- Filter controls: search text, start/end date inputs, preset chips (All / Needs review / Anomalies).
- Preset values map to `filterPreset`: `null` (All), `"needs_review"`, `"anomalies"`.
- Date inputs use native `type="date"` and produce `YYYY-MM-DD` strings or `null`.
- No URL persistence; filter state is local component state.
- No backend changes; reuse existing `TxnFilterInput` fields.
- Keep `Transactions.tsx` client-side filtering intact.
- Follow existing styling with `.toolbar`, `.chip`, `.btn`, `.tbl`.
- Run `pnpm tsc --noEmit` and relevant tests before each commit.

---

### Task 1: Create `TransactionFilter` component with tests

**Files:**
- Create: `ui/src/components/TransactionFilter.tsx`
- Create: `ui/src/components/TransactionFilter.test.tsx`
- Modify: `ui/src/components/index.ts` (if exists; otherwise skip)

**Interfaces:**
- Consumes: `TxnFilterInput` type from `../api/client`.
- Produces: `TransactionFilter` component with props:
  ```ts
  interface TransactionFilterProps {
    value: TxnFilterInput;
    onChange: (filter: TxnFilterInput) => void;
    counts?: { review: number; anomalies: number };
    className?: string;
  }
  ```

- [x] **Step 1: Write the failing test**

  In `ui/src/components/TransactionFilter.test.tsx`:

  ```tsx
  import { describe, it, expect, vi } from "vitest";
  import { render, screen, fireEvent } from "@testing-library/react";
  import TransactionFilter from "./TransactionFilter";
  import type { TxnFilterInput } from "../api/client";

  const baseFilter: TxnFilterInput = {
    accountId: null,
    limit: null,
    offset: null,
    search: null,
    filterPreset: null,
    startDate: null,
    endDate: null,
  };

  describe("TransactionFilter", () => {
    it("calls onChange when search input changes", () => {
      const onChange = vi.fn();
      render(<TransactionFilter value={baseFilter} onChange={onChange} />);
      const input = screen.getByLabelText("Search transactions");
      fireEvent.change(input, { target: { value: "coffee" } });
      expect(onChange).toHaveBeenCalledWith({ ...baseFilter, search: "coffee" });
    });

    it("calls onChange when start date changes", () => {
      const onChange = vi.fn();
      render(<TransactionFilter value={baseFilter} onChange={onChange} />);
      const input = screen.getByLabelText("Start date");
      fireEvent.change(input, { target: { value: "2026-01-01" } });
      expect(onChange).toHaveBeenCalledWith({ ...baseFilter, startDate: "2026-01-01" });
    });

    it("calls onChange when end date changes", () => {
      const onChange = vi.fn();
      render(<TransactionFilter value={baseFilter} onChange={onChange} />);
      const input = screen.getByLabelText("End date");
      fireEvent.change(input, { target: { value: "2026-01-31" } });
      expect(onChange).toHaveBeenCalledWith({ ...baseFilter, endDate: "2026-01-31" });
    });

    it("calls onChange with preset values", () => {
      const onChange = vi.fn();
      render(
        <TransactionFilter
          value={baseFilter}
          onChange={onChange}
          counts={{ review: 5, anomalies: 2 }}
        />
      );
      fireEvent.click(screen.getByRole("button", { name: /Needs review 5/ }));
      expect(onChange).toHaveBeenCalledWith({ ...baseFilter, filterPreset: "needs_review" });

      fireEvent.click(screen.getByRole("button", { name: /Anomalies 2/ }));
      expect(onChange).toHaveBeenCalledWith({ ...baseFilter, filterPreset: "anomalies" });

      fireEvent.click(screen.getByRole("button", { name: /^All$/ }));
      expect(onChange).toHaveBeenCalledWith({ ...baseFilter, filterPreset: null });
    });
  });
  ```

- [x] **Step 2: Run test to verify it fails**

  Run: `cd ui && npx vitest run src/components/TransactionFilter.test.tsx`
  Expected: FAIL — "TransactionFilter" not found or module missing.

- [x] **Step 3: Implement `TransactionFilter.tsx`**

  ```tsx
  import type { TxnFilterInput } from "../api/client";

  interface TransactionFilterProps {
    value: TxnFilterInput;
    onChange: (filter: TxnFilterInput) => void;
    counts?: { review: number; anomalies: number };
    className?: string;
  }

  const PRESETS: { label: string; key: "all" | "needs_review" | "anomalies"; value: string | null }[] = [
    { label: "All", key: "all", value: null },
    { label: "Needs review", key: "needs_review", value: "needs_review" },
    { label: "Anomalies", key: "anomalies", value: "anomalies" },
  ];

  export default function TransactionFilter({ value, onChange, counts, className }: TransactionFilterProps) {
    const activePreset = value.filterPreset ?? "all";

    const update = (patch: Partial<TxnFilterInput>) => {
      onChange({ ...value, ...patch });
    };

    return (
      <div className={className} style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
        <div style={{ flex: 1, display: "flex", alignItems: "center", gap: 10, padding: "8px 14px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10, minWidth: 260 }}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--ink-faint)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
          <input
            type="search"
            value={value.search ?? ""}
            onChange={(e) => update({ search: e.target.value || null })}
            placeholder="Search by merchant, note, amount, or category…"
            aria-label="Search transactions"
            style={{ flex: 1, background: "transparent", border: 0, outline: 0, fontSize: 13.5, color: "var(--ink)" }}
          />
        </div>
        <input
          type="date"
          aria-label="Start date"
          value={value.startDate ?? ""}
          onChange={(e) => update({ startDate: e.target.value || null })}
          style={{ padding: "8px 10px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10, color: "var(--ink)", fontSize: 13 }}
        />
        <input
          type="date"
          aria-label="End date"
          value={value.endDate ?? ""}
          onChange={(e) => update({ endDate: e.target.value || null })}
          style={{ padding: "8px 10px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10, color: "var(--ink)", fontSize: 13 }}
        />
        <div className="toolbar">
          {PRESETS.map((preset) => {
            const count = preset.key === "needs_review" ? counts?.review : preset.key === "anomalies" ? counts?.anomalies : undefined;
            return (
              <button
                key={preset.key}
                className={activePreset === preset.value ? "on" : ""}
                type="button"
                onClick={() => update({ filterPreset: preset.value })}
              >
                {preset.label} {count ? count : ""}
              </button>
            );
          })}
        </div>
      </div>
    );
  }
  ```

- [x] **Step 4: Run tests to verify they pass**

  Run: `cd ui && npx vitest run src/components/TransactionFilter.test.tsx`
  Expected: PASS.

- [x] **Step 5: Run TypeScript check**

  Run: `cd ui && npx tsc --noEmit`
  Expected: No new errors from `TransactionFilter.tsx`.

- [x] **Step 6: Commit**

  ```bash
  git add ui/src/components/TransactionFilter.tsx ui/src/components/TransactionFilter.test.tsx
  git commit -m "feat: add reusable TransactionFilter component"
  ```

---

### Task 2: Wire `TransactionFilter` into `Accounts.tsx`

**Files:**
- Modify: `ui/src/screens/Accounts.tsx`
- Modify: `ui/src/screens/Accounts.test.tsx`

**Interfaces:**
- Consumes: `TransactionFilter` from `../components/TransactionFilter`.
- Produces: Account detail page with working filter controls feeding `useTransactions`.

- [x] **Step 1: Add import and local state**

  Add import:
  ```tsx
  import TransactionFilter from "../components/TransactionFilter";
  ```

  Add local state near other `useState` declarations:
  ```tsx
  const [filterOpen, setFilterOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [startDate, setStartDate] = useState<string | null>(null);
  const [endDate, setEndDate] = useState<string | null>(null);
  const [preset, setPreset] = useState<"all" | "needs_review" | "anomalies">("all");
  ```

- [x] **Step 2: Reset filter when account changes**

  Add a `useEffect` that resets filter state when `selectedAccount?.id` changes:
  ```tsx
  useEffect(() => {
    setFilterOpen(false);
    setSearch("");
    setStartDate(null);
    setEndDate(null);
    setPreset("all");
  }, [selectedAccount?.id]);
  ```

- [x] **Step 3: Update `txFilter` and transaction query**

  Replace the existing `txFilter` memo with:
  ```tsx
  const txFilter = useMemo(
    () => ({
      accountId: selectedAccount?.id ?? null,
      limit: null,
      offset: null,
      search: search || null,
      filterPreset: preset === "all" ? null : preset,
      startDate,
      endDate,
    }),
    [selectedAccount?.id, search, preset, startDate, endDate]
  );
  ```

- [x] **Step 4: Build filter value object for the component**

  ```tsx
  const filterValue = useMemo(
    () => ({
      accountId: selectedAccount?.id ?? null,
      limit: null,
      offset: null,
      search: search || null,
      filterPreset: preset === "all" ? null : preset,
      startDate,
      endDate,
    }),
    [selectedAccount?.id, search, preset, startDate, endDate]
  );
  ```

- [x] **Step 5: Implement `onChange` handler**

  ```tsx
  const handleFilterChange = (next: TxnFilterInput) => {
    setSearch(next.search ?? "");
    setStartDate(next.startDate ?? null);
    setEndDate(next.endDate ?? null);
    setPreset((next.filterPreset as "all" | "needs_review" | "anomalies") ?? "all");
  };
  ```

- [x] **Step 6: Toggle filter button and render component**

  Replace the non-functional Filter button:
  ```tsx
  <button
    className={`btn ghost sm ${filterOpen ? "on" : ""}`}
    type="button"
    onClick={() => setFilterOpen((open) => !open)}
    aria-expanded={filterOpen}
  >
    Filter
  </button>
  ```

  Insert `TransactionFilter` between the header row and the table:
  ```tsx
  {filterOpen && (
    <div style={{ padding: "0 22px 14px" }}>
      <TransactionFilter value={filterValue} onChange={handleFilterChange} />
    </div>
  )}
  ```

- [x] **Step 7: Add empty state in table body**

  Replace the existing `tbody` map with a conditional:
  ```tsx
  <tbody>
    {recentTransactions.length === 0 ? (
      <tr>
        <td colSpan={4} className="muted" style={{ padding: 24, textAlign: "center" }}>
          No transactions match your filters.
        </td>
      </tr>
    ) : (
      recentTransactions.slice(0, 8).map((transaction) => (
        <tr key={transaction.id}>…existing row JSX…</tr>
      ))
    )}
  </tbody>
  ```

- [x] **Step 8: Update `Accounts.test.tsx` mock and add test**

  Ensure `TransactionFilter` is either rendered without mocking or mocked trivially. Add a test:
  ```tsx
  it("opens the filter bar and filters transactions", async () => {
    render(<Accounts />);
    const filterButton = await screen.findByRole("button", { name: /Filter/i });
    await userEvent.click(filterButton);
    const searchInput = screen.getByLabelText("Search transactions");
    await userEvent.type(searchInput, "coffee");
    expect(mockListTransactions).toHaveBeenCalledWith(
      expect.objectContaining({ search: "coffee" })
    );
  });
  ```

- [x] **Step 9: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/screens/Accounts.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [x] **Step 10: Commit**

  ```bash
  git add ui/src/screens/Accounts.tsx ui/src/screens/Accounts.test.tsx
  git commit -m "feat: wire TransactionFilter into Accounts page"
  ```

---

### Task 3: Refactor `Transactions.tsx` to use `TransactionFilter`

**Files:**
- Modify: `ui/src/screens/Transactions.tsx`
- Modify: `ui/src/screens/Transactions.test.tsx` (if tests break)

**Interfaces:**
- Consumes: `TransactionFilter` from `../components/TransactionFilter`.
- Produces: Transactions page using the shared filter component; date range now included in client-side filtering.

- [x] **Step 1: Replace inline filter UI with component**

  Remove the inline search input and preset chip markup (lines ~100–110 in current file). Import and render:
  ```tsx
  import TransactionFilter from "../components/TransactionFilter";
  ```

  Build the filter value:
  ```tsx
  const filterValue = useMemo(
    () => ({
      accountId: null,
      limit: null,
      offset: null,
      search: query || null,
      filterPreset: preset === "all" ? null : preset === "review" ? "needs_review" : "anomalies",
      startDate,
      endDate,
    }),
    [query, preset, startDate, endDate]
  );
  ```

  Render:
  ```tsx
  <TransactionFilter
    value={filterValue}
    onChange={(next) => {
      setQuery(next.search ?? "");
      setPreset(
        next.filterPreset === "needs_review" ? "review" :
        next.filterPreset === "anomalies" ? "anomalies" : "all"
      );
      setStartDate(next.startDate ?? null);
      setEndDate(next.endDate ?? null);
    }}
    counts={{ review: needsReviewCount, anomalies: agentStatus?.anomalyCount ?? 0 }}
  />
  ```

- [x] **Step 2: Add date state and apply to client-side filter**

  Add state:
  ```tsx
  const [startDate, setStartDate] = useState<string | null>(null);
  const [endDate, setEndDate] = useState<string | null>(null);
  ```

  Update the `filtered` memo to include date filtering:
  ```tsx
  const filtered = useMemo(() => {
    const lower = query.trim().toLowerCase();
    return rows.filter((transaction) => {
      if (preset === "review" && !(transaction.ai_confidence !== null && transaction.ai_confidence < 0.6)) return false;
      if (preset === "anomalies" && !transaction.is_anomaly) return false;
      const posted = transaction.posted_at.slice(0, 10);
      if (startDate && posted < startDate) return false;
      if (endDate && posted > endDate) return false;
      if (!lower) return true;
      const haystack = [
        transaction.merchant_raw,
        transaction.merchant_label,
        transaction.notes,
        transaction.category_label,
        accountNameById[transaction.account_id],
        money(transaction.amount_cents, { currency: "USD", decimals: 2 }),
      ].join(" ").toLowerCase();
      return haystack.includes(lower);
    });
  }, [accountNameById, endDate, preset, query, rows, startDate]);
  ```

- [x] **Step 3: Update export CSV call to include dates**

  Update the export button's `exportTransactionsCsv` call:
  ```tsx
  const result = await commands.exportTransactionsCsv({
    accountId: null,
    limit: null,
    offset: null,
    search: query || null,
    filterPreset: preset === "all" ? null : preset === "review" ? "needs_review" : "anomalies",
    startDate,
    endDate,
  });
  ```

- [x] **Step 4: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/screens/Transactions.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [x] **Step 5: Commit**

  ```bash
  git add ui/src/screens/Transactions.tsx ui/src/screens/Transactions.test.tsx
  git commit -m "refactor: Transactions page uses shared TransactionFilter"
  ```

---

### Task 4: Final verification

**Files:**
- Modify: None (verification only).

- [x] **Step 1: Run all frontend tests**

  Run: `cd ui && npx vitest run`
  Expected: All tests pass (or only pre-existing failures remain).

- [x] **Step 2: Run Rust check**

  Run: `cargo check --workspace`
  Expected: No errors.

- [x] **Step 3: Final commit (if any fixes were needed)**

  If tests required fixes, commit them; otherwise this task is verification only.

---

## Self-Review

**Spec coverage:**
- ✅ Inline filter bar on Accounts page → Task 2
- ✅ Search, date range, preset chips → Task 1 component
- ✅ Reusable component shared with Transactions page → Task 3
- ✅ Backend `TxnFilterInput` reused, no backend changes → Global constraints
- ✅ Empty state for no results → Task 2 Step 7
- ✅ Tests → Tasks 1, 2, 3

**Placeholder scan:**
- ✅ No TBD/TODO placeholders
- ✅ All code blocks contain concrete implementation
- ✅ Exact commands provided

**Type consistency:**
- ✅ `TxnFilterInput` fields used consistently across component and screens
- ✅ Preset mapping (`"all" | "needs_review" | "anomalies"`) matches backend strings

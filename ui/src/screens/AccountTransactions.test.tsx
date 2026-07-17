import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import AccountTransactions from "./AccountTransactions";
import { downloadBlob } from "../lib/downloadBlob";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("../components/ImportMappingDialog", () => ({
  default: ({ path }: { path: string }) => <div data-testid="import-mapping-dialog">Map CSV columns for {path}</div>,
}));

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: () => ({
    data: [
      {
        id: "acc-1",
        name: "Chase Checking",
        bank: "Chase",
        type: "checking",
        balance_cents: 324000,
        balance_known: true,
        currency: "USD",
        color: "#0f0",
        last_synced_at: "2026-06-30T10:00:00Z",
      },
    ],
  }),
  useSetAccountBalance: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false }),
}));

const { infiniteSpy } = vi.hoisted(() => ({ infiniteSpy: vi.fn() }));

vi.mock("../api/hooks/transactions", () => ({
  useInfiniteTransactions: (filter: unknown) => infiniteSpy(filter) ?? ({
    data: {
      pages: [
        [
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
          {
            id: "txn-2",
            account_id: "acc-1",
            posted_at: "2026-06-27T00:00:00Z",
            merchant_raw: "Bill Payment to AMEX",
            merchant_label: null,
            amount_cents: -113900,
            category_label: null,
            category_color: null,
            is_transfer: true,
            transfer_peer_id: "txn-peer",
            transfer_peer_account_name: "Amex Cobalt",
          },
          {
            id: "txn-3",
            account_id: "acc-1",
            posted_at: "2026-06-26T00:00:00Z",
            merchant_raw: "INTERAC e-Transfer To: Alice",
            merchant_label: null,
            amount_cents: -4000,
            category_label: null,
            category_color: null,
            is_transfer: true,
            transfer_peer_id: null,
            transfer_peer_account_name: null,
          },
        ],
      ],
      pageParams: [0],
    },
    isLoading: false,
    error: null,
    fetchNextPage: vi.fn(),
    hasNextPage: false,
    isFetchingNextPage: false,
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

vi.mock("../components/TransactionDrawer", () => ({
  // Renders enough to distinguish edit-mode (a real `transaction` prop) from
  // create-mode (`transaction` undefined) — the regression this covers is the
  // drawer silently flipping from one to the other while still open.
  default: ({ open, transaction }: { open: boolean; transaction?: { id: string } }) =>
    open ? <div data-testid="txn-drawer">{transaction ? `edit:${transaction.id}` : "add-mode"}</div> : null,
}));

vi.mock("../api/client", async () => {
  const actual = await vi.importActual("../api/client");
  return {
    ...actual,
    commands: {
      exportTransactionsCsv: vi.fn(() => Promise.resolve({ status: "ok", data: "date,amount\n2026-06-28,-84.32\n" })),
    },
  };
});

vi.mock("../lib/downloadBlob", () => ({
  downloadBlob: vi.fn(),
}));

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
    expect(screen.getByText(/-\$84\.32/)).toBeInTheDocument();
  });

  it("labels a paired transfer with its peer account and an unpaired one plainly", async () => {
    render(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );
    // Paired leg (outflow): direction arrow + the peer account's name.
    expect(await screen.findByText("Transfer → Amex Cobalt")).toBeInTheDocument();
    // Unpaired transfer (e-transfer to a friend): plain label, no arrow.
    expect(screen.getByText("Transfer")).toBeInTheDocument();
  });

  it("opens the import mapping dialog after picking a CSV", async () => {
    (openDialog as ReturnType<typeof vi.fn>).mockResolvedValueOnce("/path/to/export.csv");
    render(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );
    const importBtn = await screen.findByRole("button", { name: /Import/i });
    fireEvent.click(importBtn);
    await waitFor(() => {
      expect(screen.getByText("Map CSV columns for /path/to/export.csv")).toBeInTheDocument();
    });
  });

  it("downloads exported CSV content instead of showing it in a toast", async () => {
    render(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );
    const exportBtn = await screen.findByRole("button", { name: /Export/i });
    fireEvent.click(exportBtn);
    await waitFor(() => {
      expect(downloadBlob).toHaveBeenCalledWith("date,amount\n2026-06-28,-84.32\n", "text/csv", "transactions.csv");
    });
  });

  it("navigates back to accounts when back button is clicked", async () => {
    render(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
          <Route path="/accounts" element={<div>Accounts list</div>} />
        </Routes>
      </MemoryRouter>
    );
    const back = await screen.findByRole("button", { name: /Back to accounts/i });
    fireEvent.click(back);
    expect(screen.getByText("Accounts list")).toBeInTheDocument();
  });
});

describe("AccountTransactions — edit drawer survives filter-changing mutations", () => {
  it("keeps showing the opened transaction after it drops from the active filter's refetched list", async () => {
    // Regression: marking a transaction a transfer (or any edit that removes
    // it from the CURRENTLY ACTIVE filter, e.g. Uncategorized/Possible
    // transfers) invalidates the list query. If the drawer re-derives its
    // `transaction` prop by searching the freshly refetched (now-shorter)
    // list, the still-open drawer silently flips to blank "Add transaction"
    // mode instead of continuing to show the transaction the user is editing.
    const fullList = [
      {
        id: "txn-3",
        account_id: "acc-1",
        posted_at: "2026-06-26T00:00:00Z",
        merchant_raw: "INTERAC e-Transfer To: Alice",
        merchant_label: null,
        amount_cents: -4000,
        category_label: null,
        category_color: null,
        is_transfer: false,
        transfer_peer_id: null,
        transfer_peer_account_name: null,
      },
    ];
    infiniteSpy.mockReturnValue({
      data: { pages: [fullList], pageParams: [0] },
      isLoading: false,
      error: null,
      fetchNextPage: vi.fn(),
      hasNextPage: false,
      isFetchingNextPage: false,
    });

    const { rerender } = render(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );

    fireEvent.click(await screen.findByText(/INTERAC e-Transfer To: Alice/));
    expect(await screen.findByTestId("txn-drawer")).toHaveTextContent("edit:txn-3");

    // Simulate the post-mutation refetch: this filter no longer returns
    // txn-3 (e.g. it's now flagged as a transfer and the active filter is
    // "Possible transfers", which only lists undecided rows).
    infiniteSpy.mockReturnValue({
      data: { pages: [[]], pageParams: [0] },
      isLoading: false,
      error: null,
      fetchNextPage: vi.fn(),
      hasNextPage: false,
      isFetchingNextPage: false,
    });
    rerender(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );

    // The drawer must still be in edit mode for txn-3, not flipped to "add-mode".
    expect(screen.getByTestId("txn-drawer")).toHaveTextContent("edit:txn-3");

    infiniteSpy.mockReset();
  });
});

describe("AccountTransactions — all-accounts mode (/transactions)", () => {
  it("renders the all-accounts ledger and honors the ?filter= deep link", async () => {
    render(
      <MemoryRouter initialEntries={["/transactions?filter=transfer_review"]}>
        <Routes>
          <Route path="/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );
    expect(await screen.findByText("All transactions")).toBeInTheDocument();
    // Each row says which account it belongs to (there is no account header).
    expect(screen.getAllByText(/Chase · Chase Checking/).length).toBeGreaterThan(0);
    // The Inbox deep link flows through to the backend query…
    expect(infiniteSpy).toHaveBeenCalledWith(
      expect.objectContaining({ accountId: null, filterPreset: "transfer_review" })
    );
    // …and the matching chip shows as active.
    expect(screen.getByRole("button", { name: /Possible transfers/i })).toHaveClass("on");
  });

  it("ignores an unknown ?filter= value instead of sending it to the backend", async () => {
    render(
      <MemoryRouter initialEntries={["/transactions?filter=nonsense"]}>
        <Routes>
          <Route path="/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );
    expect(await screen.findByText("All transactions")).toBeInTheDocument();
    expect(infiniteSpy).toHaveBeenCalledWith(
      expect.objectContaining({ filterPreset: null })
    );
  });
});

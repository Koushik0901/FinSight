import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import AccountTransactions from "./AccountTransactions";

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

vi.mock("../api/hooks/transactions", () => ({
  useInfiniteTransactions: () => ({
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
  default: () => null,
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
    expect(screen.getByText(/-\$84\.32/)).toBeInTheDocument();
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

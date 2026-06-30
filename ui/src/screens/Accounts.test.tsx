import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Accounts from "./Accounts";
import { createWrapper } from "../test-utils";

const mocks = vi.hoisted(() => ({
  useTransactions: vi.fn(() => ({ data: [], isLoading: false, error: null })),
}));

vi.mock("../api/hooks/transactions", () => ({
  useTransactions: mocks.useTransactions,
}));

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [
    { id: "acc1", name: "Checking", bank: "Bank", type: "Checking", balance_cents: 10000000, currency: "USD", color: "#3B82F6" },
  ], isLoading: false, error: null })),
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useAccountBalanceSparklines: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useAccountBalanceHistory: vi.fn(() => ({ data: [], isLoading: false, error: null })),
}));

vi.mock("../api/hooks/simplefin", () => ({
  useSyncSimpleFinAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useSyncAllSimpleFinAccounts: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/assets", () => ({
  useManualAssets: vi.fn(() => ({ data: [
    { id: "a1", name: "House", assetType: "property", valueCents: 50000000, currency: "USD", notes: null, createdAt: "2026-06-01T00:00:00Z", updatedAt: "2026-06-01T00:00:00Z" },
  ], isLoading: false })),
  useCreateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useLiabilities: vi.fn(() => ({ data: [
    { id: "l1", name: "Mortgage", liabilityType: "mortgage", balanceCents: 30000000, limitCents: 35000000, aprPct: 5.5, minPaymentCents: 180000, payoffDate: "2045-01-01", currency: "USD", createdAt: "2026-06-01T00:00:00Z", updatedAt: "2026-06-01T00:00:00Z" },
  ] })),
  useCreateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("Accounts — transaction filters", () => {
  beforeEach(() => {
    mocks.useTransactions.mockClear();
  });

  it("toggles the filter bar when Filter is clicked", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    const filterBtn = screen.getByRole("button", { name: "Filter" });
    expect(screen.queryByLabelText("Search transactions")).not.toBeInTheDocument();
    fireEvent.click(filterBtn);
    expect(screen.getByLabelText("Search transactions")).toBeInTheDocument();
    fireEvent.click(filterBtn);
    expect(screen.queryByLabelText("Search transactions")).not.toBeInTheDocument();
  });

  it("calls useTransactions with search when typing in the filter search input", async () => {
    render(<Accounts />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Filter" }));
    const input = screen.getByLabelText("Search transactions");
    fireEvent.change(input, { target: { value: "coffee" } });
    await waitFor(() => {
      expect(mocks.useTransactions).toHaveBeenCalledWith(
        expect.objectContaining({ search: "coffee" })
      );
    });
  });
});

describe("Accounts — manual assets", () => {
  it("renders the manual assets section with an asset row", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Manual assets")).toBeInTheDocument();
    expect(screen.getByText("House")).toBeInTheDocument();
    expect(screen.getByText("$500,000.00")).toBeInTheDocument();
  });

  it("renders the liabilities section with a liability row", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Liabilities")).toBeInTheDocument();
    expect(screen.getByText("Mortgage")).toBeInTheDocument();
  });

  it("shows a net-worth stat of accounts + assets − liabilities", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Net worth total")).toBeInTheDocument();
    // $300,000 value appears in the stat row (outside the header)
    const statValues = screen.getAllByText("$300,000");
    expect(statValues.length).toBeGreaterThan(0);
  });
});

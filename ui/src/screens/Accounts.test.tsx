import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Accounts from "./Accounts";
import { createWrapper, createWrapperWithEntries } from "../test-utils";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [
    { id: "acc-1", name: "Chase Checking", bank: "Chase", type: "Checking", balance_cents: 10000000, currency: "USD", color: "#3B82F6" },
    { id: "acc-2", name: "Ally Savings", bank: "Ally", type: "Savings", balance_cents: 25000000, currency: "USD", color: "#22C55E" },
  ], isLoading: false, error: null })),
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useAccountBalanceHistory: vi.fn(() => ({ data: [] })),
  useAccountBalanceSparklines: vi.fn(() => ({ data: [] })),
}));

vi.mock("../api/hooks/transactions", async () => {
  const actual = await vi.importActual("../api/hooks/transactions");
  return { ...actual, useTransactions: vi.fn(() => ({ data: [], isLoading: false, error: null })) };
});

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

describe("Accounts — navigation", () => {
  beforeEach(() => {
    mockNavigate.mockClear();
  });

  it("navigates to the account register when a connected-account row is clicked", async () => {
    render(<Accounts />, { wrapper: createWrapper() });
    const rows = await screen.findAllByText("Chase Checking");
    const listRow = rows.map((el) => el.closest("button")).find((btn) => btn !== null)!;
    fireEvent.click(listRow);
    expect(mockNavigate).toHaveBeenCalledWith("/accounts/acc-1/transactions");
  });

  it("navigates to the correct account after clicking a different row", async () => {
    render(<Accounts />, { wrapper: createWrapper() });
    const allySavingsRows = screen.getAllByText("Ally Savings").map((el) => el.closest("button")).filter((btn): btn is HTMLButtonElement => btn !== null);
    fireEvent.click(allySavingsRows[0]!);
    expect(mockNavigate).toHaveBeenCalledWith("/accounts/acc-2/transactions");
  });

  it("opens the liability editor when focusLiability is present", async () => {
    render(<Accounts />, { wrapper: createWrapperWithEntries(["/accounts?focusLiability=l1"]) });
    expect(await screen.findByText("Edit liability")).toBeInTheDocument();
    expect(screen.getByText("Mortgage")).toBeInTheDocument();
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
    // (acc-1 $100,000 + acc-2 $250,000) + manual asset $500,000 − liability $300,000 = $550,000
    const statValues = screen.getAllByText("$550,000");
    expect(statValues.length).toBeGreaterThan(0);
  });
});

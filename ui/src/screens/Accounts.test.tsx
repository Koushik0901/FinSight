import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import Accounts from "./Accounts";
import { createWrapper } from "../test-utils";

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [
    { id: "acc1", name: "Checking", bank: "Bank", type: "Checking", balance_cents: 10000000, currency: "USD", color: "#3B82F6" },
  ], isLoading: false, error: null })),
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
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

  it("shows a net-worth header of accounts + assets − liabilities", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Net worth")).toBeInTheDocument();
    const header = screen.getByText("Net worth").closest("header")!;
    expect(within(header).getByText("$300,000")).toBeInTheDocument();
  });
});

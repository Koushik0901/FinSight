import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Accounts from "./Accounts";
import { createWrapper, createWrapperWithEntries } from "../test-utils";
import { useAccounts } from "../api/hooks/accounts";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [
    { id: "acc-1", name: "Chase Checking", bank: "Chase", type: "Checking", balance_cents: 10000000, balance_known: true, currency: "USD", color: "#3B82F6" },
    { id: "acc-2", name: "Ally Savings", bank: "Ally", type: "Savings", balance_cents: 25000000, balance_known: true, currency: "USD", color: "#22C55E" },
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

vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: vi.fn(() => ({ data: [
    { id: "m1", name: "Koushik", color: "#38BDF8", createdAt: "2026-01-01T00:00:00Z" },
    { id: "m2", name: "Swathi", color: "#F472B6", createdAt: "2026-01-02T00:00:00Z" },
  ] })),
  useAccountOwners: vi.fn(() => ({ data: [
    // Ally Savings is JOINT (both members); Chase Checking is unassigned.
    { accountId: "acc-2", memberId: "m1" },
    { accountId: "acc-2", memberId: "m2" },
  ] })),
  useCreateHouseholdMember: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteHouseholdMember: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useSetAccountOwners: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useSetAccountOwnerShares: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useSetSelfMember: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/assets", () => ({
  useManualAssets: vi.fn(() => ({ data: [
    { id: "a1", name: "House", assetType: "property", valueCents: 50000000, currency: "USD", notes: null, createdAt: "2026-06-01T00:00:00Z", updatedAt: "2026-06-01T00:00:00Z" },
  ], isLoading: false })),
  useCreateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
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

  it("shows a Joint badge with owner names and per-owner attribution with equal splits", async () => {
    render(<Accounts />, { wrapper: createWrapper() });

    // Ally Savings (2 owners) gets the Joint badge and both names.
    expect(screen.getByText("Joint")).toBeInTheDocument();
    expect(screen.getByText(/Koushik & Swathi/)).toBeInTheDocument();

    // Attribution: Chase Checking $100,000 is unassigned → Household (shared);
    // Ally Savings $250,000 joint → $125,000 each.
    expect(screen.getByText(/By owner/)).toBeInTheDocument();
    expect(screen.getByText("Household (shared)")).toBeInTheDocument();
    expect(screen.getAllByText("$125,000")).toHaveLength(2);
    expect(screen.getByText("$100,000")).toBeInTheDocument();
  });

  it("opens the account editor with the owner picker from a row's Edit button", async () => {
    render(<Accounts />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByRole("button", { name: "Edit Chase Checking" }));

    // The edit drawer opens with the household owner picker.
    expect(screen.getByRole("heading", { name: "Edit Account" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Owner Koushik" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Owner Swathi" })).toBeInTheDocument();
    expect(screen.getByLabelText("New household member name")).toBeInTheDocument();
  });

  it("opens the unified Add chooser and routes each choice to its drawer", async () => {
    render(<Accounts />, { wrapper: createWrapper() });

    // One unified button — the old separate ones are gone.
    expect(screen.queryByRole("button", { name: "+ Add account" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add manual asset" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Add account, asset, or liability" }));
    expect(screen.getByText("What do you want to add?")).toBeInTheDocument();

    // Picking "Bank account" closes the chooser and opens the account drawer.
    fireEvent.click(screen.getByRole("button", { name: /Bank account/ }));
    expect(screen.queryByText("What do you want to add?")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Add account" })).toBeInTheDocument();
  });

  it("navigates to the correct account after clicking a different row", async () => {
    render(<Accounts />, { wrapper: createWrapper() });
    const allySavingsRows = screen.getAllByText("Ally Savings").map((el) => el.closest("button")).filter((btn): btn is HTMLButtonElement => btn !== null);
    fireEvent.click(allySavingsRows[0]!);
    expect(mockNavigate).toHaveBeenCalledWith("/accounts/acc-2/transactions");
  });

  it("opens the account editor when focusAccount is present", async () => {
    render(<Accounts />, { wrapper: createWrapperWithEntries(["/accounts?focusAccount=acc-2"]) });
    expect(await screen.findByRole("heading", { name: "Edit Account" })).toBeInTheDocument();
    expect(screen.getByDisplayValue("Ally Savings")).toBeInTheDocument();
  });
});

describe("Accounts — manual assets", () => {
  it("renders the manual assets section with an asset row", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Manual assets")).toBeInTheDocument();
    expect(screen.getByText("House")).toBeInTheDocument();
    expect(screen.getByText("$500,000.00")).toBeInTheDocument();
  });

  it("shows a net-worth stat of accounts + assets, with debt counted as a negative-balance account", () => {
    // Debt is a Credit/Loan-type Account with a negative balance, not a
    // separate liabilities-table row. useAccounts() is called both by
    // Accounts.tsx directly and internally by useNetWorth(), so override
    // the persistent implementation (not just one call) for this test.
    const withDebtAccount = {
      data: [
        { id: "acc-1", name: "Chase Checking", bank: "Chase", type: "Checking", balance_cents: 10000000, balance_known: true, currency: "USD", color: "#3B82F6" },
        { id: "acc-2", name: "Ally Savings", bank: "Ally", type: "Savings", balance_cents: 25000000, balance_known: true, currency: "USD", color: "#22C55E" },
        { id: "acc-3", name: "Mortgage", bank: "Manual", type: "Loan", balance_cents: -30000000, balance_known: true, currency: "USD", color: "#F87171" },
      ],
      isLoading: false,
      error: null,
    };
    (useAccounts as unknown as ReturnType<typeof vi.fn>).mockReturnValue(withDebtAccount);
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Net worth total")).toBeInTheDocument();
    // (acc-1 $100,000 + acc-2 $250,000 − acc-3 debt $300,000) + manual asset $500,000 = $550,000
    const statValues = screen.getAllByText("$550,000");
    expect(statValues.length).toBeGreaterThan(0);
  });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import TransactionDrawer from "./TransactionDrawer";
import { createWrapper } from "../test-utils";
import { useAccountOwners, useHouseholdMembers } from "../api/hooks/household";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));

const setFlags = vi.fn();
const setOwner = vi.fn();
const setTransfer = vi.fn();
const applySimilar = vi.fn();

vi.mock("../api/hooks/transactions", () => ({
  useCreateTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useUpdateTransaction: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue({
      transaction: { id: "t1", notes: "edited", category_id: "cat1" },
      proposed_rule: { pattern: "STARBUCKS", category_id: "cat1", category_label: "Food" },
    }),
  })),
  useDeleteTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useCreateRule: vi.fn(() => ({ mutate: vi.fn() })),
  useCategories: vi.fn(() => ({ data: [{ id: "cat1", label: "Food", color: "#f00", group_id: "g1", group_label: "Daily" }] })),
  useSetTransactionFlags: vi.fn(() => ({ mutateAsync: setFlags, isPending: false })),
  useSetTransactionTransfer: vi.fn(() => ({ mutateAsync: setTransfer, isPending: false })),
  useApplyTransferVerdictToSimilar: vi.fn(() => ({ mutateAsync: applySimilar, isPending: false })),
  useSetAnomalyDismissed: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSetTransactionOwner: vi.fn(() => ({ mutate: setOwner })),
  useTransactionSplits: vi.fn(() => ({ data: [] })),
  useSetTransactionSplits: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
}));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
// Household owners default to empty (solo account → no attribution selector); the
// joint-account test overrides these per-render.
vi.mock("../api/hooks/household", () => ({
  useAccountOwners: vi.fn(() => ({ data: [] })),
  useHouseholdMembers: vi.fn(() => ({ data: [] })),
}));
vi.mock("sonner", () => ({ toast: { custom: vi.fn(), error: vi.fn(), success: vi.fn() } }));

const existingTxn = {
  id: "t1", account_id: "a1",
  posted_at: "2024-01-15T00:00:00Z",
  amount_cents: 500, merchant_raw: "STARBUCKS",
  merchant_id: null, merchant_label: null, merchant_color: null, merchant_initials: null,
  category_id: null, category_label: null, category_color: null,
  status: "cleared" as const, notes: null,
  ai_confidence: null, ai_explanation: null, is_anomaly: false,
  created_at: "2024-01-15T00:00:00Z",
  is_reimbursable: false, is_split: false, is_transfer: false,
  transfer_peer_id: null, transfer_peer_account_name: null, owner_member_id: null,
  imported_id: null, source: null,
  raw_synced_data: null, pending: false, external_tx_id: null, external_account_id: null,
};

describe("TransactionDrawer — edit mode", () => {
  beforeEach(() => {
    setFlags.mockReset();
    setOwner.mockReset();
    // Solo account by default — the attribution selector is joint-account-only.
    vi.mocked(useAccountOwners).mockReturnValue({ data: [] } as any);
    vi.mocked(useHouseholdMembers).mockReturnValue({ data: [] } as any);
  });

  it("pre-fills merchant_raw field", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByDisplayValue("STARBUCKS")).toBeInTheDocument();
  });

  it("shows 'Edit Transaction' title and Save Changes button", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByText("Edit Transaction")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /save changes/i })).toBeInTheDocument();
  });

  it("renders category picker", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByRole("listbox", { name: /category/i })).toBeInTheDocument();
  });

  it("shows delete button", () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByRole("button", { name: /delete transaction/i })).toBeInTheDocument();
  });

  it("toggles the reimbursable flag", async () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    fireEvent.click(screen.getByRole("button", { name: /reimbursable/i }));
    await waitFor(() =>
      expect(setFlags).toHaveBeenCalledWith({ id: "t1", isReimbursable: true, isSplit: false })
    );
  });

  it("marks a transaction as a transfer via the Transfer chip", async () => {
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    fireEvent.click(screen.getByRole("button", { name: /^transfer$/i }));
    await waitFor(() =>
      expect(setTransfer).toHaveBeenCalledWith({ id: "t1", isTransfer: true })
    );
  });

  it("offers to apply the verdict to undecided siblings with the same counterparty", async () => {
    const { toast } = await import("sonner");
    setTransfer.mockResolvedValueOnce({
      transaction: { ...existingTxn, is_transfer: true },
      similarPattern: "%swathi%",
      similarLabel: "swathi",
      similarCount: 11,
    });
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    fireEvent.click(screen.getByRole("button", { name: /^transfer$/i }));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());

    const [, opts] = vi.mocked(toast.success).mock.calls.at(-1)!;
    const action = (opts as unknown as { action?: { label: string; onClick: () => Promise<void> } }).action;
    expect(action?.label).toMatch(/11 more with «swathi»/);

    // Taking the offer rules the whole counterparty in one call.
    applySimilar.mockResolvedValueOnce(11);
    await action!.onClick();
    expect(applySimilar).toHaveBeenCalledWith({ pattern: "%swathi%", isTransfer: true });
  });

  it("shows the transfer state and hides the category picker on a transfer", () => {
    render(
      <TransactionDrawer
        open={true}
        onClose={() => {}}
        transaction={{ ...existingTxn, is_transfer: true, transfer_peer_account_name: "Savings" }}
      />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByText("Transfer between your accounts")).toBeInTheDocument();
    expect(screen.getByText(/matched with the opposite leg in savings/i)).toBeInTheDocument();
    // Transfers are never categorized — the picker is replaced by a note.
    expect(screen.queryByRole("listbox", { name: /category/i })).not.toBeInTheDocument();
    expect(screen.getByText(/transfers aren't categorized/i)).toBeInTheDocument();
    // The chip reads as pressed, and clicking it unmarks the transfer.
    const chip = screen.getByRole("button", { name: /^transfer$/i });
    expect(chip).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(chip);
    expect(setTransfer).toHaveBeenCalledWith({ id: "t1", isTransfer: false });
  });

  it("hides the attribution selector on a solo (0/1-owner) account", () => {
    // Default mocks: no owners on this account → nothing to attribute between.
    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );
    expect(screen.queryByRole("combobox", { name: /attribute this transaction to/i })).not.toBeInTheDocument();
  });

  it("shows the 'Attributed to' selector on a joint (2-owner) account and attributes on change", () => {
    // A jointly-owned account (both members own acct "a1", the txn's account) is
    // the only place a per-transaction override is meaningful.
    vi.mocked(useAccountOwners).mockReturnValue({ data: [
      { accountId: "a1", memberId: "m1", shareBps: null },
      { accountId: "a1", memberId: "m2", shareBps: null },
    ] } as any);
    vi.mocked(useHouseholdMembers).mockReturnValue({ data: [
      { id: "m1", name: "Alex", color: "#38BDF8", createdAt: "2026-01-01T00:00:00Z" },
      { id: "m2", name: "Sam", color: "#F472B6", createdAt: "2026-01-02T00:00:00Z" },
    ] } as any);

    render(
      <TransactionDrawer open={true} onClose={() => {}} transaction={existingTxn} />,
      { wrapper: createWrapper() },
    );

    const select = screen.getByRole("combobox", { name: /attribute this transaction to/i });
    expect(select).toBeInTheDocument();
    // Both owners are attribution options, plus the shared default.
    expect(screen.getByRole("option", { name: /shared — split by account ownership/i })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Alex" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Sam" })).toBeInTheDocument();

    // Attributing to one member calls the override mutation with that member.
    fireEvent.change(select, { target: { value: "m1" } });
    expect(setOwner).toHaveBeenCalledWith({ transactionId: "t1", memberId: "m1" });
  });
});

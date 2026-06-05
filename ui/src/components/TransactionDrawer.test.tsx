import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import TransactionDrawer from "./TransactionDrawer";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));

const setFlags = vi.fn();

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
}));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
vi.mock("sonner", () => ({ toast: { custom: vi.fn(), error: vi.fn() } }));

const existingTxn = {
  id: "t1", account_id: "a1",
  posted_at: "2024-01-15T00:00:00Z",
  amount_cents: 500, merchant_raw: "STARBUCKS",
  merchant_id: null, merchant_label: null, merchant_color: null, merchant_initials: null,
  category_id: null, category_label: null, category_color: null,
  status: "cleared" as const, notes: null,
  ai_confidence: null, ai_explanation: null, is_anomaly: false,
  created_at: "2024-01-15T00:00:00Z",
  is_reimbursable: false, is_split: false,
};

describe("TransactionDrawer — edit mode", () => {
  beforeEach(() => {
    setFlags.mockReset();
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
});

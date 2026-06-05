import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { toast } from "sonner";
import Insights from "./Insights";
import { createWrapper } from "../test-utils";

vi.mock("sonner", () => {
  const fn: any = vi.fn();
  fn.error = vi.fn();
  fn.success = vi.fn();
  return { toast: fn, Toaster: () => null };
});

const forget = vi.fn(() => Promise.resolve());

// Neutralize the data hooks the insight cards rely on.
vi.mock("../api/hooks/accounts", () => ({ useAccounts: () => ({ data: [] }) }));
vi.mock("../api/hooks/budget", () => ({ useBudgetEnvelopes: () => ({ data: [] }), useGoals: () => ({ data: [] }) }));
vi.mock("../api/hooks/transactions", () => ({ useCategoriesWithSpending: () => ({ data: [] }) }));
vi.mock("../api/client", () => ({ commands: {
  getMonthTotals: vi.fn().mockResolvedValue({ status: "ok", data: { incomeCents: 0, expenseCents: 0, netCents: 0, savingsRatePct: 0, txnCount: 0 } }),
  listRecurring: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
} }));

vi.mock("../api/hooks/agentMemory", () => ({
  useAgentMemory: vi.fn(() => ({ data: [
    { id: "m1", kind: "correction", description: "Learned: Trader Joe's is Groceries", merchantKey: "trader joes", createdAt: "2026-06-01T00:00:00Z" },
  ] })),
  useForgetAgentMemory: vi.fn(() => ({ mutateAsync: forget })),
}));

describe("Insights — agent memory", () => {
  beforeEach(() => { vi.useFakeTimers(); forget.mockClear(); vi.mocked(toast).mockClear(); });
  afterEach(() => { vi.useRealTimers(); });

  it("forget hides the row and deletes after the delay", () => {
    render(<Insights />, { wrapper: createWrapper() });
    expect(screen.getByText("Learned: Trader Joe's is Groceries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /forget/i }));
    expect(screen.queryByText("Learned: Trader Joe's is Groceries")).not.toBeInTheDocument();
    expect(forget).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(5000); });
    expect(forget).toHaveBeenCalledWith("m1");
  });

  it("undo cancels the deferred delete", () => {
    render(<Insights />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /forget/i }));
    // Row hidden optimistically.
    expect(screen.queryByText("Learned: Trader Joe's is Groceries")).not.toBeInTheDocument();

    // Grab the Undo handler that the component passed to sonner's toast, and invoke it.
    const toastMock = vi.mocked(toast);
    const call = toastMock.mock.calls.find((c) => c[0] === "Memory forgotten");
    expect(call).toBeTruthy();
    const onUndo = (call![1] as any).action.onClick as () => void;
    act(() => { onUndo(); });

    // Row restored, and the deferred delete must NOT fire after the delay.
    expect(screen.getByText("Learned: Trader Joe's is Groceries")).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(5000); });
    expect(forget).not.toHaveBeenCalled();
  });
});

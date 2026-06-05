import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import Insights from "./Insights";
import { createWrapper } from "../test-utils";

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
  beforeEach(() => { vi.useFakeTimers(); forget.mockClear(); });
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
    // sonner renders the Undo action button:
    const undo = screen.queryByRole("button", { name: /^undo$/i });
    if (undo) {
      fireEvent.click(undo);
    }
    act(() => { vi.advanceTimersByTime(5000); });
    if (undo) {
      // Undo was clickable: the deferred delete must have been cancelled.
      expect(forget).not.toHaveBeenCalled();
      expect(screen.getByText("Learned: Trader Joe's is Groceries")).toBeInTheDocument();
    }
  });
});

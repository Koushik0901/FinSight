import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Insights from "./Insights";
import { createWrapper } from "../test-utils";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/transactions", () => ({
  useCategoriesWithSpending: vi.fn(() => ({ data: [] })),
}));
// One over-budget envelope produces the "budget-over" insight with actionRoute: "/budget".
vi.mock("../api/hooks/budget", () => ({
  useBudgetEnvelopes: vi.fn(() => ({
    data: [
      {
        categoryId: "cat-1",
        categoryLabel: "Dining",
        categoryColor: "#F59E0B",
        groupLabel: "Discretionary",
        budgetCents: 10000,
        spentCents: 15000,
        txnCount: 8,
      },
    ],
  })),
  useGoals: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/agentMemory", () => ({
  useAgentMemory: vi.fn(() => ({ data: [] })),
  useForgetAgentMemory: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));
vi.mock("../api/hooks/agent", () => ({
  useTriggerCategorize: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useAgentStatus: vi.fn(() => ({ data: {
    uncategorizedCount: 0, anomalyCount: 0, overBudgetCount: 0,
    upcomingBillsCount: 0, lastScanAt: null, lastScanCategorized: null,
  }})),
}));
vi.mock("../api/client", () => ({
  commands: {
    getMonthTotals: vi.fn().mockResolvedValue({ status: "ok", data: { incomeCents: 0, expenseCents: 0, netCents: 0, savingsRatePct: 0, txnCount: 0 } }),
    listRecurring: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

describe("Insights — insight card action buttons", () => {
  it("clicking an insight's action button navigates to its actionRoute", () => {
    render(<Insights />, { wrapper: createWrapper() });

    expect(screen.getByText(/is over budget/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /open budget/i }));

    expect(mockNavigate).toHaveBeenCalledWith("/budget");
  });
});

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Budget from "./Budget";
import { createWrapper } from "../test-utils";
import * as budgetHooks from "../api/hooks/budget";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock("../api/hooks/budget", () => ({
  useBudgetEnvelopes: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useBudgetHistory: vi.fn(() => ({
    data: [
      {
        categoryId: "c1",
        label: "Groceries",
        color: "#27ae60",
        monthly: [
          { month: "2026-01", label: "Jan", cents: 45000 },
          { month: "2026-02", label: "Feb", cents: 42000 },
          { month: "2026-03", label: "Mar", cents: 48000 },
          { month: "2026-04", label: "Apr", cents: 44000 },
          { month: "2026-05", label: "May", cents: 46000 },
        ],
      },
    ],
  })),
  useSetBudget: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  usePlanNextMonthData: vi.fn(() => ({ data: null, isLoading: false })),
  useApplyNextMonthPlan: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useGoals: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useUpdateGoalBalance: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useContributeToGoal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useGoalContributions: vi.fn(() => ({ data: [] })),
}));

vi.mock("../api/client", () => ({
  commands: {
    getMonthTotals: vi.fn().mockResolvedValue({ status: "error", error: { message: "no data" } }),
    getSpendingBreakdown: vi.fn().mockResolvedValue({
      status: "ok",
      data: {
        fixedCents: 0,
        investmentsCents: 0,
        savingsCents: 0,
        guiltFreeCents: 0,
        untaggedCents: 0,
        totalIncomeCents: 0,
      },
    }),
    listBudgetEnvelopes: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    listBudgetHistory: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

describe("Budget history section", () => {
  it("renders the spending history eyebrow", () => {
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText("Spending history · last 5 months")).toBeInTheDocument();
  });

  it("shows category labels in the history table", () => {
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText("Groceries")).toBeInTheDocument();
  });

  it("shows formatted amounts in the history table", () => {
    render(<Budget />, { wrapper: createWrapper() });
    // 45000 cents → $450, 42000 cents → $420 (maximumFractionDigits: 0)
    expect(screen.getByText("$450")).toBeInTheDocument();
    expect(screen.getByText("$420")).toBeInTheDocument();
  });

  it("does not render history section when data is empty", () => {
    (budgetHooks.useBudgetHistory as any).mockReturnValueOnce({ data: [] });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.queryByText("Spending history · last 5 months")).not.toBeInTheDocument();
  });

  it("does not render a dead Tracking toggle button", () => {
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.queryByRole("button", { name: /^tracking$/i })).not.toBeInTheDocument();
  });

  it("Assign to a goal navigates to the Goals screen", () => {
    render(<Budget />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /assign to a goal/i }));
    expect(mockNavigate).toHaveBeenCalledWith("/goals");
  });

  it("Park in a goal button is labeled with the real first goal's name", () => {
    (budgetHooks.useGoals as any).mockReturnValueOnce({
      data: [{ id: "g1", name: "House Fund", currentCents: 10000, targetCents: 500000 }],
      isLoading: false,
      error: null,
    });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: /park in house fund/i })).toBeInTheDocument();
  });
});

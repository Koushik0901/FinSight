import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Budget from "./Budget";
import { createWrapper } from "../test-utils";
import * as budgetHooks from "../api/hooks/budget";

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
}));

vi.mock("../api/client", () => ({
  commands: {
    getMonthTotals: vi.fn().mockResolvedValue({ status: "error", error: { message: "no data" } }),
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
    vi.mocked(budgetHooks.useBudgetHistory).mockReturnValueOnce({ data: [] } as ReturnType<typeof budgetHooks.useBudgetHistory>);
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.queryByText("Spending history · last 5 months")).not.toBeInTheDocument();
  });
});

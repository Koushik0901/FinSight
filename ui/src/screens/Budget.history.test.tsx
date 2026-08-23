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
  useBudgetEnvelopes: vi.fn(() => ({
    data: [{
      categoryId: "c1",
      categoryLabel: "Groceries",
      categoryColor: "#27ae60",
      groupLabel: "Everyday",
      budgetCents: 50000,
      spentCents: 45000,
      carryoverCents: 0,
      txnCount: 12,
    }],
    isLoading: false,
    error: null,
  })),
  useBudgetHistory: vi.fn(() => ({
    data: [
      {
        categoryId: "c1",
        label: "Groceries",
        color: "#27ae60",
        monthly: [
          { month: "2026-01", label: "Jan", spentCents: 45000, budgetedCents: 50000 },
          { month: "2026-02", label: "Feb", spentCents: 42000, budgetedCents: 50000 },
          { month: "2026-03", label: "Mar", spentCents: 48000, budgetedCents: 50000 },
          { month: "2026-04", label: "Apr", spentCents: 44000, budgetedCents: 50000 },
          { month: "2026-05", label: "May", spentCents: 46000, budgetedCents: 50000 },
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
  useMemberBudgetEnvelopes: vi.fn(() => ({ data: [] })),
}));

vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/reports", () => ({
  useMonthTotals: vi.fn(() => ({ data: { incomeCents: 100000, expenseCents: 45000 } })),
}));


vi.mock("../api/client", () => ({
  unwrap: async (p: Promise<{ status: "ok" | "error"; data?: unknown; error?: { message: string } }>) => { const r = await p; if (r.status === "error") throw new Error(r.error?.message ?? "command failed"); return r.data; },
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
    expect(screen.getAllByText("Groceries").length).toBeGreaterThan(0);
  });

  it("shows formatted amounts in the history table", () => {
    render(<Budget />, { wrapper: createWrapper() });
    // 45000 cents → $450, 42000 cents → $420 (maximumFractionDigits: 0).
    // $450 also happens to be the 5-month average, so it legitimately appears
    // twice (the Jan cell and the "Your typical" column) — getAllByText, not getByText.
    expect(screen.getAllByText("$450").length).toBeGreaterThan(0);
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

  it("Choose a goal navigates to the Goals screen", () => {
    render(<Budget />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /choose a goal/i }));
    expect(mockNavigate).toHaveBeenCalledWith("/goals");
  });

  it("keeps rendering persisted budget data when a background refresh fails", () => {
    (budgetHooks.useBudgetEnvelopes as any).mockReturnValueOnce({
      data: [{
        categoryId: "c1",
        categoryLabel: "Groceries",
        categoryColor: "#27ae60",
        groupLabel: "Everyday",
        budgetCents: 50000,
        spentCents: 45000,
        carryoverCents: 0,
        txnCount: 12,
      }],
      isLoading: false,
      error: new Error("offline"),
    });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.queryByText("Budget could not load")).not.toBeInTheDocument();
    expect(screen.getByText("Where the plan stands today.")).toBeInTheDocument();
  });

  it("Record goal progress button is labeled with the real first goal's name", () => {
    (budgetHooks.useGoals as any).mockReturnValueOnce({
      data: [{ id: "g1", name: "House Fund", currentCents: 10000, targetCents: 500000 }],
      isLoading: false,
      error: null,
    });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: /record toward house fund/i })).toBeInTheDocument();
  });

  it("explains that recording goal progress does not move money", () => {
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText(/does not move money/i)).toBeInTheDocument();
  });

  it("shows a confidence-aware month-end range instead of a precise forecast", () => {
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText("Likely month-end range")).toBeInTheDocument();
    expect(
      screen.getByText(/^(Early|Developing|Higher-confidence) estimate$/),
    ).toBeInTheDocument();
    expect(screen.queryByText("End-of-month estimate")).not.toBeInTheDocument();
  });

  it("does not repeat attention envelopes in the remaining list", () => {
    (budgetHooks.useBudgetEnvelopes as any).mockReturnValueOnce({
      data: [{
        categoryId: "c1",
        categoryLabel: "Groceries",
        categoryColor: "#27ae60",
        groupLabel: "Everyday",
        budgetCents: 50000,
        spentCents: 51000,
        carryoverCents: 0,
        txnCount: 12,
      }],
      isLoading: false,
      error: null,
    });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText("Start with these categories.")).toBeInTheDocument();
    expect(screen.queryByText("Remaining envelopes.")).not.toBeInTheDocument();
  });


  it("shows a carryover line when carryoverCents is non-zero", () => {
    (budgetHooks.useBudgetEnvelopes as any).mockReturnValueOnce({
      data: [
        {
          categoryId: "c1",
          categoryLabel: "Utilities",
          categoryColor: "#FACC15",
          groupLabel: "Fixed costs",
          budgetCents: 35000,
          spentCents: 30000,
          carryoverCents: 4200,
          txnCount: 3,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText("Carried from last month")).toBeInTheDocument();
    expect(screen.getByText("+$42")).toBeInTheDocument();
  });

  it("groups zero-budget, zero-spend, zero-carryover categories under 'Not yet budgeted'", () => {
    (budgetHooks.useBudgetEnvelopes as any).mockReturnValueOnce({
      data: [
        {
          categoryId: "c1",
          categoryLabel: "Utilities",
          categoryColor: "#FACC15",
          groupLabel: "Fixed costs",
          budgetCents: 35000,
          spentCents: 30000,
          carryoverCents: 0,
          txnCount: 3,
        },
        {
          categoryId: "c2",
          categoryLabel: "Hobbies",
          categoryColor: "#818CF8",
          groupLabel: "Lifestyle",
          budgetCents: 0,
          spentCents: 0,
          carryoverCents: 0,
          txnCount: 0,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Budget />, { wrapper: createWrapper() });
    expect(screen.getByText("Not yet budgeted · 1")).toBeInTheDocument();
    expect(screen.getByText("Set budget")).toBeInTheDocument();
  });
});

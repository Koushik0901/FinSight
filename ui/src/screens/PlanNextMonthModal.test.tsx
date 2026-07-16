import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import PlanNextMonthModal from "./PlanNextMonthModal";
import { createWrapper } from "../test-utils";

const applyMutate = vi.fn();
const updateGoalMonthlyMutate = vi.fn();

vi.mock("../api/hooks/budget", () => ({
  usePlanNextMonthData: vi.fn(() => ({ data: undefined, isLoading: true })),
  useApplyNextMonthPlan: vi.fn(() => ({ mutateAsync: applyMutate, isPending: false })),
  useUpdateGoalMonthly: vi.fn(() => ({ mutateAsync: updateGoalMonthlyMutate, isPending: false })),
  useBudgetEnvelopes: vi.fn(() => ({ data: [] })),
  useBudgetHistory: vi.fn(() => ({ data: [] })),
}));

vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

const MOCK_DATA = {
  incomeCents: 500000,
  recurringExpenseCents: 80000,
  lookBack: [
    { categoryId: "c2", categoryLabel: "Groceries", kind: "under", amountCents: 200, streakMonths: 0 },
  ],
  sinkingFunds: [
    { id: "s1", name: "Car insurance", goalType: "sinking-fund", targetCents: 48000, currentCents: 20000, monthlyCents: 8000, targetDate: null, color: "#000", notes: null, purpose: null, sortOrder: 0, createdAt: "2026-01-01", accountId: null },
  ],
  goals: [
    { id: "g1", name: "Emergency Fund", goalType: "build-balance", targetCents: 1000000, currentCents: 250000, monthlyCents: 90000, targetDate: null, color: "#000", notes: null, purpose: null, sortOrder: 0, createdAt: "2026-01-01", accountId: null },
  ],
  categories: [
    {
      categoryId: "c1",
      label: "Rent",
      color: "#e74c3c",
      groupLabel: "Fixed costs",
      budgetCents: 150000,
      m0Cents: 150000,
      m1Cents: 150000,
      m2Cents: 150000,
    },
    {
      categoryId: "c2",
      label: "Groceries",
      color: "#27ae60",
      groupLabel: "Daily life",
      budgetCents: 40000,
      m0Cents: 38000,
      m1Cents: 42000,
      m2Cents: 41000,
    },
  ],
};

describe("PlanNextMonthModal", () => {
  beforeEach(async () => {
    vi.clearAllMocks();

    const budget = await import("../api/hooks/budget");
    vi.mocked(budget.usePlanNextMonthData).mockReturnValue({
      data: MOCK_DATA,
      isLoading: false,
    } as ReturnType<typeof budget.usePlanNextMonthData>);
    vi.mocked(budget.useApplyNextMonthPlan).mockReturnValue({
      mutateAsync: applyMutate,
      isPending: false,
    } as unknown as ReturnType<typeof budget.useApplyNextMonthPlan>);
    vi.mocked(budget.useUpdateGoalMonthly).mockReturnValue({
      mutateAsync: updateGoalMonthlyMutate,
      isPending: false,
    } as unknown as ReturnType<typeof budget.useUpdateGoalMonthly>);
  });

  it("renders the Look back step by default", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getByText("First, look back.")).toBeInTheDocument();
    expect(screen.getByText("Step 1 of 7 · Look back")).toBeInTheDocument();
    expect(screen.getAllByText("$5,000").length).toBeGreaterThan(0);
  });

  it("navigates to the Fixed costs step on Next click", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →"));
    expect(screen.getByText("What's already spoken for?")).toBeInTheDocument();
    expect(screen.getByText("Step 2 of 7 · Fixed costs")).toBeInTheDocument();
  });

  it("shows Back button after navigating forward", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →"));
    expect(screen.getByText("← Back")).toBeInTheDocument();
  });

  it("calls onClose when ✕ Close is clicked", () => {
    const onClose = vi.fn();
    render(<PlanNextMonthModal onClose={onClose} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("✕ Close"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("reaches the Review step after 6 Next clicks", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    for (let i = 0; i < 6; i++) {
      fireEvent.click(screen.getByText("Next →"));
    }
    expect(screen.getByText("Apply budget")).toBeInTheDocument();
  });

  it("calls apply and onClose on Apply budget click", async () => {
    applyMutate.mockResolvedValue(undefined);
    updateGoalMonthlyMutate.mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(<PlanNextMonthModal onClose={onClose} />, { wrapper: createWrapper() });
    for (let i = 0; i < 6; i++) {
      fireEvent.click(screen.getByText("Next →"));
    }
    fireEvent.click(screen.getByText("Apply budget"));
    await waitFor(() => expect(applyMutate).toHaveBeenCalled());
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("updates the live preview's Unassigned total as fixed-cost amounts are entered", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getAllByText("$5,000").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByText("Next →")); // → Fixed costs
    const rentInput = screen.getByDisplayValue("1500"); // Rent budgetCents 150000 → $1,500
    fireEvent.change(rentInput, { target: { value: "2000" } });

    expect(screen.getByText("Unassigned")).toBeInTheDocument();
    expect(screen.getByText("$3,000")).toBeInTheDocument();
  });

  it("shows the sinking funds step with a monthly slider", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →")); // Fixed costs
    fireEvent.click(screen.getByText("Next →")); // Sinking funds
    expect(screen.getByText("Car insurance")).toBeInTheDocument();
  });

  it("shows an Adjust suggestion when a category is over budget 2+ of 3 months", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    // Groceries: budgetCents 40000, m0/m1/m2 = 38000/42000/41000 → over in 2 of 3 months.
    for (let i = 0; i < 5; i++) fireEvent.click(screen.getByText("Next →")); // → Adjust (step index 5)
    expect(screen.getByText("Raise Groceries to $420")).toBeInTheDocument();
  });

  it("shows loading state when data is not ready", async () => {
    const budget = await import("../api/hooks/budget");
    vi.mocked(budget.usePlanNextMonthData).mockReturnValue({
      data: undefined,
      isLoading: true,
    } as ReturnType<typeof budget.usePlanNextMonthData>);
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getByText("Loading…")).toBeInTheDocument();
  });
});

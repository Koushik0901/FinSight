import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import PlanNextMonthModal from "./PlanNextMonthModal";
import { createWrapper } from "../test-utils";

const applyMutate = vi.fn();

vi.mock("../api/hooks/budget", () => ({
  usePlanNextMonthData: vi.fn(() => ({ data: undefined, isLoading: true })),
  useApplyNextMonthPlan: vi.fn(() => ({ mutateAsync: applyMutate, isPending: false })),
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
  goals: [
    { id: "g1", name: "Emergency Fund", targetCents: 1000000, currentCents: 250000 },
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
  });

  it("renders the Income step by default", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getByText("Your estimated income.")).toBeInTheDocument();
    expect(screen.getByText("Step 1 of 6 · Income")).toBeInTheDocument();
    expect(screen.getAllByText("$5,000").length).toBeGreaterThan(0);
  });

  it("navigates to the next step on Next click", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →"));
    expect(screen.getByText("Essential expenses.")).toBeInTheDocument();
    expect(screen.getByText("Step 2 of 6 · Essentials")).toBeInTheDocument();
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

  it("reaches the Review step after 5 Next clicks", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    for (let i = 0; i < 5; i++) {
      fireEvent.click(screen.getByText("Next →"));
    }
    expect(screen.getByText("Apply budget")).toBeInTheDocument();
  });

  it("calls apply and onClose on Apply budget click", async () => {
    applyMutate.mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(<PlanNextMonthModal onClose={onClose} />, { wrapper: createWrapper() });
    for (let i = 0; i < 5; i++) {
      fireEvent.click(screen.getByText("Next →"));
    }
    fireEvent.click(screen.getByText("Apply budget"));
    await waitFor(() => expect(applyMutate).toHaveBeenCalled());
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("updates the live preview's Unassigned total as amounts are entered", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    // Income step: nothing assigned yet, full income is unassigned.
    expect(screen.getAllByText("$5,000").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByText("Next →")); // → Essentials
    const rentInput = screen.getByDisplayValue("1500"); // Rent budgetCents 150000 → $1,500
    fireEvent.change(rentInput, { target: { value: "2000" } });

    // Assigned $2,000 of $5,000 income → $3,000 unassigned.
    expect(screen.getByText("Unassigned")).toBeInTheDocument();
    expect(screen.getByText("$3,000")).toBeInTheDocument();
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

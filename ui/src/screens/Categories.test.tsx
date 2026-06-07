import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Categories from "./Categories";
import { createWrapper } from "../test-utils";

vi.mock("../api/hooks/transactions", () => ({
  useCategoriesWithSpending: vi.fn(() => ({
    data: [
      { id: "c1", label: "Groceries", color: "#4ade80", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 30000, lastMonthCents: 50000, txnCount: 5, yearTotalCents: 300000, budgetCents: 40000 },
      { id: "c2", label: "Dining Out", color: "#fb923c", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 20000, lastMonthCents: 10000, txnCount: 3, yearTotalCents: 150000, budgetCents: 0 },
    ],
    isLoading: false,
    error: null,
  })),
}));

describe("Categories — AI insight sentence", () => {
  it("shows insight sentence when last-month data exists", () => {
    render(<Categories />, { wrapper: createWrapper() });
    // Groceries: thisMonth=30000, lastMonth=50000 → delta=-20000 (dropped, best gainer)
    // Dining Out: thisMonth=20000, lastMonth=10000 → delta=+10000 (rose, top riser)
    // Both names appear in the table AND the insight, so use getAllByText
    expect(screen.getAllByText(/Groceries/).length).toBeGreaterThan(0);
    expect(screen.getByText(/dropped/)).toBeInTheDocument();
    expect(screen.getAllByText(/Dining Out/).length).toBeGreaterThan(0);
    expect(screen.getByText(/rose/)).toBeInTheDocument();
  });
});

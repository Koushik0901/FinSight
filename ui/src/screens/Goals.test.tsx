import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Goals from "./Goals";
import { createWrapper } from "../test-utils";

const mockUpdateMonthly = vi.fn().mockResolvedValue(undefined);

vi.mock("../api/hooks/budget", () => ({
  useGoals: vi.fn(() => ({
    data: [
      { id: "g1", name: "Italy Fund", goalType: "save-by-date",
        targetCents: 500000, currentCents: 100000, monthlyCents: 20000,
        targetDate: "2027-06-01", color: "#C9F950", notes: null, sortOrder: 0, createdAt: "2026-01-01" },
      { id: "g2", name: "Car repair", goalType: "save-by-date",
        targetCents: 200000, currentCents: 50000, monthlyCents: 10000,
        targetDate: new Date(Date.now() + 180 * 86400000).toISOString().slice(0, 10),
        color: "#34D399", notes: null, sortOrder: 1, createdAt: "2026-01-01" },
    ],
    isLoading: false,
    error: null,
  })),
  useCreateGoal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateGoalBalance: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useArchiveGoal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateGoalMonthly: vi.fn(() => ({ mutateAsync: mockUpdateMonthly, isPending: false })),
}));

describe("Goals — sinking funds", () => {
  it("shows sinking fund card for save-by-date goal within a year", () => {
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText("Sinking funds")).toBeInTheDocument();
    expect(screen.getAllByText("Car repair").length).toBeGreaterThanOrEqual(1);
  });
});

describe("Goals — apply what-if", () => {
  it("Apply button calls updateGoalMonthly with correct monthlyCents", async () => {
    render(<Goals />, { wrapper: createWrapper() });
    const slider = screen.getByRole("slider");
    fireEvent.change(slider, { target: { value: "200" } });
    const applyBtn = await waitFor(() =>
      screen.getByRole("button", { name: /apply/i })
    );
    fireEvent.click(applyBtn);
    await waitFor(() => {
      // Italy Fund monthlyCents=20000, extra=200 dollars → 200*100=20000 more cents → new=40000
      expect(mockUpdateMonthly).toHaveBeenCalledWith({
        id: "g1",
        monthlyCents: 40000,
      });
    });
  });
});

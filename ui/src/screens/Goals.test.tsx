import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Goals from "./Goals";
import { createWrapper } from "../test-utils";

const mockUpdateMonthly = vi.fn().mockResolvedValue(undefined);

vi.mock("../api/hooks/budget", () => ({
  useGoals: vi.fn(() => ({
    data: [
      {
        id: "g1", name: "Italy Fund", goalType: "save-by-date",
        targetCents: 500000, currentCents: 100000, monthlyCents: 20000,
        targetDate: "2027-06-01", color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null,
      },
      {
        id: "g2", name: "Car repair", goalType: "save-by-date",
        targetCents: 200000, currentCents: 50000, monthlyCents: 10000,
        targetDate: new Date(Date.now() + 180 * 86400000).toISOString().slice(0, 10),
        color: "#34D399", notes: null, purpose: null, sortOrder: 1,
        createdAt: "2026-01-01", liabilityId: null, accountId: null,
      },
    ],
    isLoading: false,
    error: null,
  })),
  useCreateGoal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateGoalBalance: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useArchiveGoal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateGoalMonthly: vi.fn(() => ({ mutateAsync: mockUpdateMonthly, isPending: false })),
  useUpdateGoalPurpose: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useProjectGoalGrowth: vi.fn(() => ({ data: null })),
}));

vi.mock("../api/hooks/assets", () => ({
  useLiabilities: vi.fn(() => ({ data: [] })),
}));

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));

describe("Goals — sinking funds", () => {
  it("shows sinking fund card for save-by-date goal within a year", () => {
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText("Sinking funds")).toBeInTheDocument();
    expect(screen.getAllByText("Car repair").length).toBeGreaterThanOrEqual(1);
  });
});

describe("Goals — linked liability", () => {
  it("renders linked liability details and disables manual balance edit", async () => {
    const budget = await import("../api/hooks/budget");
    const assets = await import("../api/hooks/assets");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{
        id: "g3", name: "Car payoff", goalType: "debt-payoff",
        targetCents: 2000000, currentCents: 1500000, monthlyCents: 50000,
        targetDate: null, color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", liabilityId: "l1", accountId: null,
      }],
      isLoading: false,
      error: null,
    });
    (assets.useLiabilities as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{ id: "l1", name: "Car loan", balanceCents: 1500000, aprPct: 4.5 }],
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getAllByText("Car payoff").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Linked to Car loan/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /edit balance/i })).not.toBeInTheDocument();
  });
});

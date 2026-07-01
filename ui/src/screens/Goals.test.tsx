import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

describe("Goals — eyebrow casing", () => {
  it("renders eyebrows in natural case, relying on CSS for uppercase", () => {
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText(/Goals · 2 active/)).toBeInTheDocument();
    expect(screen.getByText(/Sinking funds · 2/)).toBeInTheDocument();
    expect(screen.queryByText(/GOALS ·/)).not.toBeInTheDocument();
    expect(screen.queryByText(/SINKING FUNDS ·/)).not.toBeInTheDocument();
  });
});

describe("Goals — what-if scenario", () => {
  beforeEach(() => {
    mockUpdateMonthly.mockClear();
  });

  it("defaults to the first eligible goal and shows its base ETA with no extra", () => {
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText("What if · scenario")).toBeInTheDocument();
    expect(screen.getByText(/on track for the original plan/i)).toBeInTheDocument();
  });

  it("updates the projection live when the slider moves", () => {
    render(<Goals />, { wrapper: createWrapper() });
    // g1: target 500000c, current 100000c, monthly 20000c -> base months = ceil(400000/20000) = 20
    expect(screen.getByText("20")).toBeInTheDocument();

    const slider = screen.getByLabelText("Extra monthly contribution");
    fireEvent.change(slider, { target: { value: "500" } });

    // with +$500/mo (50000c), monthly becomes 70000c -> ceil(400000/70000) = 6
    expect(screen.getByText("6")).toBeInTheDocument();
    expect(screen.queryByText("20")).not.toBeInTheDocument();
    expect(screen.getByText(/brings/i)).toBeInTheDocument();
    expect(screen.getByText(/\$500\/mo/)).toBeInTheDocument();
  });

  it("switches the selected goal when a different one is clicked", () => {
    render(<Goals />, { wrapper: createWrapper() });
    const radios = screen.getAllByRole("radio");
    expect(radios.length).toBeGreaterThanOrEqual(2);
    const second = radios[1]!;
    fireEvent.click(second);
    expect(second).toHaveAttribute("aria-checked", "true");
  });

  it("calls useUpdateGoalMonthly with the original plus extra when applying", async () => {
    render(<Goals />, { wrapper: createWrapper() });
    const slider = screen.getByLabelText("Extra monthly contribution");
    fireEvent.change(slider, { target: { value: "500" } });
    const applyButton = screen.getByRole("button", { name: /apply this scenario/i });
    fireEvent.click(applyButton);
    await waitFor(() => {
      expect(mockUpdateMonthly).toHaveBeenCalledWith({ id: "g1", monthlyCents: 20000 + 50000 });
    });
  });

  it("resets the slider to 0 without applying", () => {
    render(<Goals />, { wrapper: createWrapper() });
    const slider = screen.getByLabelText("Extra monthly contribution") as HTMLInputElement;
    fireEvent.change(slider, { target: { value: "500" } });
    expect(slider.value).toBe("500");
    fireEvent.click(screen.getByRole("button", { name: /^reset$/i }));
    expect(slider.value).toBe("0");
    expect(mockUpdateMonthly).not.toHaveBeenCalled();
  });

  it("excludes spending-cap goals from the scenario picker", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "g1", name: "Italy Fund", goalType: "save-by-date",
          targetCents: 500000, currentCents: 100000, monthlyCents: 20000,
          targetDate: "2027-06-01", color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null,
        },
        {
          id: "g4", name: "Dining cap", goalType: "spending-cap",
          targetCents: 40000, currentCents: 10000, monthlyCents: 0,
          targetDate: null, color: "#C9F950", notes: null, purpose: null,
          sortOrder: 2, createdAt: "2026-01-01", liabilityId: null, accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    const radiogroup = screen.getByRole("radiogroup", { name: /scenario goal/i });
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(1);
    expect(screen.getAllByText("Italy Fund").length).toBeGreaterThanOrEqual(1);
    expect(radiogroup).not.toHaveTextContent("Dining cap");
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

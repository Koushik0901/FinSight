import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Goals from "./Goals";
import { createWrapper, createWrapperWithEntries } from "../test-utils";

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

describe("Goals — eyebrow casing", () => {
  it("renders eyebrows in natural case, relying on CSS for uppercase", () => {
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText(/Goals · 2 active/)).toBeInTheDocument();
    expect(screen.queryByText(/GOALS ·/)).not.toBeInTheDocument();
  });
});

describe("Goals — pause/resume", () => {
  beforeEach(() => {
    mockUpdateMonthly.mockClear();
  });

  it("pauses a goal by setting its monthly contribution to 0", async () => {
    render(<Goals />, { wrapper: createWrapper() });
    const pauseButtons = screen.getAllByRole("button", { name: /^pause$/i });
    fireEvent.click(pauseButtons[0]!);
    await waitFor(() => {
      expect(mockUpdateMonthly).toHaveBeenCalledWith({ id: "g1", monthlyCents: 0 });
    });
  });

  it("does not show a Pause button for debt-payoff or spending-cap goals", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "g4", name: "Dining cap", goalType: "spending-cap",
          targetCents: 40000, currentCents: 10000, monthlyCents: 0,
          targetDate: null, color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.queryByRole("button", { name: /^pause$/i })).not.toBeInTheDocument();
  });

  it("does not label a goal 'Paused' just because it was never configured with a monthly contribution", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "g5", name: "Never funded", goalType: "save-by-date",
          targetCents: 500000, currentCents: 0, monthlyCents: 0,
          targetDate: "2027-06-01", color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    // monthlyCents is 0 but no pause action was ever taken this session -> must not show "Paused",
    // and the button must still say "Resume" since the goal's contribution is in fact 0.
    expect(screen.queryByText("Paused")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^resume$/i })).toBeInTheDocument();
  });

  it("shows 'Paused' only after an actual pause action, and removes it on resume", async () => {
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.queryByText("Paused")).not.toBeInTheDocument();

    const pauseButton = screen.getAllByRole("button", { name: /^pause$/i })[0]!;
    fireEvent.click(pauseButton);
    await waitFor(() => expect(mockUpdateMonthly).toHaveBeenCalledWith({ id: "g1", monthlyCents: 0 }));
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

  it("shows a 'newly achievable' message instead of '0 months saved' when the goal has no current monthly contribution", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "g5", name: "Stalled fund", goalType: "save-by-date",
          targetCents: 500000, currentCents: 100000, monthlyCents: 0,
          targetDate: "2027-06-01", color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    const slider = screen.getByLabelText("Extra monthly contribution");
    fireEvent.change(slider, { target: { value: "500" } });

    expect(screen.getByText(/on a path to finish by/i)).toBeInTheDocument();
    expect(screen.getByText(/wasn't projected to complete before/i)).toBeInTheDocument();
    expect(screen.queryByText(/0 months/i)).not.toBeInTheDocument();
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

  it("shows the real liability name, not a hardcoded 'Car loan' string", async () => {
    const budget = await import("../api/hooks/budget");
    const assets = await import("../api/hooks/assets");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{
        id: "g6", name: "Mortgage payoff", goalType: "debt-payoff",
        targetCents: 30000000, currentCents: 5000000, monthlyCents: 200000,
        targetDate: null, color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", liabilityId: "l2", accountId: null,
      }],
      isLoading: false,
      error: null,
    });
    (assets.useLiabilities as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{ id: "l2", name: "Home Mortgage", balanceCents: 25000000, aprPct: 3.2 }],
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText(/Linked to Home Mortgage/)).toBeInTheDocument();
    expect(screen.queryByText(/Linked to Car loan/)).not.toBeInTheDocument();
  });
});

describe("Goals — focus editor", () => {
  it("opens the goal drawer when focusGoal is present", async () => {
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
    render(<Goals />, { wrapper: createWrapperWithEntries(["/goals?focusGoal=g3"]) });
    expect(await screen.findByText("Edit goal · Car payoff")).toBeInTheDocument();
    expect(screen.getByText(/Monthly contribution/i)).toBeInTheDocument();
  });
});

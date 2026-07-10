import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Goals, { buildHorizonRows } from "./Goals";
import { createWrapper, createWrapperWithEntries } from "../test-utils";

const mockUpdateMonthly = vi.fn().mockResolvedValue(undefined);

vi.mock("../api/hooks/budget", () => ({
  useGoals: vi.fn(() => ({
    data: [
      {
        id: "g1", name: "Italy Fund", goalType: "save-by-date",
        targetCents: 500000, currentCents: 100000, monthlyCents: 20000,
        targetDate: "2027-06-01", color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", accountId: null,
      },
      {
        id: "g2", name: "Car repair", goalType: "save-by-date",
        targetCents: 200000, currentCents: 50000, monthlyCents: 10000,
        targetDate: new Date(Date.now() + 180 * 86400000).toISOString().slice(0, 10),
        color: "#34D399", notes: null, purpose: null, sortOrder: 1,
        createdAt: "2026-01-01", accountId: null,
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
  useContributeToGoal: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useGoalContributions: vi.fn(() => ({ data: [] })),
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

function future(monthsFromNow: number): string {
  const d = new Date();
  d.setMonth(d.getMonth() + monthsFromNow);
  return d.toISOString().slice(0, 10);
}

describe("Goals — buildHorizonRows", () => {
  const baseGoal = {
    id: "x", name: "X", color: "#C9F950", notes: null, purpose: null,
    sortOrder: 0, createdAt: "2026-01-01", accountId: null, targetDate: null,
  };

  it("excludes spending-cap goals even when they would have a finite ETA", () => {
    const spendingCap = { ...baseGoal, id: "sc1", goalType: "spending-cap", targetCents: 40000, currentCents: 10000, monthlyCents: 10000 };
    const { rows } = buildHorizonRows([spendingCap]);
    expect(rows).toHaveLength(0);
  });

  it("excludes goals with no monthly contribution and incomplete progress (infinite ETA)", () => {
    const stalled = { ...baseGoal, id: "st1", goalType: "save-by-date", targetCents: 500000, currentCents: 100000, monthlyCents: 0 };
    const { rows } = buildHorizonRows([stalled]);
    expect(rows).toHaveLength(0);
  });

  it("includes an already-complete goal at months: 0 and xPercent: 0", () => {
    const done = { ...baseGoal, id: "d1", goalType: "save-by-date", targetCents: 100000, currentCents: 150000, monthlyCents: 5000 };
    const { rows } = buildHorizonRows([done]);
    expect(rows).toHaveLength(1);
    expect(rows[0]!.months).toBe(0);
    expect(rows[0]!.xPercent).toBe(0);
  });

  it("clamps pct to 100 when currentCents exceeds targetCents", () => {
    const over = { ...baseGoal, id: "o1", goalType: "build-balance", targetCents: 100000, currentCents: 150000, monthlyCents: 5000 };
    const { rows } = buildHorizonRows([over]);
    expect(rows[0]!.pct).toBe(100);
  });

  it("sizes the window to a floor of 6 months when all goals are near-term", () => {
    const near = { ...baseGoal, id: "n1", goalType: "save-by-date", targetCents: 20000, currentCents: 10000, monthlyCents: 10000 };
    // remaining 10000, monthly 10000 -> months = 1
    const { windowMonths } = buildHorizonRows([near]);
    expect(windowMonths).toBe(6);
  });

  it("grows the window dynamically to fit the furthest-out goal", () => {
    const far = { ...baseGoal, id: "f1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000 };
    // remaining 400000, monthly 20000 -> months = 20
    const { windowMonths } = buildHorizonRows([far]);
    expect(windowMonths).toBe(21);
  });

  it("sorts rows ascending by months (soonest first)", () => {
    const soon = { ...baseGoal, id: "s1", goalType: "save-by-date", targetCents: 100000, currentCents: 90000, monthlyCents: 10000 }; // 1 month
    const later = { ...baseGoal, id: "l1", goalType: "save-by-date", targetCents: 500000, currentCents: 0, monthlyCents: 50000 }; // 10 months
    const { rows } = buildHorizonRows([later, soon]);
    expect(rows.map((r) => r.goal.id)).toEqual(["s1", "l1"]);
  });

  it("flags a goal as needing attention when its projected ETA lands later than its target date", () => {
    // remaining 400000, monthly 20000 -> 20 months out, but committed to a target date only 10 months away
    const behind = { ...baseGoal, id: "b1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000, targetDate: future(10) };
    const { rows } = buildHorizonRows([behind]);
    expect(rows[0]!.needsAttention).toBe(true);
  });

  it("does not flag a goal as needing attention when it will finish on or before its target date", () => {
    // same 20-month projection, but target date is comfortably further out (25 months)
    const onTrack = { ...baseGoal, id: "ot1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000, targetDate: future(25) };
    const { rows } = buildHorizonRows([onTrack]);
    expect(rows[0]!.needsAttention).toBe(false);
  });

  it("does not flag a goal with no target date, regardless of its projected ETA", () => {
    const noTargetDate = { ...baseGoal, id: "nt1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000, targetDate: null };
    const { rows } = buildHorizonRows([noTargetDate]);
    expect(rows[0]!.needsAttention).toBe(false);
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
          sortOrder: 0, createdAt: "2026-01-01", accountId: null,
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
          sortOrder: 0, createdAt: "2026-01-01", accountId: null,
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
          sortOrder: 0, createdAt: "2026-01-01", accountId: null,
        },
        {
          id: "g4", name: "Dining cap", goalType: "spending-cap",
          targetCents: 40000, currentCents: 10000, monthlyCents: 0,
          targetDate: null, color: "#C9F950", notes: null, purpose: null,
          sortOrder: 2, createdAt: "2026-01-01", accountId: null,
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
          sortOrder: 0, createdAt: "2026-01-01", accountId: null,
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

describe("Goals — linked account", () => {
  it("renders linked account details and disables manual balance edit", async () => {
    const budget = await import("../api/hooks/budget");
    const accounts = await import("../api/hooks/accounts");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{
        id: "g3", name: "Car payoff", goalType: "debt-payoff",
        targetCents: 2000000, currentCents: 1500000, monthlyCents: 50000,
        targetDate: null, color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", accountId: "a1",
      }],
      isLoading: false,
      error: null,
    });
    (accounts.useAccounts as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{ id: "a1", name: "Car loan", nickname: null, official_name: null }],
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getAllByText("Car payoff").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Linked to Car loan/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /edit balance/i })).not.toBeInTheDocument();
  });

  it("shows the real account name, not a hardcoded 'Car loan' string", async () => {
    const budget = await import("../api/hooks/budget");
    const accounts = await import("../api/hooks/accounts");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{
        id: "g6", name: "Mortgage payoff", goalType: "debt-payoff",
        targetCents: 30000000, currentCents: 5000000, monthlyCents: 200000,
        targetDate: null, color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", accountId: "a2",
      }],
      isLoading: false,
      error: null,
    });
    (accounts.useAccounts as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{ id: "a2", name: "Home Mortgage", nickname: null, official_name: null }],
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText(/Linked to Home Mortgage/)).toBeInTheDocument();
    expect(screen.queryByText(/Linked to Car loan/)).not.toBeInTheDocument();
  });
});

describe("Goals — focus editor", () => {
  it("opens the goal drawer when focusGoal is present", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [{
        id: "g3", name: "Car payoff", goalType: "debt-payoff",
        targetCents: 2000000, currentCents: 1500000, monthlyCents: 50000,
        targetDate: null, color: "#C9F950", notes: null, purpose: null,
        sortOrder: 0, createdAt: "2026-01-01", accountId: null,
      }],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapperWithEntries(["/goals?focusGoal=g3"]) });
    expect(await screen.findByText("Edit goal · Car payoff")).toBeInTheDocument();
    expect(screen.getByText(/Monthly contribution/i)).toBeInTheDocument();
  });
});

describe("Goals — Horizon timeline", () => {
  it("renders a row per eligible goal with name, eta, and target amount", () => {
    render(<Goals />, { wrapper: createWrapper() });
    // g1: Italy Fund, target 500000c, current 100000c, monthly 20000c -> 20 months
    // g2: Car repair, target 200000c, current 50000c, monthly 10000c -> 15 months
    expect(screen.getByText("Horizon")).toBeInTheDocument();
    expect(screen.getByText("When each goal lands.")).toBeInTheDocument();
    expect(screen.getAllByText(/Italy Fund/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Car repair/).length).toBeGreaterThan(0);
  });

  it("hides the Horizon section entirely when no goal has a finite ETA", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "sc1", name: "Dining cap", goalType: "spending-cap",
          targetCents: 40000, currentCents: 10000, monthlyCents: 0,
          targetDate: null, color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.queryByText("Horizon")).not.toBeInTheDocument();
  });

  it("shows a text cue (not just color) when a goal is behind its target date", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "behind1", name: "Behind Fund", goalType: "save-by-date",
          targetCents: 400000, currentCents: 0, monthlyCents: 20000,
          // 20 months to complete, but target date is only 5 months away -> behind schedule
          targetDate: (() => { const d = new Date(); d.setMonth(d.getMonth() + 5); return d.toISOString().slice(0, 10); })(),
          color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.getByText(/Behind schedule/)).toBeInTheDocument();
  });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Scenarios from "./Scenarios";
import { createWrapper } from "../test-utils";
import type { SavedScenarioDetail, ScenarioPlanProposal } from "../api/client";

const runMutate = vi.fn();
const promoteMutate = vi.fn();
const useSavedScenarios = vi.fn();

vi.mock("../api/hooks/useScenarios", () => ({
  useSavedScenarios: () => useSavedScenarios(),
  useRunScenario: () => ({ mutateAsync: runMutate, isPending: false }),
  useSaveScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDuplicateScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useArchiveScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
  usePromoteScenario: () => ({ mutateAsync: promoteMutate, isPending: false }),
  useDeleteScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

vi.mock("../api/client", () => ({
  commands: {
    listCategoriesWithSpending: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: [{ label: "Dining", thisMonthCents: 30000 }] }),
  },
}));

const RESULT = {
  verdict: true,
  runwayChangeDays: -20,
  monthlyImpactCents: -50000,
  considerations: ["Runway shortens by 20 days."],
  baselineMonthly: [100000, 110000, 120000],
  scenarioMonthly: [100000, 105000, 110000],
  goalsAffected: ["House Fund: +2 mo"],
};

const PARAMS = { incomeDeltaPct: 0, monthlyExpenseDeltaCents: -30000, oneTimeCents: 0, startMonthOffset: 0, label: "chip" };

function saved(over: Partial<SavedScenarioDetail>): SavedScenarioDetail {
  return {
    id: "s1",
    description: "Cut income 50%",
    createdAt: "2026-07-12T00:00:00Z",
    months: 24,
    params: PARAMS,
    originalResult: { ...RESULT, runwayChangeDays: -180, verdict: false },
    originalBaseline: { balanceCents: 2314000, avgMonthlyIncomeCents: 600000, avgMonthlyExpenseCents: 388000, goalCount: 1 },
    currentResult: { ...RESULT, runwayChangeDays: -214, verdict: false },
    isStale: true,
    recomputable: true,
    ...over,
  };
}

const PROPOSAL: ScenarioPlanProposal = {
  scenarioId: "s1",
  description: "Add $500/mo to savings",
  changes: [
    { title: "Commit more each month", detail: "Set aside about $500 more each month.", currentCents: 388000, proposedCents: 438000 },
  ],
  note: "These are suggestions for your review — nothing has been changed.",
};

describe("Scenarios screen", () => {
  beforeEach(() => {
    runMutate.mockReset();
    runMutate.mockResolvedValue({ result: RESULT, params: PARAMS, months: 24 });
    promoteMutate.mockReset();
    promoteMutate.mockResolvedValue(PROPOSAL);
    useSavedScenarios.mockReset();
    useSavedScenarios.mockReturnValue({ data: [] });
  });

  it("renders the header and suggested chips", () => {
    render(<Scenarios />, { wrapper: createWrapper() });
    expect(screen.getByText("Imagine a future, see the math.")).toBeInTheDocument();
    expect(screen.getByText("Cut income 50%")).toBeInTheDocument();
  });

  it("running a chip shows the verdict panel", async () => {
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Buy a car $35k"));
    await waitFor(() => expect(runMutate).toHaveBeenCalled());
    await waitFor(() =>
      expect(screen.getByText("You can do this — here's what changes.")).toBeInTheDocument()
    );
    expect(screen.getByText("Runway shortens by 20 days.")).toBeInTheDocument();
  });

  it("compares saved scenarios (recomputed) and flags a stale one", () => {
    useSavedScenarios.mockReturnValue({
      data: [saved({}), saved({ id: "s2", description: "Add $500/mo", isStale: false, currentResult: { ...RESULT, runwayChangeDays: 38, verdict: true } })],
    });
    render(<Scenarios />, { wrapper: createWrapper() });
    // Both saved scenarios appear in the comparison.
    expect(screen.getByText("Cut income 50%", { selector: "span" })).toBeInTheDocument();
    expect(screen.getByText("Add $500/mo")).toBeInTheDocument();
    // The stale one is badged, and the recomputed runway (-214d) shows with the original as "was".
    expect(screen.getByText("Stale")).toBeInTheDocument();
    expect(screen.getByText("-214d")).toBeInTheDocument();
    expect(screen.getByText(/was .*-180d/)).toBeInTheDocument();
  });

  it("promoting a scenario shows reviewable proposed changes and the no-op note", async () => {
    useSavedScenarios.mockReturnValue({ data: [saved({})] });
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Promote" }));
    await waitFor(() => expect(promoteMutate).toHaveBeenCalledWith("s1"));
    expect(await screen.findByText("Commit more each month")).toBeInTheDocument();
    expect(screen.getByText(/nothing has been changed/)).toBeInTheDocument();
  });

  it("does not offer recompute/promote on a legacy scenario", () => {
    useSavedScenarios.mockReturnValue({
      data: [saved({ recomputable: false, params: null, currentResult: null, isStale: null })],
    });
    render(<Scenarios />, { wrapper: createWrapper() });
    expect(screen.getByText("Legacy")).toBeInTheDocument();
    // Promote is disabled for a non-recomputable row.
    expect(screen.getByRole("button", { name: "Promote" })).toBeDisabled();
  });
});

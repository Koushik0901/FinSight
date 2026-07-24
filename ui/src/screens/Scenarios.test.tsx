import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Scenarios from "./Scenarios";
import { createWrapper } from "../test-utils";
import type { SavedScenarioDetail, ScenarioPlanProposal } from "../api/client";

const runMutate = vi.fn();
const promoteMutate = vi.fn();
const reviseMutate = vi.fn();
const applyMutate = vi.fn();
const useSavedScenarios = vi.fn();
const scenarioExplanation = vi.fn();

vi.mock("../api/hooks/useScenarios", () => ({
  useSavedScenarios: () => useSavedScenarios(),
  useRunScenario: () => ({ mutateAsync: runMutate, isPending: false }),
  useSaveScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDuplicateScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useArchiveScenario: () => ({ mutateAsync: vi.fn(), isPending: false }),
  usePromoteScenario: () => ({ mutateAsync: promoteMutate, isPending: false }),
  useApplyScenario: () => ({ mutateAsync: applyMutate, isPending: false }),
  useReviseScenario: () => ({ mutateAsync: reviseMutate, isPending: false }),
  useClearScenarioRevision: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useScenarioExplanation: (id: string | null) => scenarioExplanation(id),
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
    revisedParams: null,
    revisedResult: null,
    ...over,
  };
}

const PROPOSAL: ScenarioPlanProposal = {
  scenarioId: "s1",
  description: "Buy a car $35k",
  changes: [
    { id: "expense", title: "Commit more each month", detail: "Set aside about $500 more each month.", currentCents: 388000, proposedCents: 438000, applyable: false },
    { id: "one_time", title: "Set aside for a one-time amount", detail: "Plan for a one-off of about $35,000. Applying adds it as a planned transaction.", currentCents: null, proposedCents: 3500000, applyable: true },
  ],
  note: "These are suggestions for your review — nothing has been changed.",
};

describe("Scenarios screen", () => {
  beforeEach(() => {
    runMutate.mockReset();
    runMutate.mockResolvedValue({ result: RESULT, params: PARAMS, months: 24 });
    promoteMutate.mockReset();
    promoteMutate.mockResolvedValue(PROPOSAL);
    reviseMutate.mockReset();
    reviseMutate.mockResolvedValue(saved({}));
    applyMutate.mockReset();
    applyMutate.mockResolvedValue({ applied: ["one_time"], skipped: [], note: "Applied 1 change(s) to your plan as planned transactions. The scenario is unchanged." });
    useSavedScenarios.mockReset();
    useSavedScenarios.mockReturnValue({ data: [] });
    scenarioExplanation.mockReset();
    scenarioExplanation.mockReturnValue({
      data: {
        key: "scenario",
        label: "Cut income 50%",
        value: { kind: "money", cents: -50000 },
        definition: "How this changes your finances, projected over 24 months.",
        inputs: [{ label: "Starting balance", amountCents: 2314000, detail: null }],
        exclusions: [],
        assumptions: [{ label: "Projection horizon", value: "24 months" }],
        tradeoffs: ["Runway shortens by 20 days — from 3.2 years to 3.1 years."],
        period: "Projected from today",
        warnings: [],
      },
      isLoading: false,
    });
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

  it("promoting a scenario shows changes split into applyable vs recommendation-only", async () => {
    useSavedScenarios.mockReturnValue({ data: [saved({})] });
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Promote" }));
    await waitFor(() => expect(promoteMutate).toHaveBeenCalledWith("s1"));
    expect(await screen.findByText("Commit more each month")).toBeInTheDocument();
    // The aggregate change is recommendation-only; the one-time is applyable.
    expect(screen.getByText("Recommendation")).toBeInTheDocument();
    expect(screen.getByText("Applyable")).toBeInTheDocument();
  });

  it("applies only the approved applyable changes to the plan and reports the outcome (#72)", async () => {
    useSavedScenarios.mockReturnValue({ data: [saved({})] });
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Promote" }));
    // The applyable one-time is pre-approved; apply writes it.
    fireEvent.click(await screen.findByRole("button", { name: /Apply 1 to plan/i }));
    await waitFor(() =>
      expect(applyMutate).toHaveBeenCalledWith({ id: "s1", approvedChangeIds: ["one_time"] }),
    );
    // The result summary surfaces what was written; the scenario is unchanged.
    expect(await screen.findByText(/Applied 1 change/i)).toBeInTheDocument();
  });

  it("cannot re-apply a change already written to the plan (#72 no double-write)", async () => {
    useSavedScenarios.mockReturnValue({ data: [saved({})] });
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Promote" }));
    fireEvent.click(await screen.findByRole("button", { name: /Apply 1 to plan/i }));
    await screen.findByText(/Applied 1 change/i);
    // The applied change is now marked and the button drops to zero, disabled —
    // a second click can't write a duplicate planned transaction.
    expect(screen.getByText("Applied")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Apply 0 to plan/i })).toBeDisabled();
    expect(applyMutate).toHaveBeenCalledTimes(1);
  });

  it("re-initialises the approval selection when a different scenario is promoted (#72)", async () => {
    // Scenario A promotes to a recommendation-only proposal; B has an applyable change.
    useSavedScenarios.mockReturnValue({ data: [saved({ id: "s1", description: "A" }), saved({ id: "s2", description: "B" })] });
    promoteMutate.mockImplementation((id: string) =>
      id === "s2"
        ? Promise.resolve({ ...PROPOSAL, scenarioId: "s2", description: "B" })
        : Promise.resolve({ scenarioId: "s1", description: "A", changes: [{ id: "rec", title: "Handle it yourself", detail: "", currentCents: null, proposedCents: null, applyable: false }], note: "Nothing to apply automatically." }),
    );
    render(<Scenarios />, { wrapper: createWrapper() });
    // Promote A (recommendation-only), then B without closing the panel.
    fireEvent.click(screen.getAllByRole("button", { name: "Promote" })[0]!);
    await screen.findByText("Handle it yourself");
    fireEvent.click(screen.getAllByRole("button", { name: "Promote" })[1]!);
    // B's applyable change must be freshly pre-selected — not stuck on A's empty set.
    const applyBtn = await screen.findByRole("button", { name: /Apply 1 to plan/i });
    expect(applyBtn).toBeEnabled();
  });

  it("explains a saved scenario in the inspector, with its tradeoffs (#71)", async () => {
    useSavedScenarios.mockReturnValue({ data: [saved({})] });
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Explain" }));
    // The drawer opens with the structured explanation: its definition and the
    // engine's own considerations, surfaced as tradeoffs.
    expect(await screen.findByText(/projected over 24 months/i)).toBeInTheDocument();
    expect(screen.getByText("Tradeoffs")).toBeInTheDocument();
    expect(screen.getByText(/Runway shortens by 20 days/)).toBeInTheDocument();
  });

  it("revising a scenario re-evaluates the new assumptions without touching the plan (#73)", async () => {
    useSavedScenarios.mockReturnValue({ data: [saved({})] });
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: "Revise" }));
    // The panel opens, seeded with the scenario's params, and states it's non-destructive.
    expect(await screen.findByText(/new assumptions/i)).toBeInTheDocument();
    expect(screen.getByText(/never changes your budgets, goals, or plan/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Re-evaluate" }));
    await waitFor(() => expect(reviseMutate).toHaveBeenCalledWith(expect.objectContaining({ id: "s1", params: expect.objectContaining({ monthlyExpenseDeltaCents: -30000 }) })));
  });

  it("re-seeds the revise inputs when switching to a different scenario (#73)", async () => {
    useSavedScenarios.mockReturnValue({ data: [
      saved({ id: "s1", description: "A", params: { incomeDeltaPct: -50, monthlyExpenseDeltaCents: 0, oneTimeCents: 0, startMonthOffset: 0, label: "A" } }),
      saved({ id: "s2", description: "B", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 0, oneTimeCents: 3500000, startMonthOffset: 0, label: "B" } }),
    ] });
    render(<Scenarios />, { wrapper: createWrapper() });
    // Revise A: income field seeded to -50.
    fireEvent.click(screen.getAllByRole("button", { name: "Revise" })[0]!);
    expect(await screen.findByDisplayValue("-50")).toBeInTheDocument();
    // Switch to B without closing: its one-time ($35k) must seed fresh, and A's
    // -50 must be gone — otherwise a re-evaluate would apply A's assumptions to B.
    fireEvent.click(screen.getAllByRole("button", { name: "Revise" })[1]!);
    expect(await screen.findByDisplayValue("35000")).toBeInTheDocument();
    expect(screen.queryByDisplayValue("-50")).not.toBeInTheDocument();
  });

  it("marks a scenario that carries a revision and shows the revised-vs-original comparison", () => {
    const revised = saved({
      revisedParams: { incomeDeltaPct: -50, monthlyExpenseDeltaCents: 0, oneTimeCents: 0, startMonthOffset: 0, label: "r" },
      revisedResult: { ...RESULT, runwayChangeDays: -300, verdict: false },
    });
    useSavedScenarios.mockReturnValue({ data: [revised] });
    render(<Scenarios />, { wrapper: createWrapper() });
    expect(screen.getByText("Revised")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Revise" }));
    // The revised result is shown against the original assumptions' result.
    expect(screen.getByText("-300d")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Discard revision" })).toBeInTheDocument();
    // The edit itself is legible: income was 0% before the revision to -50%.
    expect(screen.getByText(/was 0%/)).toBeInTheDocument();
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

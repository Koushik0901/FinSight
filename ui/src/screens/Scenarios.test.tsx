import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Scenarios from "./Scenarios";
import { createWrapper } from "../test-utils";

const runMutate = vi.fn();

vi.mock("../api/hooks/useScenarios", () => ({
  useScenarioHistory: vi.fn(() => ({ data: [] })),
  useRunScenario: vi.fn(() => ({ mutateAsync: runMutate, isPending: false })),
  useSaveScenario: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteScenario: vi.fn(() => ({ mutateAsync: vi.fn() })),
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

describe("Scenarios screen", () => {
  beforeEach(() => {
    runMutate.mockReset();
    runMutate.mockResolvedValue(RESULT);
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
});

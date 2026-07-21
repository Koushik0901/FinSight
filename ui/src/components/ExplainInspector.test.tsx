import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import ExplainInspector from "./ExplainInspector";
import type { MetricExplanation } from "../api/client";

// Controllable hook mock: the inspector reads explanations by key from here.
const useMetricExplanations = vi.fn();
vi.mock("../api/hooks/metrics", () => ({
  useMetricExplanations: (memberId?: string | null) => useMetricExplanations(memberId),
}));

const SAVINGS_RATE: MetricExplanation = {
  key: "savings_rate",
  label: "Savings rate",
  value: { kind: "percent", pct: 30 },
  definition: "The share of your income you keep: (income − spending) ÷ income, over the window.",
  inputs: [
    { label: "Average monthly income", amountCents: 500000, detail: null },
    { label: "Average monthly spending", amountCents: 350000, detail: null },
  ],
  exclusions: ["Transfers and investment-account activity."],
  assumptions: [{ label: "Your target savings rate", value: "20%" }],
  period: "Trailing 90 days",
  warnings: [
    { level: "caution", message: "Only 22 days of history so far — this monthly average is extrapolated from a partial month." },
  ],
};

const RUNWAY_WITHHELD: MetricExplanation = {
  key: "runway_days",
  label: "Cash runway",
  value: { kind: "withheld" },
  definition: "How long your liquid cash would last with no new income, at your typical spending.",
  inputs: [
    { label: "Liquid cash", amountCents: 420000, detail: null },
    { label: "Conservative monthly spending", amountCents: 260000, detail: "the larger of your 12-month and 90-day average" },
  ],
  exclusions: [],
  assumptions: [],
  period: "As of today, at your conservative monthly spending",
  warnings: [
    { level: "withheld", message: "Withheld until there's about 30 days of history — currently 8." },
  ],
};

function mockData(...metrics: MetricExplanation[]) {
  useMetricExplanations.mockReturnValue({
    data: Object.fromEntries(metrics.map((m) => [m.key, m])),
    isLoading: false,
  });
}

describe("ExplainInspector", () => {
  beforeEach(() => {
    useMetricExplanations.mockReset();
  });

  it("renders a stated metric with its definition, inputs, exclusions, assumptions, period and warnings", () => {
    mockData(SAVINGS_RATE);
    render(<ExplainInspector metricKey="savings_rate" currency="USD" onClose={() => {}} />);

    // Title + definition
    expect(screen.getByRole("heading", { name: "Savings rate" })).toBeInTheDocument();
    expect(screen.getByText(/share of your income you keep/)).toBeInTheDocument();
    // The percent value is shown (not withheld)
    expect(screen.getByText("30%")).toBeInTheDocument();
    // Inputs, an exclusion, an assumption, the period, and the caution warning
    expect(screen.getByText("Average monthly income")).toBeInTheDocument();
    expect(screen.getByText("Average monthly spending")).toBeInTheDocument();
    expect(screen.getByText(/Transfers and investment-account activity/)).toBeInTheDocument();
    expect(screen.getByText("Your target savings rate")).toBeInTheDocument();
    expect(screen.getByText("Trailing 90 days")).toBeInTheDocument();
    expect(screen.getByText(/extrapolated from a partial month/)).toBeInTheDocument();
    // No fabricated "not shown" state for a stated figure.
    expect(screen.queryByText("Not shown yet")).not.toBeInTheDocument();
  });

  it("shows the reason instead of a fabricated number when a figure is withheld", () => {
    mockData(RUNWAY_WITHHELD);
    render(<ExplainInspector metricKey="runway_days" currency="USD" onClose={() => {}} />);

    expect(screen.getByText("Not shown yet")).toBeInTheDocument();
    expect(screen.getByText(/Withheld until there's about 30 days of history/)).toBeInTheDocument();
    // Inputs still explain WHAT would feed the number.
    expect(screen.getByText("What would feed it")).toBeInTheDocument();
    expect(screen.getByText("Liquid cash")).toBeInTheDocument();
    // The per-input detail hint is surfaced.
    expect(screen.getByText(/larger of your 12-month and 90-day average/)).toBeInTheDocument();
  });

  it("renders nothing when closed (metricKey is null)", () => {
    mockData(SAVINGS_RATE);
    render(<ExplainInspector metricKey={null} currency="USD" onClose={() => {}} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("degrades gracefully when the requested metric is absent", () => {
    mockData(SAVINGS_RATE);
    render(<ExplainInspector metricKey="not_a_metric" currency="USD" onClose={() => {}} />);
    // Drawer opens (a key was requested) but shows a safe fallback, no crash.
    expect(screen.getByText(/No explanation is available/)).toBeInTheDocument();
  });
});

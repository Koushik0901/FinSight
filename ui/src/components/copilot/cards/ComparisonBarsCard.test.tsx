import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ComparisonBarsCard } from "./ComparisonBarsCard";

describe("ComparisonBarsCard", () => {
  it("delegates to FinSightBarComparison with the block's title/current/prior, only once streaming has finished", () => {
    render(
      <ComparisonBarsCard
        isRunning={false}
        block={{
          kind: "comparisonBars",
          title: "Dining · this month vs average",
          current: { label: "May 2026", amountCents: 41_200 },
          prior: { label: "12-mo avg", amountCents: 36_500 },
        }}
      />
    );
    expect(screen.getByText("Dining · this month vs average")).toBeInTheDocument();
    expect(screen.getByText("May 2026")).toBeInTheDocument();
    expect(screen.getByText("12-mo avg")).toBeInTheDocument();
    expect(screen.getByText(/\$412/)).toBeInTheDocument();
    expect(screen.getByText(/\$365/)).toBeInTheDocument();
  });

  it("shows a lightweight placeholder instead of mounting the chart while the message is still streaming", () => {
    render(
      <ComparisonBarsCard
        isRunning
        block={{
          kind: "comparisonBars",
          title: "Dining",
          current: { label: "May", amountCents: 100 },
          prior: { label: "Apr", amountCents: 80 },
        }}
      />
    );
    expect(screen.getByText("Dining")).toBeInTheDocument();
    expect(screen.queryByText("May")).not.toBeInTheDocument();
    expect(screen.queryByText(/\$1/)).not.toBeInTheDocument();
  });
});

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FinSightBarComparison } from "./FinSightChart";

describe("FinSightBarComparison", () => {
  it("renders both labeled bars with formatted currency values", () => {
    render(
      <FinSightBarComparison
        title="Dining · this month vs average"
        current={{ label: "May 2026", amountCents: 41200 }}
        prior={{ label: "12-mo avg", amountCents: 36500 }}
      />
    );
    expect(screen.getByText("Dining · this month vs average")).toBeInTheDocument();
    expect(screen.getByText("May 2026")).toBeInTheDocument();
    expect(screen.getByText("12-mo avg")).toBeInTheDocument();
    expect(screen.getByText("$412")).toBeInTheDocument();
    expect(screen.getByText("$365")).toBeInTheDocument();
  });

  it("shows an empty state instead of a chart when both values are zero", () => {
    render(
      <FinSightBarComparison
        title="No data"
        current={{ label: "This month", amountCents: 0 }}
        prior={{ label: "Last month", amountCents: 0 }}
      />
    );
    expect(screen.getByText(/no comparison data/i)).toBeInTheDocument();
  });
});

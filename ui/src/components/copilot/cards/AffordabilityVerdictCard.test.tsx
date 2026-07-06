import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { AffordabilityVerdictCard } from "./AffordabilityVerdictCard";

describe("AffordabilityVerdictCard", () => {
  it("renders the headline, sub, caveat, and funding source", () => {
    render(
      <AffordabilityVerdictCard
        block={{
          kind: "affordabilityVerdict",
          canAfford: true,
          headline: "Yes",
          sub: "$540 · about 1% of liquid cash · 0 goals affected",
          caveat: "Exceeds your May Shopping envelope by $426.",
          fundingSource: { label: "Cover it from Travel", detail: "$500 budgeted · $0 spent" },
        }}
      />
    );
    expect(screen.getByText("Yes")).toBeInTheDocument();
    expect(screen.getByText(/1% of liquid cash/)).toBeInTheDocument();
    expect(screen.getByText(/Exceeds your May Shopping envelope/)).toBeInTheDocument();
    expect(screen.getByText("Cover it from Travel")).toBeInTheDocument();
  });

  it("omits the caveat and funding rows when absent", () => {
    render(
      <AffordabilityVerdictCard
        block={{ kind: "affordabilityVerdict", canAfford: false, headline: "No", sub: "Not enough liquid cash", caveat: null, fundingSource: null }}
      />
    );
    expect(screen.getByText("No")).toBeInTheDocument();
    expect(screen.queryByText(/Cover it from/)).not.toBeInTheDocument();
  });
});

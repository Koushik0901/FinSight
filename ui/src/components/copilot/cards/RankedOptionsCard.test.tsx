import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { RankedOptionsCard } from "./RankedOptionsCard";

describe("RankedOptionsCard", () => {
  it("renders the title and each option's verdict tone, label, detail, and rationale", () => {
    render(
      <RankedOptionsCard
        block={{
          kind: "rankedOptions",
          title: "The three routes you asked about",
          options: [
            { rankTone: "primary", label: "Pay off the loan", detail: "$2,418 → Amex Gold", rationale: "Highest-interest debt at 24.9%." },
            { rankTone: "muted", label: "Save for a car", detail: "no active goal", rationale: "Finish the emergency fund first." },
          ],
        }}
      />
    );
    expect(screen.getByText("The three routes you asked about")).toBeInTheDocument();
    expect(screen.getByText("Pay off the loan")).toBeInTheDocument();
    expect(screen.getByText("$2,418 → Amex Gold")).toBeInTheDocument();
    expect(screen.getByText("Do this first")).toBeInTheDocument();
    expect(screen.getByText("Not yet")).toBeInTheDocument();
  });

  it("applies the correct CSS tone class per option (primary/neutral/muted)", () => {
    render(
      <RankedOptionsCard
        block={{
          kind: "rankedOptions",
          title: "Options",
          options: [
            { rankTone: "primary", label: "A", detail: "a", rationale: "a" },
            { rankTone: "neutral", label: "B", detail: "b", rationale: "b" },
            { rankTone: "muted", label: "C", detail: "c", rationale: "c" },
          ],
        }}
      />
    );
    expect(screen.getByText("Do this first").className).toContain("cp-verdict-primary");
    expect(screen.getByText("With what's left").className).toContain("cp-verdict-neutral");
    expect(screen.getByText("Not yet").className).toContain("cp-verdict-muted");
  });
});

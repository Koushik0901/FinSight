import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { AllocationSplitCard } from "./AllocationSplitCard";

describe("AllocationSplitCard", () => {
  it("renders the total, each segment's label/rationale/amount, and a proportional bar", () => {
    render(
      <AllocationSplitCard
        block={{
          kind: "allocationSplit",
          totalCents: 520_000,
          segments: [
            { label: "Pay off Amex", amountCents: 241_800, rationale: "24.9% APR — guaranteed return", categoryKey: "debt" },
            { label: "Emergency fund", amountCents: 180_000, rationale: "76% ➜ 83% of target", categoryKey: "savings" },
          ],
        }}
      />
    );
    expect(screen.getByText(/Recommended split of/)).toBeInTheDocument();
    expect(screen.getByText("$5,200")).toBeInTheDocument();
    expect(screen.getByText("Pay off Amex")).toBeInTheDocument();
    expect(screen.getByText("$2,418")).toBeInTheDocument();
    expect(screen.getByText("24.9% APR — guaranteed return")).toBeInTheDocument();
  });
});

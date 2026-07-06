import { describe, it, expect } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { FinSightBarComparison } from "./FinSightChart";

describe("FinSightBarComparison mid-stream stability", () => {
  it("still renders final labeled values correctly after several rapid re-renders (simulated streaming reflow)", () => {
    const { rerender } = render(
      <FinSightBarComparison
        title="Dining"
        current={{ label: "May", amountCents: 10000 }}
        prior={{ label: "Apr", amountCents: 8000 }}
      />
    );
    // Simulate the parent message bubble reflowing repeatedly while text streams in.
    for (let i = 0; i < 10; i++) {
      act(() => {
        rerender(
          <FinSightBarComparison
            title="Dining"
            current={{ label: "May", amountCents: 10000 }}
            prior={{ label: "Apr", amountCents: 8000 }}
          />
        );
      });
    }
    expect(screen.getByText("May")).toBeInTheDocument();
    expect(screen.getByText("Apr")).toBeInTheDocument();
    expect(screen.getByText(/\$100/)).toBeInTheDocument();
    expect(screen.getByText(/\$80/)).toBeInTheDocument();
  });
});

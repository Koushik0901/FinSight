import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { BudgetEnvelopeChip } from "./Budget";

describe("BudgetEnvelopeChip", () => {
  it("over-budget chip shows signed money via moneyDisplay", () => {
    render(<BudgetEnvelopeChip remaining={-1200} />);
    expect(screen.getByText(/Over by/)).toHaveTextContent("Over by -$12.00");
    expect(screen.getByText(/Over by/).classList.contains("money")).toBe(true);
  });

  it("left chip shows signed money", () => {
    render(<BudgetEnvelopeChip remaining={1200} />);
    expect(screen.getByText(/Left/)).toHaveTextContent("Left $12.00");
    expect(screen.getByText(/Left/).classList.contains("money")).toBe(true);
  });
});

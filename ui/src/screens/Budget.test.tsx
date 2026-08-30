import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { BudgetEnvelope } from "./Budget";

describe("BudgetEnvelope", () => {
  it("over-budget chip shows signed money via moneyDisplay", () => {
    render(<BudgetEnvelope remaining={-1200} />);
    expect(screen.getByText(/Over by/)).toHaveTextContent("Over by -$12.00");
    expect(screen.getByText(/Over by/).classList.contains("money")).toBe(true);
  });

  it("left chip shows signed money", () => {
    render(<BudgetEnvelope remaining={1200} />);
    expect(screen.getByText(/Left/)).toHaveTextContent("Left $12.00");
    expect(screen.getByText(/Left/).classList.contains("money")).toBe(true);
  });
});

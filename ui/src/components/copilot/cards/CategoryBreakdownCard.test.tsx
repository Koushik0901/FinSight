import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CategoryBreakdownCard } from "./CategoryBreakdownCard";

describe("CategoryBreakdownCard", () => {
  it("renders each row's category and amount, tagging fixed and lever rows", () => {
    render(
      <CategoryBreakdownCard
        block={{
          kind: "categoryBreakdown",
          periodLabel: "May",
          rows: [
            { categoryKey: "Housing", amountCents: 185_000, isFixed: true, isLever: false },
            { categoryKey: "Dining", amountCents: 41_200, isFixed: false, isLever: true },
          ],
        }}
      />
    );
    expect(screen.getByText("Housing")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    expect(screen.getByText("fixed")).toBeInTheDocument();
    expect(screen.getByText("lever")).toBeInTheDocument();
  });
});

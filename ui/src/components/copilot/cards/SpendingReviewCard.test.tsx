import { render, screen } from "@testing-library/react";
import { SpendingReviewCard } from "./SpendingReviewCard";

const block = {
  kind: "spendingReview" as const,
  months: [
    {
      label: "May 2026",
      spentCents: 408600,
      subtitle: "8 of 10 envelopes under",
      categories: [
        { label: "Housing", amountCents: 185000, tag: "fixed" as const },
        { label: "Dining", amountCents: 41200, tag: "over" as const },
      ],
      summary: "A steady month.",
      actions: ["Glance at the PG&E bill"],
    },
  ],
};

test("renders month header, category bars, summary, and action plan", () => {
  render(<SpendingReviewCard block={block} />);
  expect(screen.getByText("May 2026")).toBeInTheDocument();
  expect(screen.getByText(/8 of 10 envelopes under/)).toBeInTheDocument();
  expect(screen.getByText("Housing")).toBeInTheDocument();
  expect(screen.getByText("Dining")).toBeInTheDocument();
  expect(screen.getByText("A steady month.")).toBeInTheDocument();
  expect(screen.getByText("Glance at the PG&E bill")).toBeInTheDocument();
});

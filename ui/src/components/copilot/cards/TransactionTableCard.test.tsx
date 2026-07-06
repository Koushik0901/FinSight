import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { TransactionTableCard } from "./TransactionTableCard";

describe("TransactionTableCard", () => {
  it("renders row merchant, category, and formatted amount, plus a more-count footer", () => {
    render(
      <TransactionTableCard
        block={{
          kind: "transactionTable",
          count: 42,
          totalCents: 1_193_000,
          rows: [
            { date: "2026-05-03", merchant: "Bay Property · Rent", categoryKey: "Housing", amountCents: 185_000, flag: null },
            { date: "2026-05-10", merchant: "PG&E", categoryKey: "Utilities", amountCents: 22_000, flag: "2.1× avg" },
          ],
          more: 40,
        }}
      />
    );
    expect(screen.getByText("42 transactions")).toBeInTheDocument();
    expect(screen.getByText("Bay Property · Rent")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    expect(screen.getByText("2.1× avg")).toBeInTheDocument();
    expect(screen.getByText(/\+ 40 more/)).toBeInTheDocument();
  });
});

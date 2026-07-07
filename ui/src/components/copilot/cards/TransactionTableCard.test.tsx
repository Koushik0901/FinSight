import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TransactionTableCard } from "./TransactionTableCard";

vi.mock("../../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../api/client")>();
  return {
    ...actual,
    commands: {
      exportSearchTransactionsCsv: vi.fn().mockResolvedValue({ status: "ok", data: "C:/tmp/transactions.csv" }),
    },
  };
});

import { commands } from "../../../api/client";

describe("TransactionTableCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

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

  it("exports CSV with the original tool-call args mapped to camelCase params", async () => {
    render(
      <TransactionTableCard
        block={{
          kind: "transactionTable",
          count: 3,
          totalCents: 12_000,
          rows: [
            { date: "2026-05-03", merchant: "Costco", categoryKey: "Groceries", amountCents: 9_999, flag: null },
          ],
          more: 2,
        }}
        toolArgs={{
          account: "amex",
          min_amount_cents: 6000,
          direction: "expense",
          start_date: "2026-01-01",
        }}
      />
    );

    const btn = screen.getByRole("button", { name: /Export 3 as CSV/ });
    fireEvent.click(btn);

    await waitFor(() => {
      expect(commands.exportSearchTransactionsCsv).toHaveBeenCalledWith({
        merchant: null,
        account: "amex",
        startDate: "2026-01-01",
        endDate: null,
        minAmountCents: 6000,
        direction: "expense",
      });
    });
  });
});

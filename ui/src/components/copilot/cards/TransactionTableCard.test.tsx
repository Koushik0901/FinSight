import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TransactionTableCard } from "./TransactionTableCard";

vi.mock("../../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../api/client")>();
  return {
    ...actual,
    commands: {
      exportSearchTransactionsCsv: vi.fn().mockResolvedValue({ status: "ok", data: "date,amount\n2026-01-01,10.00\n" }),
    },
  };
});

vi.mock("../../../lib/downloadBlob", () => ({
  downloadBlob: vi.fn(),
}));

import { commands } from "../../../api/client";
import { downloadBlob } from "../../../lib/downloadBlob";

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
          query: null,
        }}
      />
    );
    expect(screen.getByText("42 transactions")).toBeInTheDocument();
    expect(screen.getByText("Bay Property · Rent")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    expect(screen.getByText("2.1× avg")).toBeInTheDocument();
    expect(screen.getByText(/\+ 40 more/)).toBeInTheDocument();
  });

  it("does not offer an export when the block carries no query", () => {
    render(
      <TransactionTableCard
        block={{
          kind: "transactionTable",
          count: 3,
          totalCents: 12_000,
          rows: [{ date: "2026-05-03", merchant: "Costco", categoryKey: "Groceries", amountCents: 9_999, flag: null }],
          more: 2,
          query: null,
        }}
      />
    );
    expect(screen.queryByRole("button", { name: /Export/ })).not.toBeInTheDocument();
  });

  it("exports CSV with the block's own captured query, re-running the exact search", async () => {
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
          query: {
            merchant: null,
            account: "amex",
            startDate: "2026-01-01",
            endDate: null,
            minAmountCents: 6000,
            direction: "expense",
          },
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
    await waitFor(() => {
      expect(downloadBlob).toHaveBeenCalledWith("date,amount\n2026-01-01,10.00\n", "text/csv", "transactions.csv");
    });
  });
});

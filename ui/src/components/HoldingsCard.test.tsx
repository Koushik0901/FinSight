import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import HoldingsCard from "./HoldingsCard";
import type { AccountSummary } from "../api/client";

vi.mock("../api/client", () => ({
  commands: {
    listAccountPositions: vi.fn().mockResolvedValue({
      status: "ok",
      data: [
        {
          symbol: "GLOBEX",
          name: "Globex Corp",
          quantity: 12.4567,
          lastPrice: 55.32,
          lastTradeAt: "2026-07-08T12:00:00Z",
          marketValueCents: 68_900,
          investedCents: 62_000,
        },
        {
          symbol: "INITECH",
          name: "Initech Inc",
          quantity: 30.891,
          lastPrice: 22.15,
          lastTradeAt: "2026-07-03T12:00:00Z",
          marketValueCents: 68_423,
          investedCents: 60_000,
        },
      ],
    }),
    getInvestmentSummary: vi.fn().mockResolvedValue({
      status: "ok",
      data: {
        cashCents: 55_000,
        positionsValueCents: 137_323,
        portfolioEstimateCents: 192_323,
        dividendIncomeCents: 6_500,
        interestIncomeCents: 25,
        withholdingTaxCents: 300,
        openPositions: 2,
        hasNegativeQuantity: false,
      },
    }),
    setAccountBalance: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

const account = {
  id: "inv1",
  bank: "Wealthsimple",
  name: "TFSA",
  type: "Investment",
  currency: "CAD",
} as unknown as AccountSummary;

function wrap(children: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe("HoldingsCard", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders positions, income chips, and the portfolio estimate", async () => {
    render(wrap(<HoldingsCard account={account} />));
    await waitFor(() => expect(screen.getByText("GLOBEX")).toBeInTheDocument());

    expect(screen.getByText("INITECH")).toBeInTheDocument();
    expect(screen.getByText("12.4567")).toBeInTheDocument();
    expect(screen.getByText(/Dividends/)).toBeInTheDocument();
    expect(screen.getByText(/Interest/)).toBeInTheDocument();
    expect(screen.getByText(/Withholding tax/)).toBeInTheDocument();
    // CA$1,923.23 estimate (55000 cash + 137323 positions).
    expect(screen.getByText(/1,923\.23/)).toBeInTheDocument();
    expect(screen.queryByText(/may be incomplete/)).not.toBeInTheDocument();
  });

  it("Set balance from estimate calls setAccountBalance with the estimate", async () => {
    const { commands } = await import("../api/client");
    render(wrap(<HoldingsCard account={account} />));
    await waitFor(() => expect(screen.getByText("GLOBEX")).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: /set balance from estimate/i }));
    await waitFor(() =>
      expect(commands.setAccountBalance).toHaveBeenCalledWith("inv1", 192_323),
    );
  });

  it("warns when positions carry a negative quantity (partial history)", async () => {
    const { commands } = await import("../api/client");
    (commands.getInvestmentSummary as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: {
        cashCents: 21_000,
        positionsValueCents: -21_000,
        portfolioEstimateCents: 0,
        dividendIncomeCents: 0,
        interestIncomeCents: 0,
        withholdingTaxCents: 0,
        openPositions: 1,
        hasNegativeQuantity: true,
      },
    });
    render(wrap(<HoldingsCard account={account} />));
    await waitFor(() =>
      expect(screen.getByText(/Positions may be incomplete/)).toBeInTheDocument(),
    );
  });
});

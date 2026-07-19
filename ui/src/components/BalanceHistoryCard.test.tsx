import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import BalanceHistoryCard from "./BalanceHistoryCard";
import type { AccountSummary } from "../api/client";

const getAccountBalanceTimeline = vi.fn();
vi.mock("../api/client", () => ({
  commands: {
    getAccountBalanceTimeline: (...args: unknown[]) => getAccountBalanceTimeline(...args),
  },
}));

const account = { id: "sav1", name: "Car Savings", type: "Savings" } as unknown as AccountSummary;

function timeline(over: Record<string, unknown> = {}) {
  return {
    status: "ok",
    data: {
      accountId: "sav1",
      accountName: "Car Savings",
      points: [
        { date: "2024-01-31", balanceCents: 100_000 },
        { date: "2024-02-01", balanceCents: 600_000 },
        { date: "2024-05-01", balanceCents: 1_000_000 },
        { date: "2024-08-01", balanceCents: 300_000 },
      ],
      peak: { date: "2024-05-01", balanceCents: 1_000_000 },
      trough: { date: "2024-01-31", balanceCents: 100_000 },
      currentCents: 300_000,
      anchor: "anchoredOpening",
      earliestTxnDate: "2024-02-01",
      reconstructable: true,
      skipReason: null,
      ...over,
    },
  };
}

function wrap(children: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe("BalanceHistoryCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getAccountBalanceTimeline.mockResolvedValue(timeline());
  });

  it("reports the peak and its date — the question the card exists to answer", async () => {
    render(wrap(<BalanceHistoryCard account={account} />));
    await waitFor(() => expect(screen.getByText("Highest")).toBeInTheDocument());

    expect(screen.getByText("$10,000")).toBeInTheDocument();
    expect(screen.getByText("May 1, 2024")).toBeInTheDocument();
    expect(screen.getByText("Lowest")).toBeInTheDocument();
  });

  it("warns that amounts are unanchored while dates still stand", async () => {
    getAccountBalanceTimeline.mockResolvedValue(timeline({ anchor: "assumedZero" }));
    render(wrap(<BalanceHistoryCard account={account} />));

    await waitFor(() => expect(screen.getByText(/Dates are exact/i)).toBeInTheDocument());
    expect(screen.getByText(/off by the same unknown amount/i)).toBeInTheDocument();
  });

  it("stays silent when the balance cannot be honestly reconstructed", async () => {
    getAccountBalanceTimeline.mockResolvedValue(
      timeline({ reconstructable: false, points: [], peak: null, trough: null }),
    );
    const { container } = render(wrap(<BalanceHistoryCard account={account} />));

    // Nothing to await on — assert the card never appears rather than racing it.
    await waitFor(() => expect(getAccountBalanceTimeline).toHaveBeenCalled());
    expect(container.querySelector(".card")).toBeNull();
    expect(screen.queryByText("Balance history")).not.toBeInTheDocument();
  });

  it("re-queries with a since date when the range changes", async () => {
    render(wrap(<BalanceHistoryCard account={account} />));
    await waitFor(() => expect(screen.getByText("Highest")).toBeInTheDocument());

    // Default range is 1Y, so the first call already carries a since date.
    expect(getAccountBalanceTimeline).toHaveBeenCalledWith("sav1", expect.any(String));

    fireEvent.click(screen.getByRole("button", { name: "All" }));
    await waitFor(() =>
      expect(getAccountBalanceTimeline).toHaveBeenCalledWith("sav1", null),
    );
  });

  it("does not draw a curve from a single point", async () => {
    getAccountBalanceTimeline.mockResolvedValue(
      timeline({ points: [{ date: "2024-05-01", balanceCents: 1_000 }] }),
    );
    const { container } = render(wrap(<BalanceHistoryCard account={account} />));

    await waitFor(() => expect(screen.getByText(/Not enough activity/i)).toBeInTheDocument());
    expect(container.querySelector("svg")).toBeNull();
  });
});

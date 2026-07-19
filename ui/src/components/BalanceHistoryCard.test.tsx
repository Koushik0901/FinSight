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

    // The refusal is only knowable once the backend answers, so the card mounts
    // its chrome first and then withdraws. What matters is that it never shows a
    // number for an account whose balance can't be honestly derived.
    await waitFor(() => expect(container.querySelector(".card")).toBeNull());
    expect(screen.queryByText("Balance history")).not.toBeInTheDocument();
    expect(screen.queryByText("Highest")).not.toBeInTheDocument();
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

  it("keeps the range chips mounted while a new range loads", async () => {
    // Never resolving: the card must not yank its own controls out from under
    // the click that triggered the fetch.
    getAccountBalanceTimeline.mockReturnValue(new Promise(() => {}));
    render(wrap(<BalanceHistoryCard account={account} />));

    await waitFor(() => expect(screen.getByRole("button", { name: "All" })).toBeInTheDocument());
    expect(screen.getByText("Balance history")).toBeInTheDocument();
    // And it must not claim there's no activity when it simply doesn't know yet.
    expect(screen.queryByText(/Not enough activity/i)).not.toBeInTheDocument();
  });

  it("derives the since date from local time, not UTC", async () => {
    render(wrap(<BalanceHistoryCard account={account} />));
    await waitFor(() => expect(getAccountBalanceTimeline).toHaveBeenCalled());

    const since = getAccountBalanceTimeline.mock.calls[0]![1] as string;
    const expected = new Date();
    expected.setDate(expected.getDate() - 365);
    const pad = (n: number) => String(n).padStart(2, "0");
    expect(since).toBe(
      `${expected.getFullYear()}-${pad(expected.getMonth() + 1)}-${pad(expected.getDate())}`,
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

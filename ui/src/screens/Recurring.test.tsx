import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import Recurring from "./Recurring";
import { createWrapperWithEntries } from "../test-utils";
import { useRecurring, useSetSubscriptionVerdict } from "../api/hooks/recurring";
import { usePlannedTransactions } from "../api/hooks/plannedTransactions";

const mockSetVerdict = vi.fn();
vi.mock("../api/hooks/recurring", () => ({
  useSetSubscriptionVerdict: vi.fn(() => ({ mutate: mockSetVerdict, isPending: false })),
  useRecurring: vi.fn(() => ({
    data: [
      {
        merchantKey: "spotify",
        merchantRaw: "Spotify",
        categoryLabel: "Subscriptions",
        categoryColor: "#22C55E",
        lastAmountCents: -1299,
        minAmountCents: -1299,
        maxAmountCents: -1299,
        avgGapDays: 30,
        occurrences: 5,
        lastSeen: "2026-06-01",
        nextExpected: "2026-07-01",
        cadence: "monthly",
        isSubscription: true,
        kind: "subscription",
        confidence: 0.9,
        reasons: ["5 occurrences", "~monthly cadence", "known subscription vendor (spotify)"],
        monthlyEquivalentCents: 1299,
        feedsProjections: true,
      },
      {
        // Detected, but too weak to budget against: it must stay VISIBLE (the
        // user is the one who can confirm or dismiss it) while being excluded
        // from the committed-per-month headline.
        merchantKey: "odd jobs ltd",
        merchantRaw: "Odd Jobs Ltd",
        categoryLabel: "Other",
        categoryColor: "#94A3B8",
        lastAmountCents: -20000,
        minAmountCents: -31500,
        maxAmountCents: -4100,
        avgGapDays: 47,
        occurrences: 3,
        lastSeen: "2026-06-02",
        nextExpected: "2026-07-19",
        cadence: "monthly",
        isSubscription: false,
        kind: "bill",
        confidence: 0.34,
        reasons: ["3 occurrences", "irregular cadence"],
        monthlyEquivalentCents: 20000,
        feedsProjections: false,
      },
    ],
    isLoading: false,
    error: null,
  })),
}));

vi.mock("../api/hooks/plannedTransactions", () => ({
  usePlannedTransactions: vi.fn(() => ({
    data: [
      {
        id: "pt-1",
        description: "Insurance premium",
        amountCents: -350000,
        accountId: "acc-1",
        categoryId: "cat-1",
        dueDate: "2026-07-10",
        status: "planned",
        source: "manual",
        createdAt: "2026-06-01T00:00:00Z",
      },
    ],
  })),
  useCreatePlannedTransaction: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdatePlannedTransaction: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeletePlannedTransaction: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [{ id: "acc-1", name: "Chase Checking", bank: "Chase", type: "Checking", balance_cents: 100000, currency: "USD", color: "#3B82F6" }] })),
}));

vi.mock("../api/hooks/transactions", () => ({
  useCategories: vi.fn(() => ({ data: [{ id: "cat-1", label: "Insurance" }] })),
}));

describe("Recurring — empty state", () => {
  it("renders an intentional empty state when nothing is recurring or planned", () => {
    vi.mocked(useRecurring).mockReturnValueOnce({ data: [], isLoading: false, error: null } as unknown as ReturnType<typeof useRecurring>);
    vi.mocked(usePlannedTransactions).mockReturnValueOnce({ data: [] } as unknown as ReturnType<typeof usePlannedTransactions>);
    render(<Recurring />, { wrapper: createWrapperWithEntries(["/recurring"]) });
    expect(screen.getByText("No recurring items yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Import transactions/i })).toBeInTheDocument();
  });
});

describe("Recurring — planned transactions", () => {
  it("opens the planned transaction drawer when focusPlanned is present", async () => {
    render(<Recurring />, { wrapper: createWrapperWithEntries(["/recurring?focusPlanned=pt-1"]) });
    expect(await screen.findByText("Planned transaction · Insurance premium")).toBeInTheDocument();
    expect(screen.getByText("Description")).toBeInTheDocument();
  });
});

describe("Recurring — low-confidence entries", () => {
  it("lists a weak entry but leaves it out of the committed-per-month total", () => {
    render(<Recurring />, { wrapper: createWrapperWithEntries(["/recurring"]) });

    // Visible: the user is the one who can confirm or dismiss it.
    expect(screen.getByText(/Odd Jobs Ltd/i)).toBeInTheDocument();
    expect(screen.getByText(/not used in forecasts/i)).toBeInTheDocument();

    // The headline counts only the $12.99 subscription, not the $200 guess.
    // If the weak entry were counted the figure would read $213.
    expect(screen.getByText("$13")).toBeInTheDocument();
    expect(screen.queryByText("$213")).not.toBeInTheDocument();
    expect(
      screen.getByText(/1 less certain entry is listed below but not counted/i),
    ).toBeInTheDocument();
  });
});

// Placed last: these set a persistent mockReturnValue, so ordering after the
// default-mock tests keeps them from leaking into earlier cases.
describe("Recurring — subscription price changes (#58)", () => {
  const withChange = [
    {
      merchantKey: "spotify",
      merchantRaw: "Spotify",
      categoryLabel: "Subscriptions",
      categoryColor: "#22C55E",
      lastAmountCents: -1299,
      minAmountCents: -1299,
      maxAmountCents: -1299,
      avgGapDays: 30,
      occurrences: 9,
      lastSeen: "2026-06-01",
      nextExpected: "2026-07-01",
      cadence: "monthly",
      isSubscription: true,
      kind: "subscription",
      confidence: 0.96,
      reasons: ["9 occurrences", "~monthly cadence"],
      monthlyEquivalentCents: 1299,
      feedsProjections: true,
      priceChange: { fromCents: 999, toCents: 1299, pct: 30, effectiveDate: "2026-05-05", currency: "USD" },
      verdict: null,
    },
  ];

  it("surfaces a detected price change with confirm/dismiss and wires the verdict", () => {
    vi.mocked(useRecurring).mockReturnValue({ data: withChange, isLoading: false, error: null } as unknown as ReturnType<typeof useRecurring>);
    vi.mocked(usePlannedTransactions).mockReturnValue({ data: [] } as unknown as ReturnType<typeof usePlannedTransactions>);
    mockSetVerdict.mockClear();
    render(<Recurring />, { wrapper: createWrapperWithEntries(["/recurring"]) });

    // The review card summarizes the move with evidence (the from → to amounts
    // are unique to the card; the old price doesn't appear in the table row).
    expect(screen.getByText(/Changes to review/i)).toBeInTheDocument();
    expect(screen.getByText(/9\.99/)).toBeInTheDocument();

    // Dismiss routes to the verdict mutation with the series' key.
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(mockSetVerdict).toHaveBeenCalledWith({ merchantKey: "spotify", verdict: "dismissed" });
  });

  it("hides the review card once every change is confirmed or dismissed", () => {
    vi.mocked(useRecurring).mockReturnValue({
      data: [{ ...withChange[0], verdict: "dismissed" }],
      isLoading: false,
      error: null,
    } as unknown as ReturnType<typeof useRecurring>);
    vi.mocked(usePlannedTransactions).mockReturnValue({ data: [] } as unknown as ReturnType<typeof usePlannedTransactions>);
    render(<Recurring />, { wrapper: createWrapperWithEntries(["/recurring"]) });
    expect(screen.queryByText(/Changes to review/i)).not.toBeInTheDocument();
    // The row is still listed (dismissed), with a Restore affordance.
    expect(screen.getByRole("button", { name: /Restore Spotify/i })).toBeInTheDocument();
  });
});

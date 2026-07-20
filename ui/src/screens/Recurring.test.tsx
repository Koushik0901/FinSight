import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Recurring from "./Recurring";
import { createWrapperWithEntries } from "../test-utils";
import { useRecurring } from "../api/hooks/recurring";
import { usePlannedTransactions } from "../api/hooks/plannedTransactions";

vi.mock("../api/hooks/recurring", () => ({
  useRecurring: vi.fn(() => ({
    data: [
      {
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

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Recurring from "./Recurring";
import { createWrapperWithEntries } from "../test-utils";

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

describe("Recurring — planned transactions", () => {
  it("opens the planned transaction drawer when focusPlanned is present", async () => {
    render(<Recurring />, { wrapper: createWrapperWithEntries(["/recurring?focusPlanned=pt-1"]) });
    expect(await screen.findByText("Planned transaction · Insurance premium")).toBeInTheDocument();
    expect(screen.getByText("Description")).toBeInTheDocument();
  });
});

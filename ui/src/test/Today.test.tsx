import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { vi } from "vitest";
import Today from "../screens/Today";
import type { ReactNode } from "react";

const mockUseAccounts = vi.fn();
const mockUseMonthTotals = vi.fn();
const mockUseSavingsRateHistory = vi.fn();
const mockUseFinancialMetrics = vi.fn();
const mockUseCategoriesWithSpending = vi.fn();
const mockUseNeedsReviewCount = vi.fn();
const mockUseAgentStatus = vi.fn();

vi.mock("../api/hooks/accounts", async (importOriginal) => {
  const actual = await importOriginal() as any;
  return { ...actual, useAccounts: () => mockUseAccounts() };
});
vi.mock("../api/hooks", async (importOriginal) => {
  const actual = await importOriginal() as any;
  return { ...actual, useMonthTotals: () => mockUseMonthTotals(), useSavingsRateHistory: () => mockUseSavingsRateHistory() };
});
vi.mock("../api/hooks/metrics", async (importOriginal) => {
  const actual = await importOriginal() as any;
  return { ...actual, useFinancialMetrics: () => mockUseFinancialMetrics() };
});
vi.mock("../api/hooks/transactions", async (importOriginal) => {
  const actual = await importOriginal() as any;
  return { ...actual, useCategoriesWithSpending: () => mockUseCategoriesWithSpending() };
});
vi.mock("../api/hooks/agent", async (importOriginal) => {
  const actual = await importOriginal() as any;
  return { ...actual, useNeedsReviewCount: () => mockUseNeedsReviewCount(), useAgentStatus: () => mockUseAgentStatus() };
});

vi.mock("../api/hooks/networth", () => ({
  useNetWorth: () => 1482042,
  useNetWorthHistory: () => ({ data: [] }),
}));

vi.mock("../api/hooks/recurring", () => ({
  useRecurring: () => ({
    data: [
      {
        merchantRaw: "Spotify",
        categoryLabel: "Subscriptions",
        categoryColor: "#8B5CF6",
        lastAmountCents: -999,
        avgGapDays: 30,
        occurrences: 6,
        lastSeen: "2026-05-05",
        nextExpected: new Date(Date.now() + 2 * 86400000).toISOString().slice(0, 10),
        frequency: "monthly",
      },
      {
        merchantRaw: "OldGym",
        categoryLabel: "Health",
        categoryColor: "#34D399",
        lastAmountCents: -4999,
        avgGapDays: 30,
        occurrences: 3,
        lastSeen: "2026-04-01",
        nextExpected: new Date(Date.now() + 40 * 86400000).toISOString().slice(0, 10),
        frequency: "monthly",
      },
    ],
  }),
}));

vi.mock("../api/hooks/budget", () => ({
  useGoals: () => ({
    data: [
      { id: "g1", name: "Italy Fund", goalType: "save-by-date", targetCents: 500000,
        currentCents: 100000, monthlyCents: 50000, targetDate: "2027-06-01",
        color: "#C9F950", notes: null, sortOrder: 0, createdAt: "2026-01-01" },
    ],
  }),
  useUpdateGoalBalance: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false }),
  useContributeToGoal: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false }),
  useGoalContributions: () => ({ data: [] }),
}));

vi.mock("../api/hooks/insights", () => ({
  useHealthScore: () => ({ data: null }),
}));

vi.mock("../api/hooks/assets", () => ({
  useUncelebratedMilestones: () => ({ data: [] }),
}));

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={qc}>
      <BrowserRouter>{node}</BrowserRouter>
    </QueryClientProvider>
  );
}

const defaultAccounts = [
  { id: "a1", name: "Joint Checking", bank: "Mercury", type: "Checking", balanceCents: 1482042, currency: "USD", color: "#C9F950", accountGroup: "cash", goalEarmark: null } as any,
];
const defaultMetrics = {
  liquidCents: 1482042, investedCents: 0, debtCents: 0, netWorthCents: 1482042, currency: "USD", runwayDays: 30, avgMonthlyExpenseCents: 400000, accountsWithUnknownBalance: 0, unconvertedHoldings: [],
} as any;
const defaultCategories = [] as any[];

beforeEach(() => {
  mockUseAccounts.mockReturnValue({ data: defaultAccounts, isLoading: false });
  mockUseMonthTotals.mockReturnValue({ data: { incomeCents: 0, expenseCents: 0, netCents: 0, savingsRatePct: 0, txnCount: 0 }, isLoading: false });
  mockUseSavingsRateHistory.mockReturnValue({ data: [] });
  mockUseFinancialMetrics.mockReturnValue({ data: defaultMetrics });
  mockUseCategoriesWithSpending.mockReturnValue({ data: defaultCategories });
  mockUseNeedsReviewCount.mockReturnValue({ data: 0 });
  mockUseAgentStatus.mockReturnValue({ data: { lastScanAt: null, anomalyCount: 0 } });
});

describe("Today", () => {
  it("renders the net-worth hero from useNetWorth", async () => {
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getAllByText(/\$14,820/).length).toBeGreaterThan(0);
    });
  });

  it("offers to record remaining cash flow when netCents > 5000", async () => {
    mockUseMonthTotals.mockReturnValue({ data: { incomeCents: 600000, expenseCents: 400000, netCents: 200000, savingsRatePct: 33, txnCount: 42 }, isLoading: false });
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText(/remains from this month.s cash flow/i)).toBeInTheDocument();
    });
  });

  it("hides the remaining-cash-flow action after Dismiss", async () => {
    mockUseMonthTotals.mockReturnValue({ data: { incomeCents: 600000, expenseCents: 400000, netCents: 200000, savingsRatePct: 33, txnCount: 42 }, isLoading: false });
    render(wrap(<Today />));
    await waitFor(() => screen.getByText(/remains from this month.s cash flow/i));
    fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    expect(screen.queryByText(/remains from this month.s cash flow/i)).toBeNull();
  });

  it("shows recurring chip for item within 7 days but not for item 40 days out", async () => {
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText(/Spotify/)).toBeInTheDocument();
    });
    expect(screen.queryByText(/OldGym/)).toBeNull();
  });

  it("shows Runway stat with computed value", async () => {
    mockUseMonthTotals.mockReturnValue({ data: { incomeCents: 600000, expenseCents: 300000, netCents: 300000, savingsRatePct: 50, txnCount: 20 }, isLoading: false });
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText("Runway")).toBeInTheDocument();
    });
  });
});

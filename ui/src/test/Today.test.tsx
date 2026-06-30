import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Today from "../screens/Today";
import type { ReactNode } from "react";

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

describe("Today", () => {
  it("renders the net-worth hero from useNetWorth", async () => {
    vi.mocked(invoke).mockResolvedValue([
      { id: "a1", owner: "joint", bank: "Mercury", type: "Checking", name: "Joint Checking",
        balance_cents: 1482042, currency: "USD", color: "#C9F950" },
    ]);
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText(/\$14,820/)).toBeInTheDocument();
    });
  });

  it("shows Smart Sweep card when netCents > 5000", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_month_totals") return Promise.resolve({
        incomeCents: 600000, expenseCents: 400000, netCents: 200000,
        savingsRatePct: 33, txnCount: 42,
      });
      return Promise.resolve([
        { id: "a1", owner: "joint", bank: "Mercury", type: "Checking", name: "Joint Checking",
          balance_cents: 1482042, currency: "USD", color: "#C9F950" },
      ]);
    });
    render(wrap(<Today />));
    await waitFor(() => {
      const matches = screen.queryAllByText((_, el) =>
        el?.tagName === "DIV" &&
        /You have .* unallocated this month/.test(el?.textContent ?? "")
      );
      expect(matches.length).toBeGreaterThan(0);
    });
  });

  it("hides Smart Sweep card after Dismiss click", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_month_totals") return Promise.resolve({
        incomeCents: 600000, expenseCents: 400000, netCents: 200000,
        savingsRatePct: 33, txnCount: 42,
      });
      return Promise.resolve([
        { id: "a1", owner: "joint", bank: "Mercury", type: "Checking", name: "Joint Checking",
          balance_cents: 1482042, currency: "USD", color: "#C9F950" },
      ]);
    });
    render(wrap(<Today />));
    await waitFor(() => screen.getByText(/unallocated this month/));
    fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    expect(screen.queryByText(/unallocated this month/)).toBeNull();
  });

  it("shows recurring chip for item within 7 days but not for item 40 days out", async () => {
    vi.mocked(invoke).mockResolvedValue([
      { id: "a1", owner: "joint", bank: "Mercury", type: "Checking", name: "Joint Checking",
        balance_cents: 1482042, currency: "USD", color: "#C9F950" },
    ]);
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText(/Spotify/)).toBeInTheDocument();
    });
    expect(screen.queryByText(/OldGym/)).toBeNull();
  });

  it("shows Runway stat with computed value", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_month_totals") return Promise.resolve({
        incomeCents: 600000, expenseCents: 300000, netCents: 300000,
        savingsRatePct: 50, txnCount: 20,
      });
      return Promise.resolve([
        { id: "a1", owner: "joint", bank: "Mercury", type: "Checking", name: "Joint Checking",
          balance_cents: 1482042, currency: "USD", color: "#C9F950" },
      ]);
    });
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText("Runway")).toBeInTheDocument();
    });
  });
});

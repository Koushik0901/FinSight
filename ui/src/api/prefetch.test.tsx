import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

// Force the desktop runtime gate on so hooks + prefetch actually run.
vi.mock("../utils/runtime", () => ({ isTauriRuntime: () => true, isBackendAvailable: () => true }));

// Spy-able command surface. `vi.hoisted` so the spies exist when the hoisted
// vi.mock factory runs. Each returns the ok-Result shape the hooks unwrap.
const commandMocks = vi.hoisted(() => ({
  listAccounts: vi.fn(async () => ({ status: "ok", data: [] })),
  getMonthTotals: vi.fn(async () => ({ status: "ok", data: { incomeCents: 0, expenseCents: 0 } })),
  listCategoriesWithSpending: vi.fn(async () => ({ status: "ok", data: [] })),
  listGoals: vi.fn(async () => ({ status: "ok", data: [] })),
  listRecurring: vi.fn(async () => ({ status: "ok", data: [] })),
  getSavingsRateHistory: vi.fn(async () => ({ status: "ok", data: [] })),
  getNeedsReviewCount: vi.fn(async () => ({ status: "ok", data: 0 })),
  getAgentStatus: vi.fn(async () => ({ status: "ok", data: null })),
  getFinancialHealthScore: vi.fn(async () => ({ status: "ok", data: null })),
  getSpendingBreakdown: vi.fn(async () => ({ status: "ok", data: null })),
  listBudgetEnvelopes: vi.fn(async () => ({ status: "ok", data: [] })),
  listBudgetHistory: vi.fn(async () => ({ status: "ok", data: [] })),
  listHouseholdMembers: vi.fn(async () => ({ status: "ok", data: [] })),
  listCategoryProposals: vi.fn(async () => ({ status: "ok", data: [] })),
}));
const { listAccounts, getMonthTotals } = commandMocks;
vi.mock("./openapiClient", async () => {
  const actual = await vi.importActual<typeof import("./openapiClient")>("./openapiClient");
  return {
    ...actual,
    commands: commandMocks,
  };
});

import { prefetchRoute, warmOfflineEssentials } from "./prefetch";
import { useAccounts } from "./hooks/accounts";
import { useBudgetEnvelopes, useBudgetHistory } from "./hooks/budget";
import { useHouseholdMembers } from "./hooks/household";
import { useMonthTotals } from "./hooks/reports";

beforeEach(() => {
  listAccounts.mockClear();
  getMonthTotals.mockClear();
  for (const mock of Object.values(commandMocks)) mock.mockClear();
});

function Harness({ hook }: { hook: () => unknown }) {
  hook();
  return null;
}

/**
 * The load-bearing guarantee: a prefetch under a route's descriptor key must be
 * READ by the destination screen's hook. If the keys drifted, the hook would
 * re-fetch and the command would be called twice; asserting exactly one call
 * proves byte-identical key match end-to-end.
 */
describe("prefetch key-match (warms the cache the screen reads)", () => {
  it("useAccounts reads the /accounts prefetch (command called once, not twice)", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    await qc.prefetchQuery({ queryKey: ["accounts"], queryFn: () => listAccounts().then((r) => r.data) });
    prefetchRoute(qc, "/accounts"); // idempotent: dedupes against the fresh entry
    render(
      <QueryClientProvider client={qc}>
        <Harness hook={useAccounts} />
      </QueryClientProvider>
    );
    // Give any (incorrect) refetch a chance to fire.
    await waitFor(() => expect(listAccounts).toHaveBeenCalled());
    expect(listAccounts).toHaveBeenCalledTimes(1);
  });

  it("prefetchRoute('/') warms month-totals under the exact useMonthTotals key", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    prefetchRoute(qc, "/");
    await waitFor(() => expect(getMonthTotals).toHaveBeenCalledTimes(1));
    // The screen hook now reads the warmed entry — no second call.
    render(
      <QueryClientProvider client={qc}>
        <Harness hook={useMonthTotals} />
      </QueryClientProvider>
    );
    await new Promise((r) => setTimeout(r, 20));
    expect(getMonthTotals).toHaveBeenCalledTimes(1);
  });

  it("an unmapped route is a no-op", () => {
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "prefetchQuery");
    prefetchRoute(qc, "/settings");
    expect(spy).not.toHaveBeenCalled();
  });

  it("warms the complete Budget first paint under the screen's exact keys", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    prefetchRoute(qc, "/budget");

    await waitFor(() => {
      expect(commandMocks.listBudgetEnvelopes).toHaveBeenCalledTimes(1);
      expect(commandMocks.listBudgetHistory).toHaveBeenCalledWith(5);
      expect(commandMocks.listHouseholdMembers).toHaveBeenCalledTimes(1);
      expect(commandMocks.getMonthTotals).toHaveBeenCalledTimes(1);
      expect(commandMocks.listGoals).toHaveBeenCalledTimes(1);
      expect(commandMocks.getSpendingBreakdown).toHaveBeenCalledTimes(1);
    });
  });

  it("seeds Budget's missing offline data and the real hooks reuse it", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    warmOfflineEssentials(qc);

    await waitFor(() => {
      expect(commandMocks.listBudgetEnvelopes).toHaveBeenCalledTimes(1);
      expect(commandMocks.listBudgetHistory).toHaveBeenCalledWith(5);
      expect(commandMocks.getSpendingBreakdown).toHaveBeenCalledTimes(1);
      expect(commandMocks.listHouseholdMembers).toHaveBeenCalledTimes(1);
    });

    render(
      <QueryClientProvider client={qc}>
        <Harness hook={() => {
          useBudgetEnvelopes();
          useBudgetHistory(5);
          useHouseholdMembers();
        }} />
      </QueryClientProvider>
    );
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(commandMocks.listBudgetEnvelopes).toHaveBeenCalledTimes(1);
    expect(commandMocks.listBudgetHistory).toHaveBeenCalledTimes(1);
    expect(commandMocks.listHouseholdMembers).toHaveBeenCalledTimes(1);
    expect(qc.getQueryData(["spending-breakdown"])).toBeNull();
  });
});

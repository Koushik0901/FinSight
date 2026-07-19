import { describe, it, expect, vi, beforeEach } from "vitest";
import { QueryClient } from "@tanstack/react-query";

vi.mock("../utils/runtime", () => ({ isTauriRuntime: () => true, isBackendAvailable: () => true }));

const { listAccounts, listTransactions, listCategoriesWithSpending } = vi.hoisted(() => ({
  listAccounts: vi.fn(async () => ({ status: "ok", data: [] })),
  listTransactions: vi.fn(async () => ({ status: "ok", data: [] })),
  listCategoriesWithSpending: vi.fn(async () => ({ status: "ok", data: [] })),
}));
vi.mock("./client", async () => {
  const actual = await vi.importActual<typeof import("./client")>("./client");
  return {
    ...actual,
    commands: { listAccounts, listTransactions, listCategoriesWithSpending },
  };
});

import { prefetchRoute, prefetchAccountTransactions } from "./prefetch";
import { invalidateDomains } from "./invalidation";

beforeEach(() => {
  listAccounts.mockClear();
  listTransactions.mockClear();
  listCategoriesWithSpending.mockClear();
});

describe("anticipatory infra — concurrency & edge cases", () => {
  it("rapid repeated hovers dedupe to a single fetch (prefetch is idempotent)", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    // Simulate a user sweeping the mouse over the same nav item many times.
    for (let i = 0; i < 10; i++) prefetchRoute(qc, "/accounts");
    await vi.waitFor(() => expect(listAccounts).toHaveBeenCalled());
    expect(listAccounts).toHaveBeenCalledTimes(1);
  });

  it("account-row hover prefetch dedupes across repeated hovers", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    for (let i = 0; i < 5; i++) prefetchAccountTransactions(qc, "acc-1");
    await vi.waitFor(() => expect(listTransactions).toHaveBeenCalled());
    expect(listTransactions).toHaveBeenCalledTimes(1);
    expect(listCategoriesWithSpending).toHaveBeenCalledTimes(1);
  });

  it("Delete-All (qc.clear) drops every prefetched/derived entry", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    prefetchRoute(qc, "/accounts");
    prefetchAccountTransactions(qc, "acc-1");
    await vi.waitFor(() => expect(qc.getQueryData(["accounts"])).toBeDefined());
    expect(qc.getQueryData(["transactions-infinite", expect.anything()] as never)).toBeUndefined();
    // The reset path.
    qc.clear();
    expect(qc.getQueryData(["accounts"])).toBeUndefined();
    expect(qc.getQueryCache().getAll()).toHaveLength(0);
  });

  it("a prefetch for one account does not populate another account's key", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    prefetchAccountTransactions(qc, "acc-1");
    await vi.waitFor(() => expect(listTransactions).toHaveBeenCalled());
    // acc-2's page must be a cache miss — keys are per-account, so a stale
    // prefetch can't masquerade as another account's data.
    const acc2Key = ["transactions-infinite", { accountId: "acc-2", search: null, filterPreset: null, startDate: null, endDate: null }];
    expect(qc.getQueryData(acc2Key as never)).toBeUndefined();
  });

  it("invalidateDomains marks entries stale without removing them (so no flash of empty)", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000, retry: false } } });
    await qc.prefetchQuery({ queryKey: ["accounts"], queryFn: async () => ["a"] });
    await invalidateDomains(qc, "accounts");
    // Data is still present (stale), not evicted — the screen keeps showing it
    // while the background refetch runs, unlike qc.clear().
    expect(qc.getQueryData(["accounts"])).toEqual(["a"]);
    expect(qc.getQueryState(["accounts"])?.isInvalidated).toBe(true);
  });
});

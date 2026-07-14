import type { QueryClient } from "@tanstack/react-query";
import { commands } from "./client";
import { isTauriRuntime } from "../utils/runtime";
import { TXN_PAGE_SIZE } from "./hooks/transactions";
import type { TxnFilterInput } from "./client";

/**
 * Prefetch-on-intent: warm a route's summary queries when the user *signals*
 * they're heading there (Sidebar hover/focus) — 100s of ms before the click —
 * so the screen mounts into a warm cache instead of firing an 8–13-query burst.
 *
 * CRITICAL: each descriptor's `key` must be byte-identical to the key the
 * destination screen's hook uses, or the prefetch warms a cache the screen
 * never reads (a wasted IPC round-trip and zero benefit). This is verified by
 * `prefetch.test.ts`, which prefetches then reads through the real hooks and
 * asserts a cache hit (the command is not called again).
 *
 * Prefetch is non-destructive (reads only) and idempotent: `prefetchQuery`
 * dedupes against an in-flight/fresh entry, so repeated hovers don't re-fetch.
 * A short `staleTime` keeps the warmed entry fresh through the click without
 * pinning stale data.
 */

/** Keep warmed entries fresh at least long enough to cover hover→click. */
const PREFETCH_STALE_MS = 10_000;

const unwrap = <T>(r: { status: "ok" | "error"; data?: T; error?: { message: string } }): T => {
  if (r.status === "error") throw new Error(r.error?.message ?? "command failed");
  return r.data as T;
};

interface Descriptor {
  readonly key: readonly unknown[];
  readonly fn: () => Promise<unknown>;
}

/**
 * Parameterless summary-query descriptors, each keyed EXACTLY as its screen
 * hook keys it (see the referenced hook in the comment).
 */
const D = {
  accounts: { key: ["accounts"], fn: async () => unwrap(await commands.listAccounts()) }, // useAccounts
  monthTotals: { key: ["month-totals"], fn: async () => unwrap(await commands.getMonthTotals()) }, // useMonthTotals
  categoriesWithSpending: {
    key: ["categories-with-spending"],
    fn: async () => unwrap(await commands.listCategoriesWithSpending()),
  }, // useCategoriesWithSpending
  goals: { key: ["goals"], fn: async () => unwrap(await commands.listGoals()) }, // useGoals
  recurring: { key: ["recurring"], fn: async () => unwrap(await commands.listRecurring()) }, // useRecurring
  savingsRate: {
    key: ["savings-rate-history"],
    fn: async () => unwrap(await commands.getSavingsRateHistory()),
  }, // useSavingsRateHistory
  needsReview: {
    key: ["needs-review-count"],
    fn: async () => unwrap(await commands.getNeedsReviewCount()),
  }, // useNeedsReviewCount
  agentStatus: { key: ["agent-status"], fn: async () => unwrap(await commands.getAgentStatus()) }, // useAgentStatus
  healthScore: {
    key: ["financial-health-score"],
    fn: async () => unwrap(await commands.getFinancialHealthScore()),
  }, // useHealthScore
  spendingBreakdown: {
    key: ["spending-breakdown"],
    fn: async () => unwrap(await commands.getSpendingBreakdown()),
  }, // Budget.tsx inline
} as const satisfies Record<string, Descriptor>;

/**
 * Route path → the summary queries that gate its first useful paint. Only the
 * heavy/visible ones per route — not every query the screen eventually makes.
 */
const ROUTE_PREFETCH: Record<string, readonly Descriptor[]> = {
  "/": [
    D.accounts,
    D.monthTotals,
    D.categoriesWithSpending,
    D.goals,
    D.recurring,
    D.savingsRate,
    D.needsReview,
    D.agentStatus,
    D.healthScore,
  ], // Today — the biggest burst
  "/accounts": [D.accounts],
  "/reports": [D.monthTotals, D.savingsRate, D.spendingBreakdown],
  "/categories": [D.categoriesWithSpending],
  "/budget": [D.categoriesWithSpending, D.goals, D.spendingBreakdown],
  "/recurring": [D.recurring],
  "/goals": [D.goals],
  "/inbox": [D.needsReview],
};

/** Prefetch a route's summary queries. No-op off the desktop runtime or for an unmapped path. */
export function prefetchRoute(qc: QueryClient, path: string): void {
  if (!isTauriRuntime()) return;
  const descriptors = ROUTE_PREFETCH[path];
  if (!descriptors) return;
  for (const d of descriptors) {
    void qc.prefetchQuery({ queryKey: d.key, queryFn: d.fn, staleTime: PREFETCH_STALE_MS });
  }
}

/**
 * The default (all-transactions) filter an account-transactions screen opens
 * with. Must match `AccountTransactions`'s `filterValue` for the empty state so
 * the prefetched first page is the one it reads.
 */
function defaultAccountFilter(accountId: string): Omit<TxnFilterInput, "limit" | "offset"> {
  return {
    accountId,
    search: null,
    filterPreset: null,
    startDate: null,
    endDate: null,
  };
}

/**
 * Prefetch the first page of an account's transactions (account-row hover →
 * open). Uses `prefetchInfiniteQuery` with the exact key + page shape
 * `useInfiniteTransactions` uses, plus the account list the screen also needs.
 */
export function prefetchAccountTransactions(qc: QueryClient, accountId: string): void {
  if (!isTauriRuntime()) return;
  const filter = defaultAccountFilter(accountId);
  void qc.prefetchInfiniteQuery({
    queryKey: ["transactions-infinite", filter],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) =>
      unwrap(
        await commands.listTransactions({
          ...filter,
          limit: TXN_PAGE_SIZE,
          offset: (pageParam as number) * TXN_PAGE_SIZE,
        } as TxnFilterInput)
      ),
    staleTime: PREFETCH_STALE_MS,
  });
  void qc.prefetchQuery({
    queryKey: D.categoriesWithSpending.key,
    queryFn: D.categoriesWithSpending.fn,
    staleTime: PREFETCH_STALE_MS,
  });
}

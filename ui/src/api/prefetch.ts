import type { QueryClient } from "@tanstack/react-query";
import { api } from "./openapiClient";
import { unwrapResult } from "./openapiClient";
import { isBackendAvailable } from "../utils/runtime";
import { prefetchRouteChunk } from "../utils/routePrefetch";
import { TXN_PAGE_SIZE } from "./hooks/transactions";
import type { TxnFilterInput } from "./openapiClient";

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
/** Keep startup-warmed offline data fresh through IndexedDB persistence. */
const OFFLINE_WARM_STALE_MS = 60_000;


interface Descriptor {
  readonly key: readonly unknown[];
  readonly fn: () => Promise<unknown>;
}

/**
 * Parameterless summary-query descriptors, each keyed EXACTLY as its screen
 * hook keys it (see the referenced hook in the comment).
 */
const D = {
  accounts: { key: ["accounts"], fn: async () => unwrapResult(await api.listAccounts()) }, // useAccounts
  monthTotals: { key: ["month-totals"], fn: async () => unwrapResult(await api.getMonthTotals()) }, // useMonthTotals
  categoriesWithSpending: {
    key: ["categories-with-spending"],
    fn: async () => unwrapResult(await api.listCategoriesWithSpending()),
  }, // useCategoriesWithSpending
  goals: { key: ["goals"], fn: async () => unwrapResult(await api.listGoals()) }, // useGoals
  recurring: { key: ["recurring"], fn: async () => unwrapResult(await api.listRecurring()) }, // useRecurring
  savingsRate: {
    key: ["savings-rate-history"],
    fn: async () => unwrapResult(await api.getSavingsRateHistory()),
  }, // useSavingsRateHistory
  needsReview: {
    key: ["needs-review-count"],
    fn: async () => unwrapResult(await api.getNeedsReviewCount()),
  }, // useNeedsReviewCount
  agentStatus: { key: ["agent-status"], fn: async () => unwrapResult(await api.getAgentStatus()) }, // useAgentStatus
  healthScore: {
    key: ["financial-health-score"],
    fn: async () => unwrapResult(await api.getFinancialHealthScore()),
  }, // useHealthScore
  spendingBreakdown: {
    key: ["spending-breakdown"],
    fn: async () => unwrapResult(await api.getSpendingBreakdown()),
  }, // Budget.tsx inline
  budgetEnvelopes: {
    key: ["budget-envelopes"],
    fn: async () => unwrapResult(await api.listBudgetEnvelopes()),
  }, // useBudgetEnvelopes
  budgetHistory5: {
    key: ["budget-history", 5],
    fn: async () => unwrapResult(await api.listBudgetHistory(5)),
  }, // useBudgetHistory(5)
  householdMembers: {
    key: ["household-members"],
    fn: async () => unwrapResult(await api.listHouseholdMembers()),
  }, // useHouseholdMembers
  categoryProposals: {
    key: ["category-proposals"],
    fn: async () => unwrapResult(await api.listCategoryProposals()),
  }, // useCategoryProposals
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
  "/budget": [
    D.budgetEnvelopes,
    D.budgetHistory5,
    D.monthTotals,
    D.goals,
    D.spendingBreakdown,
    D.householdMembers,
  ],
  "/recurring": [D.recurring],
  "/goals": [D.goals],
  "/inbox": [D.needsReview],
  "/review": [D.categoryProposals],
};

/**
 * Prefetch a route's summary queries AND its lazy JavaScript chunk.
 *
 * The chunk comes first and sits outside the backend guard on purpose: it is
 * the round-trip the user actually waits on when visiting a screen for the
 * first time (the data can't even start rendering until the code that renders
 * it has arrived), and it is worth warming whether or not a backend answers.
 *
 * No-op for an unmapped path, in both halves.
 */
export function prefetchRoute(qc: QueryClient, path: string): void {
  prefetchRouteChunk(path);
  if (!isBackendAvailable()) return;
  const descriptors = ROUTE_PREFETCH[path];
  if (!descriptors) return;
  for (const d of descriptors) {
    void qc.prefetchQuery({ queryKey: d.key, queryFn: d.fn, staleTime: PREFETCH_STALE_MS });
  }
}

/**
 * Seed the small set of useful Budget data that Today does not already load.
 *
 * The authenticated app lands on Today, which naturally warms accounts,
 * totals, goals, categories, recurring items, and health summaries. Budget is
 * the important exception: without visiting it while online, its envelopes,
 * history, spending mix, and household scope never reach the encrypted query
 * cache. Warming only those four reads keeps startup modest while making the
 * core planning vertical slice available during a later network outage.
 */
export function warmOfflineEssentials(qc: QueryClient): void {
  if (!isBackendAvailable()) return;
  const descriptors: readonly Descriptor[] = [
    D.budgetEnvelopes,
    D.budgetHistory5,
    D.spendingBreakdown,
    D.householdMembers,
  ];
  for (const d of descriptors) {
    void qc.prefetchQuery({
      queryKey: d.key,
      queryFn: d.fn,
      staleTime: OFFLINE_WARM_STALE_MS,
    });
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
  if (!isBackendAvailable()) return;
  const filter = defaultAccountFilter(accountId);
  void qc.prefetchInfiniteQuery({
    queryKey: ["transactions-infinite", filter],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) =>
      unwrapResult(
        await api.listTransactions({
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

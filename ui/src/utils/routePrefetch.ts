/**
 * Warm a lazy route's JavaScript chunk before the user commits to navigating.
 *
 * This is the CODE half of prefetch-on-intent. `api/prefetch.ts` is the DATA
 * half — it warms the destination's tanstack-query cache on the same hover.
 * Both are needed: warming the queries removes the RPC round-trip, but on a
 * screen's first visit the browser still has to fetch and parse the route's
 * chunk before anything can render, and that round-trip happens first.
 * `api/prefetch.ts::prefetchRoute` calls into here so every existing call site
 * (Sidebar, BottomNav) gets both halves from one hover.
 *
 * Every screen in `App.tsx` is `lazy()`, which is right — it keeps the entry
 * bundle small — but it moves the cost to the first visit: click, wait for the
 * chunk over the network, parse, then render. On localhost that is invisible;
 * over Tailscale or on a phone it is a visible stall on every screen the user
 * opens for the first time in a session.
 *
 * Hovering or focusing a nav link is a strong, early signal of intent, and it
 * arrives a few hundred milliseconds before the click — comfortably more than
 * a 5-40 kB brotli'd chunk needs. Starting the import there usually means the
 * module is already resolved by the time the route actually changes, so the
 * Suspense fallback never appears.
 *
 * Why hover/focus and NOT a blanket prefetch-everything on idle: the route
 * chunks are not uniformly cheap. `/copilot` alone drags in the AG-UI runtime
 * (~485 kB raw between the two chunks), so speculatively fetching every
 * destination would re-download most of what code-splitting just removed from
 * the critical path. Intent-driven prefetching gets the latency win without
 * giving the bundle size back.
 *
 * These import specifiers intentionally mirror the `lazy()` calls in
 * `App.tsx`. Vite resolves both to the same module ID and therefore the same
 * chunk, so a prefetch genuinely warms what the route will later request —
 * and a screen renamed on one side fails the build rather than silently
 * splitting into two chunks. `routePrefetch.test.ts` additionally pins that
 * every nav destination has an entry here.
 */

type Importer = () => Promise<unknown>;

/** Path → the same dynamic import `App.tsx` hands to `lazy()`. */
export const ROUTE_IMPORTS: Record<string, Importer> = {
  "/": () => import("../screens/Today"),
  "/inbox": () => import("../screens/Inbox"),
  "/import-review": () => import("../screens/ImportReview"),
  "/accounts": () => import("../screens/Accounts"),
  "/transactions": () => import("../screens/AccountTransactions"),
  "/budget": () => import("../screens/Budget"),
  "/categories": () => import("../screens/Categories"),
  "/recurring": () => import("../screens/Recurring"),
  "/goals": () => import("../screens/Goals"),
  "/journey": () => import("../screens/Journey"),
  "/scenarios": () => import("../screens/Scenarios"),
  "/cashflow": () => import("../screens/Cashflow"),
  "/reports": () => import("../screens/Reports"),
  "/close": () => import("../screens/MonthClose"),
  "/path-back": () => import("../screens/PathBack"),
  "/rules": () => import("../screens/Rules"),
  "/review": () => import("../screens/Review"),
  "/settings": () => import("../screens/Settings"),
  "/copilot": () => import("../screens/Copilot"),
  "/recipes": () => import("../screens/Recipes"),
};

/** Paths already started, so repeated hovers cost nothing. */
const started = new Set<string>();

/**
 * True when the user has asked us not to spend their bandwidth speculatively.
 * `saveData` is an explicit "data saver is on"; the slow effective types mean
 * a prefetch would compete with the request the user is actually waiting for
 * rather than arriving early.
 */
function shouldSkipSpeculativeWork(): boolean {
  const conn = (
    navigator as Navigator & {
      connection?: { saveData?: boolean; effectiveType?: string };
    }
  ).connection;
  if (!conn) return false;
  if (conn.saveData) return true;
  return conn.effectiveType === "slow-2g" || conn.effectiveType === "2g";
}

/**
 * Begin loading the chunk behind `path`. Safe to call on every pointer event:
 * it is idempotent, never throws, and does nothing for an unknown path.
 *
 * A rejected import is swallowed on purpose. This is speculative work the user
 * never asked for — if the network drops mid-prefetch, the real navigation
 * will retry and surface the failure through the route error boundary, which
 * is where a user can actually act on it. An unhandled rejection here would
 * just be noise in the console.
 */
export function prefetchRouteChunk(path: string): void {
  if (started.has(path)) return;
  const load = ROUTE_IMPORTS[path];
  if (!load) return;
  if (shouldSkipSpeculativeWork()) return;

  started.add(path);
  void load().catch(() => {
    // Let the real navigation own the error; allow a later retry.
    started.delete(path);
  });
}

/** Test seam — prefetching is module-level state that would otherwise leak between cases. */
export function resetPrefetchedRoutesForTest(): void {
  started.clear();
}

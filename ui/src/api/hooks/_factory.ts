/**
 * Central query-key factories + mutation wrapper.
 *
 * Single source for every tanstack queryKey shape used across hooks and
 * screens. Any consumer that previously hand-copied a literal like
 * `["action-bundles", "pending"]` must import the factory here instead —
 * a literal silently becomes a *different* cache entry the day the key gains a
 * segment.
 *
 * Also hosts the shared `mutationWrapper` guard that enforces backend
 * availability and uniform error handling, removing the 20× `getBackend()` boilerplate
 * spread across simplefin/agent hooks.
 */
import { isBackendAvailable } from "../../utils/runtime";

// ── Action bundles ──────────────────────────────────────────────────────────
export const actionBundleKeys = {
  /** Root, for prefix invalidation only. */
  all: ["action-bundles"] as const,
  /** List shape used by every bundle-list consumer (session slot may be null). */
  list: (statusFilter?: string | null, sessionId?: string | null, limit?: number) =>
    ["action-bundles", statusFilter ?? null, sessionId ?? null, limit ?? null] as const,
  /** Convenience for the common pending-list query (status=pending). */
  pending: (sessionId?: string | null, limit?: number) =>
    ["action-bundles", "pending", sessionId ?? null, limit ?? null] as const,
} as const;

// ── SimpleFIN ───────────────────────────────────────────────────────────────
export const simplefinKeys = {
  status: ["simplefin", "status"] as const,
  accounts: ["simplefin", "accounts"] as const,
  connections: ["simplefin", "connections"] as const,
  syncSettings: ["simplefin", "syncSettings"] as const,
  alerts: ["simplefin", "alerts"] as const,
  transfers: ["simplefin", "transfers"] as const,
  importReview: ["simplefin", "importReview"] as const,
} as const;

// ── Inbox / notifications ──────────────────────────────────────────────────
export const inboxKeys = {
  actionItems: ["action-items"] as const,
  inboxBadge: ["inbox-badge-count"] as const,
  unresolvedCounterparties: ["unresolved-counterparties"] as const,
  notifications: ["notifications"] as const,
} as const;

// ── Shared mutation wrapper ─────────────────────────────────────────────────
// Wraps a mutation function with the backend-availability guard and a
// consistent error surface. Keeps the 20× `if (!isBackendAvailable()) throw …`
// boilerplate out of individual hooks.
export function mutationWrapper<TArgs extends unknown[], TData>(
  fn: (...args: TArgs) => Promise<TData>,
): (...args: TArgs) => Promise<TData> {
  return async (...args: TArgs): Promise<TData> => {
    if (!isBackendAvailable()) {
      throw new Error("This action needs a connected FinSight server.");
    }
    return fn(...args);
  };
}

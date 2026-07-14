import type { QueryClient, QueryKey } from "@tanstack/react-query";

/**
 * Centralized, dependency-aware cache invalidation.
 *
 * Before this, every mutation hand-listed the derived query keys it affected —
 * the same 6-key transaction cluster was copy-pasted into a dozen mutations, so
 * forgetting a key meant silently-stale UI and over-listing meant needless
 * refetch storms. This module makes the mutation → affected-derived-data graph
 * a single source of truth (see the Phase 7B audit): a mutation declares *what
 * changed* (a domain), not *which caches to drop*.
 *
 * Keys are query-key ROOTS. TanStack `invalidateQueries({ queryKey })` matches
 * by prefix, so `["transactions"]` invalidates every `["transactions", filter]`
 * — but note `["transactions-infinite", …]` is a DISTINCT root (not a prefix of
 * "transactions"), so both are listed explicitly where both exist.
 */

/** A query-key root used for prefix-matched invalidation. */
type Root = QueryKey;

// ── Base domains ────────────────────────────────────────────────────────────

/**
 * The ledger changed (a transaction was created/edited/deleted/split/flagged,
 * an import or sync committed rows, or a bulk recategorization applied). This is
 * the widest fan-out because nearly every derived surface is a function of the
 * transaction set.
 */
const TRANSACTIONS: Root[] = [
  ["transactions"],
  ["transactions-infinite"],
  ["month-totals"],
  ["categories-with-spending"],
  ["spending-breakdown"],
  ["budget-envelopes"],
  ["journey-status"],
  ["needs-review-count"],
  ["recurring"],
  // Investment positions + portfolio estimate are derived from the ledger.
  ["investment-positions"],
  ["investment-summary"],
  // The net-worth HEADLINE (`useNetWorth`) is computed from accounts +
  // manual-assets, so it refreshes via those; the net-worth CHART is the only
  // net-worth *query* and its key is `["networth-history", days]` (one word).
  // The old hand-lists invalidated `["net-worth-history"]` (hyphenated) — a
  // dead key that matched nothing, so the chart went stale after imports/sync.
  ["networth-history"],
  ["account-balance-history"],
  ["account-balance-sparklines"],
  ["agent-status"], // anomaly count is derived from the ledger
  ["financial-health-score"],
];

/** An account was created/edited/rebalanced/deleted, or ownership changed. */
const ACCOUNTS: Root[] = [
  ["accounts"],
  ["account-owners"],
  ["networth-history"], // the net-worth chart; headline recomputes from ["accounts"]
  ["account-balance-history"],
  ["account-balance-sparklines"],
  ["budget-envelopes"], // envelopes can be account-scoped
  ["journey-status"],
  ["financial-health-score"],
];

/** A category was created/renamed/archived/recolored, or its type/guidance changed. */
const CATEGORIES: Root[] = [
  ["categories"],
  ["categories-with-spending"],
  ["spending-breakdown"],
  ["transactions"], // rendered category label/color lives on transaction rows
  ["budget-envelopes"],
  ["recurring"],
  ["rules"], // archiving a category can disable its rules
];

/** A rule was created/toggled/deleted. */
const RULES: Root[] = [["rules"], ["rule-proposals"]];

/** A goal was created/edited/funded/deleted. */
const GOALS: Root[] = [
  ["goals"],
  ["goal-projection"],
  ["journey-status"],
  ["plan-next-month"],
];

/** A budget envelope / allocation changed. */
const BUDGET_ENVELOPES: Root[] = [
  ["budget-envelopes"],
  ["budget-history"],
  ["plan-next-month"],
];

/** A Copilot / agent action bundle was proposed, approved, rejected, or applied. */
const AGENT_ACTIONS: Root[] = [
  ["action-bundles"],
  ["action-bundle"],
  ["action-items"],
  ["execution-log"],
];

/** A Copilot conversation or message changed. */
const COPILOT_CONVERSATION: Root[] = [
  ["conversations"],
  ["conversation-messages"],
  ["agent-sessions"],
];

/** CSV-import prepared/preview/mapping state. */
const IMPORT: Root[] = [
  ["csv-prepare"],
  ["csv-saved-mapping"],
  ["unfinished-imports"],
];

// ── Composite domains ───────────────────────────────────────────────────────

/** Applying an agent action bundle mutates the ledger, so it is agentActions + transactions. */
const AGENT_APPLY: Root[] = [...AGENT_ACTIONS, ...TRANSACTIONS];

/** A SimpleFin connect/sync/disconnect/purge touches the whole ledger + accounts + import state. */
const SIMPLEFIN: Root[] = [...TRANSACTIONS, ...ACCOUNTS, ...IMPORT];

/** A CSV import commit: ledger + accounts (balances) + import state. */
const IMPORT_COMMIT: Root[] = [...TRANSACTIONS, ...ACCOUNTS, ...IMPORT];

export const DOMAIN_KEYS = {
  transactions: TRANSACTIONS,
  accounts: ACCOUNTS,
  categories: CATEGORIES,
  rules: RULES,
  goals: GOALS,
  budgetEnvelopes: BUDGET_ENVELOPES,
  agentActions: AGENT_ACTIONS,
  agentApply: AGENT_APPLY,
  copilotConversation: COPILOT_CONVERSATION,
  import: IMPORT,
  importCommit: IMPORT_COMMIT,
  simplefin: SIMPLEFIN,
} as const;

export type InvalidationDomain = keyof typeof DOMAIN_KEYS;

/** Stable string id for a root, so composite domains dedupe overlapping keys. */
const rootId = (r: Root): string => JSON.stringify(r);

/**
 * Invalidate every derived query affected by the given mutation domain(s).
 * Overlapping roots across domains are invalidated once. Returns the promise so
 * callers can await a settle if they need to (mutations generally don't).
 */
export function invalidateDomains(
  qc: QueryClient,
  ...domains: InvalidationDomain[]
): Promise<void> {
  const seen = new Set<string>();
  const roots: Root[] = [];
  for (const domain of domains) {
    for (const root of DOMAIN_KEYS[domain]) {
      const id = rootId(root);
      if (!seen.has(id)) {
        seen.add(id);
        roots.push(root);
      }
    }
  }
  return Promise.all(
    roots.map((queryKey) => qc.invalidateQueries({ queryKey }))
  ).then(() => undefined);
}

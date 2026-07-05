import { describe, it, expect, vi } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { invalidateDomains, DOMAIN_KEYS } from "./invalidation";

function spyClient() {
  const qc = new QueryClient();
  const calls: string[] = [];
  vi.spyOn(qc, "invalidateQueries").mockImplementation(async (filters?: unknown) => {
    const key = (filters as { queryKey?: unknown[] } | undefined)?.queryKey;
    calls.push(JSON.stringify(key));
  });
  return { qc, calls };
}

describe("invalidateDomains", () => {
  it("invalidates every root of a single domain", async () => {
    const { qc, calls } = spyClient();
    await invalidateDomains(qc, "transactions");
    // A representative spread of the transactions fan-out must all fire.
    for (const root of [
      ["transactions"],
      ["month-totals"],
      ["budget-envelopes"],
      ["net-worth"],
      ["journey-status"],
      ["needs-review-count"],
    ]) {
      expect(calls).toContain(JSON.stringify(root));
    }
  });

  it("distinguishes transactions from transactions-infinite (not a prefix)", async () => {
    const { qc, calls } = spyClient();
    await invalidateDomains(qc, "transactions");
    expect(calls).toContain(JSON.stringify(["transactions"]));
    expect(calls).toContain(JSON.stringify(["transactions-infinite"]));
  });

  it("dedupes overlapping roots across composed domains", async () => {
    const { qc, calls } = spyClient();
    // simplefin = transactions + accounts + import; the shared net-worth/account
    // roots must be invalidated exactly once, not per-domain.
    await invalidateDomains(qc, "simplefin");
    const netWorth = calls.filter((c) => c === JSON.stringify(["net-worth"]));
    expect(netWorth).toHaveLength(1);
    const accounts = calls.filter((c) => c === JSON.stringify(["accounts"]));
    expect(accounts).toHaveLength(1);
  });

  it("multiple domains passed together are unioned and deduped", async () => {
    const { qc, calls } = spyClient();
    await invalidateDomains(qc, "transactions", "categories");
    // categories adds ["categories"], shared ["transactions"] appears once.
    expect(calls).toContain(JSON.stringify(["categories"]));
    expect(calls.filter((c) => c === JSON.stringify(["transactions"]))).toHaveLength(1);
  });

  it("agentApply is a superset of agentActions and the transactions ledger fan-out", async () => {
    const applyIds = new Set(DOMAIN_KEYS.agentApply.map((r) => JSON.stringify(r)));
    for (const r of DOMAIN_KEYS.agentActions) expect(applyIds.has(JSON.stringify(r))).toBe(true);
    for (const r of DOMAIN_KEYS.transactions) expect(applyIds.has(JSON.stringify(r))).toBe(true);
  });

  it("categories does NOT invalidate net-worth (dependency-aware, not refetch-all)", async () => {
    const { qc, calls } = spyClient();
    await invalidateDomains(qc, "categories");
    // A category rename can't change net worth; it must not be dropped.
    expect(calls).not.toContain(JSON.stringify(["net-worth"]));
    expect(calls).not.toContain(JSON.stringify(["accounts"]));
  });
});

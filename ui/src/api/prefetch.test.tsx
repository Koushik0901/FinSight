import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

// Force the desktop runtime gate on so hooks + prefetch actually run.
vi.mock("../utils/runtime", () => ({ isTauriRuntime: () => true, isBackendAvailable: () => true }));

// Spy-able command surface. `vi.hoisted` so the spies exist when the hoisted
// vi.mock factory runs. Each returns the ok-Result shape the hooks unwrap.
const { listAccounts, getMonthTotals } = vi.hoisted(() => ({
  listAccounts: vi.fn(async () => ({ status: "ok", data: [] })),
  getMonthTotals: vi.fn(async () => ({ status: "ok", data: { incomeCents: 0, expenseCents: 0 } })),
}));
vi.mock("./client", async () => {
  const actual = await vi.importActual<typeof import("./client")>("./client");
  return {
    ...actual,
    commands: { listAccounts, getMonthTotals },
  };
});

import { prefetchRoute } from "./prefetch";
import { useAccounts } from "./hooks/accounts";
import { useMonthTotals } from "./hooks/reports";

beforeEach(() => {
  listAccounts.mockClear();
  getMonthTotals.mockClear();
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
});

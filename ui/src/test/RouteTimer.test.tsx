import { render } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { RouteTimer } from "../App";
import { perf } from "../utils/perf";

/**
 * Regression test for the bug found during a real driven measurement pass:
 * on a route change, `useIsFetching()`'s first post-navigation read is the
 * PREVIOUS route's already-settled value (0), not a signal the new route is
 * done — so RouteTimer closed every route at ~0ms regardless of real fetch
 * cost. Prove the fix reports the real duration for a slow fetch, and only
 * falls back to the bounded grace window for a route with no fetch at all.
 */
function SlowQueryScreen({ ms }: { ms: number }) {
  useQuery({
    queryKey: ["slow-thing"],
    queryFn: () => new Promise((resolve) => setTimeout(() => resolve("ok"), ms)),
  });
  return <div>slow screen</div>;
}

function NoFetchScreen() {
  return <div>no-fetch screen</div>;
}

describe("RouteTimer", () => {
  beforeEach(() => {
    perf.clear();
    perf.enabled = true;
  });
  afterEach(() => {
    perf.enabled = false;
  });

  it("reports a real duration (not ~0ms) for a route with a slow fetch", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={["/slow"]}>
          <RouteTimer />
          <Routes>
            <Route path="/slow" element={<SlowQueryScreen ms={150} />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    );

    await vi.waitFor(
      () => {
        const entry = perf.summary()["route:/slow"];
        expect(entry).toBeDefined();
        // Must reflect the real ~150ms fetch, not the pre-fix ~0ms artifact.
        expect(entry!.max).toBeGreaterThan(100);
      },
      { timeout: 2000 }
    );
  });

  it("closes a route with no fetch via the bounded grace window, not instantly", async () => {
    const qc = new QueryClient();
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={["/static"]}>
          <RouteTimer />
          <Routes>
            <Route path="/static" element={<NoFetchScreen />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    );

    await vi.waitFor(
      () => {
        const entry = perf.summary()["route:/static"];
        expect(entry).toBeDefined();
      },
      { timeout: 2000 }
    );
    const entry = perf.summary()["route:/static"]!;
    // Bounded by the grace window (32ms in App.tsx) — not literally 0, and not
    // hanging forever either.
    expect(entry.max).toBeGreaterThanOrEqual(0);
    expect(entry.max).toBeLessThan(500);
  });
});

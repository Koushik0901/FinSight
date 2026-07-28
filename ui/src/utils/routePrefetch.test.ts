import { describe, expect, it, beforeEach, vi } from "vitest";
import { ROUTE_IMPORTS, prefetchRouteChunk, resetPrefetchedRoutesForTest } from "./routePrefetch";

type Importer = () => Promise<unknown>;
import { APP_ROUTES } from "../routes";

describe("route chunk prefetch", () => {
  beforeEach(() => {
    resetPrefetchedRoutesForTest();
  });

  /**
   * The failure this guards is silent: add a screen, wire it into the nav, and
   * hovering it warms the *data* (api/prefetch.ts) while the chunk round-trip
   * stays exactly as slow as before — with nothing to notice, because the app
   * still works.
   *
   * Two deliberate exclusions:
   *  - `/settings/users` is an admin surface nested under `/settings`;
   *    prefetching it for everyone would fetch a chunk non-admins can never
   *    render.
   *  - `/insights` is not a screen at all, it is a `<Navigate>` redirect to
   *    `/inbox` in App.tsx, so it has no chunk of its own to warm.
   */
  const NO_CHUNK_OF_THEIR_OWN = ["/settings/users", "/insights"];

  it("maps every linkable app route to a chunk importer", () => {
    const missing = APP_ROUTES.filter(
      (route) => !NO_CHUNK_OF_THEIR_OWN.includes(route) && !(route in ROUTE_IMPORTS)
    );
    expect(missing).toEqual([]);
  });

  it("does not invent routes the app does not have", () => {
    const unknown = Object.keys(ROUTE_IMPORTS).filter(
      (path) => !(APP_ROUTES as readonly string[]).includes(path)
    );
    expect(unknown).toEqual([]);
  });

  /**
   * Swap an importer for the duration of one test and always put it back —
   * `ROUTE_IMPORTS` is module state shared across every case in the file.
   */
  function withStubbedImporter(path: string, stub: Importer, body: () => void) {
    const original = ROUTE_IMPORTS[path] as Importer;
    ROUTE_IMPORTS[path] = stub;
    try {
      body();
    } finally {
      ROUTE_IMPORTS[path] = original;
    }
  }

  it("starts the import once, however many times intent is signalled", () => {
    const load = vi.fn().mockResolvedValue({});

    withStubbedImporter("/goals", load, () => {
      prefetchRouteChunk("/goals");
      prefetchRouteChunk("/goals");
      prefetchRouteChunk("/goals");
    });

    expect(load).toHaveBeenCalledTimes(1);
  });

  it("ignores an unknown path instead of throwing", () => {
    expect(() => prefetchRouteChunk("/not-a-screen")).not.toThrow();
  });

  /**
   * A prefetch is speculative work the user never asked for. If it rejects,
   * the real navigation should own the error (the route error boundary can act
   * on it) — an unhandled rejection here would just be console noise. The path
   * must also become retryable, or one flaky hover would poison it forever.
   */
  it("swallows a failed prefetch and allows a later retry", async () => {
    const load = vi.fn().mockRejectedValue(new Error("network down"));
    const original = ROUTE_IMPORTS["/reports"] as Importer;
    ROUTE_IMPORTS["/reports"] = load;

    try {
      expect(() => prefetchRouteChunk("/reports")).not.toThrow();
      await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(1));

      // The rejection must clear the "already started" mark, or a single flaky
      // hover would poison this route for the rest of the session.
      prefetchRouteChunk("/reports");
      await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    } finally {
      ROUTE_IMPORTS["/reports"] = original;
    }
  });
});

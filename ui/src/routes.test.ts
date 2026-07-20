import { describe, expect, it } from "vitest";
// `?raw` gives us App.tsx's source as a string, the same trick the service
// worker tests use. Parsing the source is deliberate: rendering App would pull
// in every screen, and we only care about which paths are declared.
import appSource from "./App.tsx?raw";
import { APP_ROUTES, NON_LINKABLE_ROUTES } from "./routes";

/**
 * Three files have to agree on what a route is: `App.tsx` (what actually
 * renders), `routes.ts` (what the frontend believes), and
 * `crates/finsight-core/src/routes.rs` (what the backend emits and validates
 * against).
 *
 * This file pins App.tsx ↔ routes.ts. The routes.ts ↔ routes.rs half is pinned
 * from the Rust side, in `routes.rs`'s `ts_mirror_matches_the_rust_registry`,
 * because reading across into `crates/` from here would mean escaping the
 * Vite root.
 *
 * Without these pins, adding a screen silently leaves the backend unable to
 * link to it, and removing one silently turns every backend link into a dead
 * end.
 */

/** Every `path="…"` declared on a `<Route>` in App.tsx. */
function declaredRoutes(): string[] {
  return [...appSource.matchAll(/<Route\s+path="([^"]+)"/g)]
    .map((match) => match[1])
    .filter((path): path is string => Boolean(path));
}

describe("route registry", () => {
  it("covers every route App.tsx declares", () => {
    const declared = declaredRoutes();
    // Sanity: the regex actually found the routes block.
    expect(declared.length).toBeGreaterThan(15);

    const accounted = new Set<string>([...APP_ROUTES, ...NON_LINKABLE_ROUTES]);
    const unaccounted = declared.filter((path) => !accounted.has(path));
    expect(
      unaccounted,
      "a route was added to App.tsx without deciding whether the backend may link to it — " +
        "add it to APP_ROUTES (and routes.rs) or to NON_LINKABLE_ROUTES",
    ).toEqual([]);
  });

  it("does not claim routes App.tsx no longer renders", () => {
    const declared = new Set(declaredRoutes());
    const stale = APP_ROUTES.filter((path) => !declared.has(path));
    expect(
      stale,
      "these routes are advertised to the backend but no longer exist in App.tsx",
    ).toEqual([]);
  });

  it("holds only absolute, parameter-free paths", () => {
    for (const path of APP_ROUTES) {
      expect(path.startsWith("/"), `${path} must be absolute`).toBe(true);
      expect(path.includes(":"), `${path} must not be parameterised`).toBe(false);
      if (path !== "/") {
        expect(path.endsWith("/"), `${path} must not have a trailing slash`).toBe(false);
      }
    }
  });

  it("has no duplicates", () => {
    expect(new Set(APP_ROUTES).size).toBe(APP_ROUTES.length);
  });
});

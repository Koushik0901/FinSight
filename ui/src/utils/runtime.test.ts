import { describe, it, expect, afterEach, beforeEach, vi } from "vitest";
import { isTauriRuntime } from "./runtime";

const realLocation = window.location;

// isTauriRuntime() short-circuits to `true` under vitest (MODE === "test" /
// VITEST truthy) and again under jsdom (navigator.userAgent contains
// "jsdom") — both checks sit ABOVE the bridge/origin logic this suite exists
// to exercise, and per Task 6 that short-circuit is NOT to be touched. So to
// actually reach the origin-aware branch from inside a vitest+jsdom test, we
// have to defeat both short-circuits by stubbing env + navigator to look like
// a real (non-test) browser. This mirrors what the shipped code path sees in
// production; it does not change isTauriRuntime() itself.
beforeEach(() => {
  vi.stubEnv("MODE", "production");
  vi.stubEnv("VITEST", "");
  vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)" });
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  Object.defineProperty(window, "location", { value: realLocation, configurable: true });
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

function setLocation(origin: string) {
  Object.defineProperty(window, "location", { value: { origin }, configurable: true });
}

describe("isTauriRuntime — origin awareness (Phase 4)", () => {
  it("true when the bridge is present AND on Tauri's own internal origin (mac/linux)", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("tauri://localhost");
    expect(isTauriRuntime()).toBe(true);
  });
  it("true on Tauri's Windows-default internal origin", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("http://tauri.localhost");
    expect(isTauriRuntime()).toBe(true);
  });
  it("true on Tauri's Windows https-scheme internal origin", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("https://tauri.localhost");
    expect(isTauriRuntime()).toBe(true);
  });
  it("FALSE when the bridge is present but the origin is a remote self-hosted server — " +
     "this is the exact Phase 4 shell-after-navigate scenario", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("https://myhost.ts.net");
    expect(isTauriRuntime()).toBe(false);
  });
  it("false when the bridge is absent regardless of origin", () => {
    setLocation("tauri://localhost");
    expect(isTauriRuntime()).toBe(false);
  });
  it("true in DEV when the bridge is present and on Vite's dev-server origin " +
     "(pnpm tauri:dev navigates the real desktop webview there for HMR)", () => {
    vi.stubEnv("DEV", true);
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("http://localhost:5173");
    expect(isTauriRuntime()).toBe(true);
  });
  it("false on localhost:5173 outside DEV — a production build must not false-positive " +
     "just because a user's self-hosted server happens to run on that port", () => {
    // Vitest's own import.meta.env.DEV defaults to true (it runs in a
    // dev-like mode), so this test must explicitly simulate a production
    // build's DEV=false to exercise the branch a real prod bundle takes.
    vi.stubEnv("DEV", false);
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("http://localhost:5173");
    expect(isTauriRuntime()).toBe(false);
  });
});

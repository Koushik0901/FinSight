import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Provide a default mock for tauri invoke so Vitest doesn't error on import.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (_cmd: string, _args?: unknown) => {
    throw new Error("invoke not mocked — set per-test with vi.mocked(invoke).mockResolvedValue(...)");
  }),
}));

// Provide a no-op mock for tauri event listeners (used by ImportProgress).
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  once: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// jsdom does not implement IntersectionObserver; stub it so components using
// scroll-spy / visibility observers (e.g. Settings sidebar nav) don't throw.
class MockIntersectionObserver implements IntersectionObserver {
  readonly root: Element | Document | null = null;
  readonly rootMargin: string = "";
  readonly thresholds: ReadonlyArray<number> = [];
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
  takeRecords = vi.fn(() => []);
}
vi.stubGlobal("IntersectionObserver", MockIntersectionObserver);

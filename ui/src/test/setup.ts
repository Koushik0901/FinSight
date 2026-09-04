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

// jsdom does not implement ResizeObserver; stub it so Recharts'
// <ResponsiveContainer> (used by copilot/charts/FinSightChart) doesn't throw
// on mount. It renders at width:0 either way — tests assert on labeled text,
// not on measured pixel geometry.
class MockResizeObserver implements ResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
vi.stubGlobal("ResizeObserver", MockResizeObserver);

// jsdom does not implement window.matchMedia. The app uses it in two places
// that tests render: useIsMobile (App shell — phone vs desktop layout) and
// Drawer's retained-mount exit animation (mobile 200ms vs desktop 180ms). A
// matches:false default keeps tests on the desktop/longer-duration path, same
// as jsdom's CSS-less behavior elsewhere. The listener surface is stubbed so
// hook subscriptions (addEventListener("change")) don't throw.
class MockMediaQueryList extends EventTarget {
  readonly media: string;
  readonly matches = false;
  readonly onchange: MediaQueryList["onchange"] = null;
  constructor(query: string) {
    super();
    this.media = query;
  }
  addListener = vi.fn();
  removeListener = vi.fn();
}
vi.stubGlobal(
  "matchMedia",
  vi.fn((query: string) => new MockMediaQueryList(query))
);

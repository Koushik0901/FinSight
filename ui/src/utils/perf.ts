import type { QueryCache } from "@tanstack/react-query";

/**
 * Lightweight, exportable perf instrumentation for real-desktop measurement.
 *
 * It records two things the Phase 7B acceptance criteria ask for — per-query
 * latency (filter/search/account-open/report/insight loads) and route
 * navigation-to-content — into an in-memory ring buffer that is also mirrored to
 * `window.__finsightPerf`. A measurement run (the user driving the packaged
 * Tauri app, or an automated computer-use pass) can read/export it: the app
 * self-instruments, so no external profiler is needed.
 *
 * Turn on by setting `localStorage.finsightPerf = "1"` (or `?perf=1`); off by
 * default so there is zero overhead in normal use. Enable BEFORE reload.
 */

export interface PerfEntry {
  kind: "query" | "route";
  label: string;
  ms: number;
  detail?: string;
  at: number;
}

const RING = 500;

function perfEnabled(): boolean {
  try {
    if (typeof window === "undefined") return false;
    if (new URLSearchParams(window.location.search).get("perf") === "1") return true;
    return window.localStorage?.getItem("finsightPerf") === "1";
  } catch {
    return false;
  }
}

interface PerfSink {
  enabled: boolean;
  entries: PerfEntry[];
  record(e: PerfEntry): void;
  clear(): void;
  export(): string;
  summary(): Record<string, { count: number; p50: number; p95: number; max: number }>;
}

function makeSink(): PerfSink {
  const entries: PerfEntry[] = [];
  const percentile = (xs: number[], p: number): number => {
    if (xs.length === 0) return 0;
    const s = [...xs].sort((a, b) => a - b);
    return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))] ?? 0;
  };
  return {
    enabled: perfEnabled(),
    entries,
    record(e) {
      entries.push(e);
      if (entries.length > RING) entries.shift();
      // Console breadcrumb so a live measurement pass sees it without exporting.
      // eslint-disable-next-line no-console
      console.info(`[perf] ${e.kind} ${e.label} ${e.ms.toFixed(1)}ms${e.detail ? ` (${e.detail})` : ""}`);
    },
    clear() {
      entries.length = 0;
    },
    export() {
      return entries.map((e) => JSON.stringify(e)).join("\n");
    },
    summary() {
      const byLabel = new Map<string, number[]>();
      for (const e of entries) {
        const k = `${e.kind}:${e.label}`;
        (byLabel.get(k) ?? byLabel.set(k, []).get(k)!).push(e.ms);
      }
      const out: Record<string, { count: number; p50: number; p95: number; max: number }> = {};
      for (const [k, xs] of byLabel) {
        out[k] = {
          count: xs.length,
          p50: Math.round(percentile(xs, 50)),
          p95: Math.round(percentile(xs, 95)),
          max: Math.round(Math.max(...xs)),
        };
      }
      return out;
    },
  };
}

export const perf: PerfSink = makeSink();

if (typeof window !== "undefined") {
  (window as unknown as { __finsightPerf?: PerfSink }).__finsightPerf = perf;
}

/**
 * Subscribe to a QueryCache and record every fetch's wall-clock duration, tagged
 * with the query-key root and whether it was served from cache (duration ~0 =
 * a prefetch hit). Call once at app startup with the app's QueryClient's cache.
 */
export function instrumentQueryCache(cache: QueryCache): void {
  if (!perf.enabled) return;
  const startedAt = new Map<string, number>();
  cache.subscribe((event) => {
    const query = event.query;
    const hash = query.queryHash;
    const root = Array.isArray(query.queryKey) ? String(query.queryKey[0]) : String(query.queryKey);
    if (event.type === "updated") {
      const action = (event as { action?: { type?: string } }).action;
      if (action?.type === "fetch") {
        startedAt.set(hash, performance.now());
      } else if (action?.type === "success" || action?.type === "error") {
        const start = startedAt.get(hash);
        if (start !== undefined) {
          perf.record({
            kind: "query",
            label: root,
            ms: performance.now() - start,
            detail: action.type === "error" ? "error" : undefined,
            at: Date.now(),
          });
          startedAt.delete(hash);
        }
      }
    }
  });
}

let routeStart: { path: string; t: number } | null = null;

/** Call when a route navigation begins (path is the destination). */
export function markRouteStart(path: string): void {
  if (!perf.enabled) return;
  routeStart = { path, t: performance.now() };
}

/**
 * Call when the destination route has painted useful content (its primary
 * queries are no longer loading). Records nav-intent→content for `path`.
 */
export function markRouteContent(path: string): void {
  if (!perf.enabled || !routeStart || routeStart.path !== path) return;
  perf.record({ kind: "route", label: path, ms: performance.now() - routeStart.t, at: Date.now() });
  routeStart = null;
}

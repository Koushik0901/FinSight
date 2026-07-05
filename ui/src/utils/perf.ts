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
 * Turn on by setting `localStorage.finsightPerf = "1"` (or `?perf=1`) before
 * reload, OR at runtime with `Ctrl+Alt+P` (no reload, no devtools needed —
 * release builds ship without devtools). `Ctrl+Alt+S` copies `summary()` to
 * the clipboard as JSON; `Ctrl+Alt+E` copies the raw `export()` (one JSON
 * object per line, in recording order) so a driven measurement pass can read
 * either out without a console. All three are wired in `App.tsx`'s global
 * keydown handler and confirm via a toast. Off by default so there is zero
 * overhead in normal use.
 *
 * `summary()` is built for small desktop measurement runs, where a label often
 * has only 1-2 samples (e.g. "visit this route once cold, once warm"): it
 * reports `min`/`max`/`first`/`last` unconditionally (each is a single real
 * sample, always meaningful), `p50` as the median, and `p95` ONLY when there
 * are enough samples for a 95th-percentile estimate to mean anything —
 * otherwise it's `null` rather than a number that silently degenerates to
 * `max` (see `MIN_SAMPLES_FOR_P95`). `first`/`last` are chronological (not
 * sorted), so a cold-vs-warm or before-vs-after run can be compared directly
 * without the percentile math obscuring which sample was which. `export()`
 * preserves every raw sample for a fuller side-by-side diff.
 */

export interface PerfEntry {
  kind: "query" | "route";
  label: string;
  ms: number;
  detail?: string;
  at: number;
}

const RING = 500;

/**
 * Minimum sample count before a p95 estimate is reported as a number. Below
 * this, `summary()` reports `p95: null` — with e.g. 2 samples, "the 95th
 * percentile" is indistinguishable from `max` (see the index-based percentile
 * formula below) and asserting a precise-looking number would misrepresent a
 * single data point as a distribution. 20 is the common rule-of-thumb floor
 * for a percentile this high to mean anything at all (it's about behavior in
 * the top 5%, which needs a meaningful few samples above the p50 to exist).
 */
const MIN_SAMPLES_FOR_P95 = 20;

function perfEnabled(): boolean {
  try {
    if (typeof window === "undefined") return false;
    if (new URLSearchParams(window.location.search).get("perf") === "1") return true;
    return window.localStorage?.getItem("finsightPerf") === "1";
  } catch {
    return false;
  }
}

/**
 * Per-label stats. `p95` is `null` when `count < MIN_SAMPLES_FOR_P95` — never
 * a number standing in for "not enough data." `first`/`last` are the
 * chronologically first/most-recent sample still in the ring buffer (NOT
 * sorted), so a cold-then-warm or before-then-after run can be read directly
 * off two fields instead of reverse-engineered from a collapsed percentile.
 */
export interface PerfStat {
  count: number;
  min: number;
  p50: number;
  p95: number | null;
  max: number;
  first: number;
  last: number;
}

interface PerfSink {
  enabled: boolean;
  entries: PerfEntry[];
  record(e: PerfEntry): void;
  clear(): void;
  export(): string;
  summary(): Record<string, PerfStat>;
  toggle(): boolean;
  copySummaryToClipboard(): Promise<void>;
  copyExportToClipboard(): Promise<void>;
}

function makeSink(): PerfSink {
  const entries: PerfEntry[] = [];
  const percentile = (xs: number[], p: number): number => {
    if (xs.length === 0) return 0;
    const s = [...xs].sort((a, b) => a - b);
    return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))] ?? 0;
  };
  const sink: PerfSink = {
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
      // Group in recording order — `chrono` is never sorted, so `first`/`last`
      // stay meaningful for a cold-vs-warm or before-vs-after comparison.
      const byLabel = new Map<string, number[]>();
      for (const e of entries) {
        const k = `${e.kind}:${e.label}`;
        (byLabel.get(k) ?? byLabel.set(k, []).get(k)!).push(e.ms);
      }
      const out: Record<string, PerfStat> = {};
      for (const [k, chrono] of byLabel) {
        out[k] = {
          count: chrono.length,
          min: Math.round(Math.min(...chrono)),
          p50: Math.round(percentile(chrono, 50)),
          p95: chrono.length >= MIN_SAMPLES_FOR_P95 ? Math.round(percentile(chrono, 95)) : null,
          max: Math.round(Math.max(...chrono)),
          first: Math.round(chrono[0]!),
          last: Math.round(chrono[chrono.length - 1]!),
        };
      }
      return out;
    },
    toggle() {
      sink.enabled = !sink.enabled;
      return sink.enabled;
    },
    async copySummaryToClipboard() {
      const payload = JSON.stringify(sink.summary(), null, 2);
      await navigator.clipboard.writeText(payload);
    },
    async copyExportToClipboard() {
      await navigator.clipboard.writeText(sink.export());
    },
  };
  return sink;
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
  // Subscribe unconditionally (cheap: a Map + a type check per cache event)
  // and gate recording per-event on the CURRENT value of `perf.enabled`, so
  // toggling at runtime (Ctrl+Alt+P) takes effect immediately with no reload.
  const startedAt = new Map<string, number>();
  cache.subscribe((event) => {
    if (!perf.enabled) return;
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

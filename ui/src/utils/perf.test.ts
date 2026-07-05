import { describe, it, expect, beforeEach, vi } from "vitest";
import { perf } from "./perf";

describe("perf sink — runtime toggle + clipboard export", () => {
  beforeEach(() => {
    perf.clear();
    perf.enabled = false;
  });

  it("toggle() flips enabled and returns the new value", () => {
    expect(perf.toggle()).toBe(true);
    expect(perf.enabled).toBe(true);
    expect(perf.toggle()).toBe(false);
    expect(perf.enabled).toBe(false);
  });

  it("copySummaryToClipboard writes the current summary as JSON", async () => {
    perf.record({ kind: "route", label: "/", ms: 42, at: Date.now() });
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    await perf.copySummaryToClipboard();
    expect(writeText).toHaveBeenCalledTimes(1);
    const written = JSON.parse(writeText.mock.calls[0]?.[0]);
    expect(written["route:/"]).toMatchObject({ count: 1, p50: 42 });
  });

  it("copyExportToClipboard writes every raw sample (not just the summary)", async () => {
    perf.record({ kind: "route", label: "/a", ms: 10, at: 1000 });
    perf.record({ kind: "route", label: "/a", ms: 20, at: 2000 });
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    await perf.copyExportToClipboard();
    const written = writeText.mock.calls[0]?.[0] as string;
    const lines = written.trim().split("\n");
    expect(lines).toHaveLength(2);
    expect(JSON.parse(lines[0]!)).toMatchObject({ ms: 10, at: 1000 });
    expect(JSON.parse(lines[1]!)).toMatchObject({ ms: 20, at: 2000 });
  });
});

describe("perf.summary() — statistics that stay honest at small sample sizes", () => {
  beforeEach(() => {
    perf.clear();
    perf.enabled = true;
  });

  it("n=1: min/p50/max/first/last all equal the single sample; p95 is null", () => {
    perf.record({ kind: "route", label: "/solo", ms: 77, at: Date.now() });
    const s = perf.summary()["route:/solo"]!;
    expect(s.count).toBe(1);
    expect(s.min).toBe(77);
    expect(s.p50).toBe(77);
    expect(s.max).toBe(77);
    expect(s.first).toBe(77);
    expect(s.last).toBe(77);
    expect(s.p95).toBeNull();
  });

  it("n=2: first/last preserve which sample was cold vs warm; p95 stays null", () => {
    // A cold-then-warm nav pair — the exact case a collapsing percentile
    // formula would otherwise obscure (see the Phase 7B report's route:/reports
    // caveat: p50/p95/max all read the same at n=2 under the old formula).
    perf.record({ kind: "route", label: "/reports", ms: 132, at: 1000 }); // cold
    perf.record({ kind: "route", label: "/reports", ms: 9, at: 2000 }); // warm revisit
    const s = perf.summary()["route:/reports"]!;
    expect(s.count).toBe(2);
    expect(s.first).toBe(132); // the cold visit, recorded first
    expect(s.last).toBe(9); // the warm revisit, recorded second
    expect(s.min).toBe(9);
    expect(s.max).toBe(132);
    // p95 is not meaningful with only 2 samples.
    expect(s.p95).toBeNull();
  });

  it("small n (10, below the p95 floor): p95 is still null even though min/p50/max are real", () => {
    const values = [5, 8, 10, 12, 15, 18, 20, 25, 30, 100];
    for (const ms of values) perf.record({ kind: "query", label: "small", ms, at: Date.now() });
    const s = perf.summary()["query:small"]!;
    expect(s.count).toBe(10);
    expect(s.min).toBe(5);
    expect(s.max).toBe(100);
    expect(s.first).toBe(5);
    expect(s.last).toBe(100);
    expect(s.p50).toBeGreaterThan(0);
    expect(s.p95).toBeNull();
  });

  it("normal larger sample set (100): p95 is a real number, distinct from max, and count crosses the floor exactly", () => {
    // 1..100 ms — p95 should land near 95, well below the max of 100, and
    // strictly greater than p50 (~50).
    const values = Array.from({ length: 100 }, (_, i) => i + 1);
    for (const ms of values) perf.record({ kind: "query", label: "big", ms, at: Date.now() });
    const s = perf.summary()["query:big"]!;
    expect(s.count).toBe(100);
    expect(s.min).toBe(1);
    expect(s.max).toBe(100);
    expect(s.first).toBe(1);
    expect(s.last).toBe(100);
    expect(s.p95).not.toBeNull();
    expect(s.p95!).toBeGreaterThan(s.p50);
    expect(s.p95!).toBeLessThan(s.max);
  });

  it("the p95 floor is exact: 19 samples -> null, 20 samples -> a number", () => {
    for (let i = 1; i <= 19; i++) {
      perf.record({ kind: "query", label: "boundary-below", ms: i, at: Date.now() });
    }
    expect(perf.summary()["query:boundary-below"]!.p95).toBeNull();

    for (let i = 1; i <= 20; i++) {
      perf.record({ kind: "query", label: "boundary-at", ms: i, at: Date.now() });
    }
    expect(perf.summary()["query:boundary-at"]!.p95).not.toBeNull();
  });

  it("labels are independent: one label's sample count doesn't affect another's p95 gating", () => {
    perf.record({ kind: "route", label: "/one", ms: 5, at: Date.now() });
    for (let i = 1; i <= 25; i++) {
      perf.record({ kind: "route", label: "/many", ms: i, at: Date.now() });
    }
    const s = perf.summary();
    expect(s["route:/one"]!.p95).toBeNull();
    expect(s["route:/many"]!.p95).not.toBeNull();
  });
});

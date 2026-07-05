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

  it("summary() aggregates count/p50/p95/max per kind:label", () => {
    for (const ms of [10, 20, 30, 40, 100]) {
      perf.record({ kind: "query", label: "accounts", ms, at: Date.now() });
    }
    const s = perf.summary()["query:accounts"];
    expect(s).toBeDefined();
    expect(s?.count).toBe(5);
    expect(s?.max).toBe(100);
    expect(s?.p50).toBeGreaterThan(0);
  });
});

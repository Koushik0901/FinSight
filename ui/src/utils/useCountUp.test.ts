import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCountUp } from "./useCountUp";

const reduceQuery = "(prefers-reduced-motion: reduce)";

function stubReducedMotion(matches: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query === reduceQuery ? matches : false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useCountUp", () => {
  it("returns null while the target is null", () => {
    stubReducedMotion(true);
    const { result } = renderHook(() => useCountUp(null));
    expect(result.current).toBeNull();
  });

  it("snaps to the target instantly under prefers-reduced-motion", () => {
    stubReducedMotion(true);
    const { result, rerender } = renderHook(({ v }: { v: number | null }) => useCountUp(v), {
      initialProps: { v: 100 },
    });
    expect(result.current).toBe(100);
    act(() => rerender({ v: 250 }));
    expect(result.current).toBe(250);
  });

  it("snaps to the target instantly when requestAnimationFrame is unavailable", () => {
    stubReducedMotion(false);
    vi.stubGlobal("requestAnimationFrame", undefined);
    const { result, rerender } = renderHook(({ v }: { v: number | null }) => useCountUp(v), {
      initialProps: { v: 100 },
    });
    act(() => rerender({ v: 250 }));
    expect(result.current).toBe(250);
  });

  it("tweens to the target and settles on it when rAF is available", () => {
    stubReducedMotion(false);
    // performance.now drives the easing clock — fake it alongside the rAF timers.
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => setTimeout(() => cb(performance.now()), 16));
    vi.stubGlobal("cancelAnimationFrame", (id: number) => clearTimeout(id));
    try {
      const { result, rerender } = renderHook(({ v }: { v: number | null }) => useCountUp(v), {
        initialProps: { v: 0 },
      });
      expect(result.current).toBe(0);
      act(() => rerender({ v: 500 }));
      // Mid-roll it must be part-way, not jumped straight to the target.
      act(() => {
        vi.advanceTimersByTime(80);
      });
      expect(result.current).toBeGreaterThan(0);
      expect(result.current).toBeLessThan(500);
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      expect(result.current).toBe(500);
    } finally {
      vi.useRealTimers();
    }
  });
});

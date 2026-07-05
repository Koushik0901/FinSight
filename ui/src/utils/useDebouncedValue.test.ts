import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDebouncedValue } from "./useDebouncedValue";

describe("useDebouncedValue", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("returns the initial value immediately", () => {
    const { result } = renderHook(() => useDebouncedValue("a", 250));
    expect(result.current).toBe("a");
  });

  it("defers updates until the delay elapses", () => {
    const { result, rerender } = renderHook(({ v }) => useDebouncedValue(v, 250), {
      initialProps: { v: "a" },
    });
    rerender({ v: "ab" });
    expect(result.current).toBe("a"); // not yet
    act(() => vi.advanceTimersByTime(250));
    expect(result.current).toBe("ab");
  });

  it("collapses a burst of changes into a single trailing update", () => {
    const { result, rerender } = renderHook(({ v }) => useDebouncedValue(v, 250), {
      initialProps: { v: "a" },
    });
    // Rapid keystrokes, each faster than the delay — only the last should land.
    for (const v of ["ab", "abc", "abcd"]) {
      rerender({ v });
      act(() => vi.advanceTimersByTime(100));
    }
    expect(result.current).toBe("a"); // still debouncing through the burst
    act(() => vi.advanceTimersByTime(250));
    expect(result.current).toBe("abcd");
  });
});

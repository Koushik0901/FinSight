import { describe, expect, it } from "vitest";
import { PLOT_DURATION_MS, PLOT_EASING, plotAnim } from "./plotMotion";

describe("plotAnim", () => {
  it("defaults to the house expo easing, 850ms, active, no delay", () => {
    expect(plotAnim()).toEqual({
      isAnimationActive: true,
      animationDuration: 850,
      // Mirrors the --ease-out-expo token; react-smooth accepts cubic-bezier strings.
      animationEasing: "cubic-bezier(0.16, 1, 0.3, 1)",
      animationBegin: 0,
    });
  });

  it("disables animation under prefers-reduced-motion", () => {
    expect(plotAnim({ reduced: true }).isAnimationActive).toBe(false);
    expect(plotAnim({ reduced: false }).isAnimationActive).toBe(true);
  });

  it("honours begin/duration overrides for multi-series stagger", () => {
    expect(plotAnim({ begin: 140, duration: 400 })).toMatchObject({ animationBegin: 140, animationDuration: 400 });
  });

  it("exposes the easing constant used by CSS-adjacent config", () => {
    expect(PLOT_EASING).toBe("cubic-bezier(0.16, 1, 0.3, 1)");
    expect(PLOT_DURATION_MS).toBe(850);
  });
});

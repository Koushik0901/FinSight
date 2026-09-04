import { useEffect, useMemo, useState } from "react";

/**
 * Shared motion config for every plot family.
 *
 * Recharts animates its geometry through react-smooth, which accepts CSS
 * `cubic-bezier(...)` strings at runtime even though its `animationEasing`
 * prop type only lists named easings — verified in react-smooth@4
 * `src/easing.js`. The cast below exists once so plot geometry can ride the
 * house `--ease-out-expo` token instead of a second, softer easing.
 */
type NamedEasing = "ease" | "ease-in" | "ease-out" | "ease-in-out" | "linear";
export const PLOT_EASING = "cubic-bezier(0.16, 1, 0.3, 1)";

/** Geometry morph length (mount draw-in + data changes). */
export const PLOT_DURATION_MS = 850;

const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";

/** Live `prefers-reduced-motion` flag. */
export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () => typeof window !== "undefined" && window.matchMedia?.(REDUCED_QUERY)?.matches === true,
  );

  useEffect(() => {
    const mql = window.matchMedia?.(REDUCED_QUERY);
    if (!mql) return;
    const onChange = (e: MediaQueryListEvent) => setReduced(e.matches);
    setReduced(mql.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  return reduced;
}

export type PlotAnim = {
  isAnimationActive: boolean;
  animationDuration: number;
  animationEasing: NamedEasing;
  animationBegin: number;
};

/**
 * Recharts animation props for one series/chart. `begin` staggers
 * multi-series charts (later series start after the first has led).
 */
export function plotAnim(opts: { reduced?: boolean; begin?: number; duration?: number } = {}): PlotAnim {
  const { reduced = false, begin = 0, duration = PLOT_DURATION_MS } = opts;
  return {
    isAnimationActive: !reduced,
    animationDuration: duration,
    animationEasing: PLOT_EASING as NamedEasing,
    animationBegin: begin,
  };
}

/** Hook form of {@link plotAnim}: spread on a `<Bar>`, `<Line>`, `<Area>`, `<Pie>`… */
export function usePlotAnim(opts: { begin?: number; duration?: number } = {}): PlotAnim {
  const reduced = useReducedMotion();
  const { begin = 0, duration = PLOT_DURATION_MS } = opts;
  return useMemo(() => plotAnim({ reduced, begin, duration }), [reduced, begin, duration]);
}

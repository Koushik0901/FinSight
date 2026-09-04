import { useEffect, useRef, useState } from "react";
import { useReducedMotion } from "./plotMotion";

/** --ease-out-expo, as a unit-interval function. */
const easeOutExpo = (t: number): number => (t >= 1 ? 1 : 1 - 2 ** (-10 * t));

/**
 * Tweens from the last settled value to `target` with an expo-out ease.
 *
 * - Returns `null` for a null/undefined target (render your placeholder).
 * - Skips animation under `prefers-reduced-motion` or when `requestAnimationFrame`
 *   is unavailable (SSR/jsdom) — value snaps to target, keeping tests deterministic.
 * - Mounts settle instantly from the initial value; only subsequent changes tween.
 */
export function useCountUp(target: number | null | undefined, durationMs = 850): number | null {
  const reduced = useReducedMotion();
  const [display, setDisplay] = useState<number | null>(target ?? null);
  const lastRef = useRef<number | null>(target ?? null);
  const rafRef = useRef(0);

  useEffect(() => {
    if (target == null) {
      setDisplay(null);
      lastRef.current = null;
      return;
    }
    if (reduced || typeof requestAnimationFrame !== "function") {
      setDisplay(target);
      lastRef.current = target;
      return;
    }
    const from = lastRef.current ?? target;
    if (from === target) {
      setDisplay(target);
      return;
    }
    const start = performance.now();
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / durationMs);
      const value = from + (target - from) * easeOutExpo(t);
      lastRef.current = value;
      setDisplay(value);
      if (t < 1) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        lastRef.current = target;
      }
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [target, durationMs, reduced]);

  return display;
}

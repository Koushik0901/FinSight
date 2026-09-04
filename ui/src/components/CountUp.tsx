import { useCountUp } from "../utils/useCountUp";

type Props = {
  /** Numeric target; null/undefined renders empty and skips the tween. */
  value: number | null | undefined;
  format?: (value: number) => string;
  className?: string;
  durationMs?: number;
};

/**
 * Number that rolls to `value` on change (expo-out, ~500ms) instead of
 * snapping — used for the figures that sit beside live plots, so totals and
 * stats settle in sync with the chart geometry. Honors prefers-reduced-motion
 * and renders final values instantly in test environments (no rAF).
 */
export default function CountUp({ value, format = String, className, durationMs }: Props) {
  const display = useCountUp(value, durationMs);
  return <span className={className}>{display == null ? "" : format(display)}</span>;
}

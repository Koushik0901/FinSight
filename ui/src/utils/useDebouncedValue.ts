import { useEffect, useState } from "react";

/**
 * Returns a debounced copy of `value` that only updates after `delayMs` have
 * elapsed without a further change. Use it to keep an input instantly
 * responsive (bind the raw value to the control) while deferring expensive
 * downstream work — a backend query, a filter recompute — until typing settles.
 *
 * Superseded changes are discarded: each new value cancels the pending timer,
 * so only the final value in a burst reaches consumers. This collapses a
 * keystroke storm into a single trailing update.
 */
export function useDebouncedValue<T>(value: T, delayMs = 300): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(t);
  }, [value, delayMs]);
  return debounced;
}

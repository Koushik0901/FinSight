import { useTweaks } from "../state/tweaks";

interface MoneyOpts {
  /** Fraction digits (both min and max). Default 0. */
  decimals?: number;
  /** ISO currency code. Default: user's configured currency, then "USD". */
  currency?: string;
}

/**
 * Format a cent amount as currency. Defaults to the user's configured
 * currency (from zustand store), 0 decimal places, comma-grouped.
 * Pass `{ decimals: 2 }` for cent precision, `{ currency }` to override.
 */
export function money(cents: number, opts: MoneyOpts = {}): string {
  const decimals = opts.decimals ?? 0;
  const currency = opts.currency ?? useTweaks.getState().currency ?? "USD";
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(cents / 100);
}

/**
 * Compact currency for chart callouts: "$137.5K", "-CA$1.2M", "$482".
 * Same currency resolution as `money`. Below $1,000 there's nothing to
 * abbreviate, so this drops to the same 0-decimal precision as `money()` —
 * otherwise a headline stat and a chart callout for the exact same value
 * can disagree (e.g. "-$69" vs "-$68.6") purely from rounding, which reads
 * as a data bug even when the numbers are identical.
 */
export function compactMoney(cents: number, opts: Pick<MoneyOpts, "currency"> = {}): string {
  const currency = opts.currency ?? useTweaks.getState().currency ?? "USD";
  const abs = Math.abs(cents / 100);
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    notation: "compact",
    maximumFractionDigits: abs >= 1000 ? 1 : 0,
  }).format(cents / 100);
}

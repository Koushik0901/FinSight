import { useTweaks } from "../state/tweaks";

interface MoneyOpts {
  /** Fraction digits (both min and max). Default 0. */
  decimals?: number;
  /** ISO currency code. Default: user's configured currency, then "USD". */
  currency?: string;
}

/**
 * `Intl.NumberFormat` construction is expensive (tens of µs) and money() runs
 * per table cell on every render, so instances are cached by their resolved
 * options — the key space is tiny (a handful of currencies × a handful of
 * decimals/notation combos), never unbounded.
 */
const numberFormatCache = new Map<string, Intl.NumberFormat>();

function getNumberFormat(currency: string | null, extra: Intl.NumberFormatOptions): Intl.NumberFormat {
  const key = `${currency ?? ""}|${JSON.stringify(extra)}`;
  let fmt = numberFormatCache.get(key);
  if (!fmt) {
    fmt = currency
      ? new Intl.NumberFormat("en-US", { style: "currency", currency, ...extra })
      : new Intl.NumberFormat("en-US", extra);
    numberFormatCache.set(key, fmt);
  }
  return fmt;
}

/**
 * `Intl.NumberFormat` THROWS a RangeError on a currency code that is not three
 * ASCII letters, which would take down the whole screen. Account currencies can
 * come from arbitrary CSV imports, so anything unrecognised falls back to
 * decimal formatting with the raw code as a prefix — the amount still renders,
 * and it is still labelled with whatever the data actually says.
 */
function formatIn(
  cents: number,
  currency: string,
  extra: Intl.NumberFormatOptions,
): string {
  const isIso4217 = /^[A-Za-z]{3}$/.test(currency);
  if (isIso4217) {
    return getNumberFormat(currency.toUpperCase(), extra).format(cents / 100);
  }
  const amount = getNumberFormat(null, extra).format(cents / 100);
  return currency ? `${currency} ${amount}` : amount;
}

/**
 * Format a cent amount as currency. Defaults to the user's configured
 * currency (from zustand store), 0 decimal places, comma-grouped.
 * Pass `{ decimals: 2 }` for cent precision, `{ currency }` to override.
 *
 * Prefer passing an explicit `currency` derived from the DATA (an account's
 * own code, or `FinancialMetrics.currency`) over relying on the default. The
 * stored preference is a display setting that nothing keeps in step with the
 * accounts a user actually holds, so it can label a CAD figure as USD.
 */
export function money(cents: number, opts: MoneyOpts = {}): string {
  const decimals = opts.decimals ?? 0;
  const currency = opts.currency ?? useTweaks.getState().currency ?? "USD";
  return formatIn(cents, currency, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
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
  return formatIn(cents, currency, {
    notation: "compact",
    maximumFractionDigits: abs >= 1000 ? 1 : 0,
  });
}

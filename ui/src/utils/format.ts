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
 * Ingest (`finsight-providers::amount`) rejects cent values beyond this range,
 * mirroring Actual Budget's `safeNumber`/`MAX_SAFE_NUMBER` guard: display
 * divides cents by 100 as a float, and past 2^51 the nearest double can render
 * a different cent amount than was stored. A non-integer or out-of-range value
 * reaching a formatter is corrupt data — throwing is loud and fixable, while
 * formatting it would print a confidently wrong amount. (Unlike an unusable
 * currency code, which is benign display metadata, wrong money is never
 * renderable "as-is".)
 */
const MAX_SAFE_CENTS = 2 ** 51 - 1;

function assertDisplayableCents(cents: number): void {
  if (
    !Number.isInteger(cents) ||
    !Number.isSafeInteger(cents) ||
    Math.abs(cents) > MAX_SAFE_CENTS
  ) {
    throw new Error(`money: cent amount is not display-safe: ${cents}`);
  }
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
  assertDisplayableCents(cents);
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
 * stored preference is hydrated from the authenticated user's server setting;
 * explicit data currency is still preferred for mixed-currency screens.
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

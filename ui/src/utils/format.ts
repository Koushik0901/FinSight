interface MoneyOpts {
  /** Fraction digits (both min and max). Default 0. */
  decimals?: number;
  /** ISO currency code. Default "USD". */
  currency?: string;
}

/**
 * Format a cent amount as currency. Defaults to USD, 0 decimal places,
 * comma-grouped. Pass `{ decimals: 2 }` for cent precision, `{ currency }`
 * for a non-USD account.
 */
export function money(cents: number, opts: MoneyOpts = {}): string {
  const decimals = opts.decimals ?? 0;
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: opts.currency ?? "USD",
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(cents / 100);
}

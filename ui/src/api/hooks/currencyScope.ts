import { useFinancialMetrics } from "./metrics";

/**
 * The currency client-side totals must be scoped to, and a predicate for
 * applying it.
 *
 * WHICH currency is primary, and whether the user holds more than one, is
 * decided in exactly one place — `finsight-core::currency` — and read from the
 * metrics payload here. This hook only *applies* that answer. Re-deriving the
 * rule in TypeScript would let a client-side headline disagree with the
 * server-computed figure beside it, which is the failure this whole area is
 * about.
 *
 * When metrics have not loaded, or the user holds a single currency, `inScope`
 * admits everything — which is both the correct answer and the existing
 * behaviour for the overwhelming majority of users.
 */
export function useCurrencyScope() {
  const { data: metrics } = useFinancialMetrics();
  const scope = metrics?.currency?.trim().toUpperCase();

  /** Matches `finsight-core::currency::normalize_code`: trim, upper-case, and
   *  treat a blank as the schema default the column would otherwise hold. */
  const inScope = (currency: string | null | undefined) => {
    if (!scope) return true;
    const code = (currency ?? "").trim().toUpperCase() || "USD";
    return code === scope;
  };

  return {
    /** Primary currency code, or undefined before metrics load. */
    currency: metrics?.currency ?? undefined,
    /** Money held in other currencies, never converted into the totals. */
    unconverted: metrics?.unconvertedHoldings ?? [],
    inScope,
  };
}

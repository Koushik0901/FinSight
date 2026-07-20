import { money } from "../utils/format";
import type { UnconvertedHolding } from "../api/client";

/**
 * Names money the user holds that the surrounding totals deliberately exclude.
 *
 * FinSight never converts between currencies: a live rate would need a network
 * call the local-first design does not make, and a stored rate goes stale
 * silently. So aggregates are scoped to one currency, and this states plainly
 * what was left out — turning a total that would have been quietly wrong into
 * an honest partial answer.
 *
 * Renders nothing in the single-currency case, which is almost everyone.
 */
export function UnconvertedCurrencies({
  holdings,
  primary,
}: {
  holdings: UnconvertedHolding[] | undefined;
  primary: string | null | undefined;
}) {
  if (!holdings || holdings.length === 0) return null;

  const held = holdings
    .map((h) => money(h.balanceCents, { currency: h.code }))
    .join(" · ");

  return (
    <div className="hero-note" role="status">
      Totals are in {primary ?? "your main currency"}. You also hold {held} —
      not converted, so not included above. FinSight doesn&rsquo;t use exchange
      rates, so these are kept separate rather than added together.
    </div>
  );
}

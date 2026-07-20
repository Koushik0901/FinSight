import type { RecurringItem } from "../api/client";

/**
 * Human cadence label for a recurring item. Prefers the backend-provided
 * `cadence`, falling back to a bucket derived from the observed average gap.
 */
export function recurringFrequency(item: Pick<RecurringItem, "cadence" | "avgGapDays">): string {
  if (item.cadence) return item.cadence;
  if (item.avgGapDays <= 8) return "weekly";
  if (item.avgGapDays <= 16) return "biweekly";
  if (item.avgGapDays <= 40) return "monthly";
  return "irregular";
}

/**
 * Per-month cost of a recurring item, positive cents.
 *
 * The normalisation itself lives in `finsight-core::recurring` and arrives on
 * the item as `monthlyEquivalentCents`. It used to be reimplemented here, which
 * meant the "committed per month" figure on this screen and the same figure in
 * the budget planner and Copilot were two independent implementations of one
 * rule, free to drift. This is now just a typed accessor.
 */
export function monthlyEquivalentCents(
  item: Pick<RecurringItem, "monthlyEquivalentCents">,
): number {
  return item.monthlyEquivalentCents;
}

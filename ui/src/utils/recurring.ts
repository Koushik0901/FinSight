import type { RecurringItem } from "../api/client";

const AVG_DAYS_PER_MONTH = 30.44;

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
 * Normalize a recurring item's last amount to a per-month figure (positive
 * cents) so weekly, biweekly, monthly, quarterly, and annual commitments are
 * directly comparable and summable. An annual subscription must NOT be counted
 * as a monthly one — that inflates annualized cost 12×.
 */
export function monthlyEquivalentCents(
  item: Pick<RecurringItem, "cadence" | "avgGapDays" | "lastAmountCents">,
): number {
  const abs = Math.abs(item.lastAmountCents);
  switch (item.cadence) {
    case "weekly":
      return Math.round(abs * (AVG_DAYS_PER_MONTH / 7));
    case "biweekly":
      return Math.round(abs * (AVG_DAYS_PER_MONTH / 14));
    case "monthly":
      return abs;
    case "quarterly":
      return Math.round(abs / 3);
    case "annual":
    case "yearly":
      return Math.round(abs / 12);
  }
  // No explicit cadence: derive from the observed average gap between hits.
  if (item.avgGapDays > 0) return Math.round(abs * (AVG_DAYS_PER_MONTH / item.avgGapDays));
  return abs;
}

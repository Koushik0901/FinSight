/**
 * Canonical color per ACCOUNT TYPE, used everywhere an account's type is
 * shown (Accounts list, ledger header, drawers, import dialog, Today cards)
 * so each type reads the same across the app. Deliberately distinct hex
 * values from the category palette in `categoryColor.ts` — an account row and
 * a category swatch must never look interchangeable.
 */
export const DEFAULT_ACCOUNT_TYPE_COLOR = "#94A3B8";

const TYPE_COLORS: Record<string, string> = {
  checking:   "#38BDF8", // sky — day-to-day money movement
  savings:    "#4ADE80", // green — growth and safety
  credit:     "#F97316", // orange — liabilities to watch
  investment: "#C084FC", // violet — long-horizon assets
  cash:       "#FBBF24", // amber — physical cash
  loan:       "#F87171", // red — debt
};

export function accountTypeColor(type: string | null | undefined): string {
  if (!type) return DEFAULT_ACCOUNT_TYPE_COLOR;
  return TYPE_COLORS[type.trim().toLowerCase()] ?? DEFAULT_ACCOUNT_TYPE_COLOR;
}

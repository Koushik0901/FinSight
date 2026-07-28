import type { AccountSummary, MonthSummary } from "../api/client";

export type DataReadiness = "unavailable" | "estimated" | "reliable";

export interface BudgetReadinessInput {
  envelopeCount: number;
  fundedEnvelopeCount: number;
  transactionCount: number;
  spentCents: number;
  dayOfMonth: number;
}

/**
 * Budget judgments need an actual plan and enough observed activity to avoid
 * presenting missing data as reassurance. A funded plan can still be useful
 * before the pace estimate is trustworthy, so that middle state is explicit.
 */
export function getBudgetReadiness(input: BudgetReadinessInput): DataReadiness {
  if (input.envelopeCount === 0 || input.fundedEnvelopeCount === 0) {
    return "unavailable";
  }

  const hasObservedSpend = input.spentCents > 0 && input.transactionCount > 0;
  const hasEnoughCoverage = input.transactionCount >= 10 || input.dayOfMonth >= 7;
  return hasObservedSpend && hasEnoughCoverage ? "reliable" : "estimated";
}

export interface ReportReadiness {
  hasActivity: boolean;
  savingsRate: DataReadiness;
  averageSpend: DataReadiness;
  netWorth: DataReadiness;
  runway: DataReadiness;
}

/**
 * Report metrics become available independently. A user may have trustworthy
 * account balances before importing transaction history, or spending history
 * without identifiable income. Keeping those states separate avoids false
 * zeroes while still surfacing the facts FinSight genuinely knows.
 */
export function getReportReadiness(
  monthly: MonthSummary[],
  accounts: AccountSummary[],
  runwayDays: number | null | undefined,
): ReportReadiness {
  const activityMonths = monthly.filter(
    (month) => month.incomeCents !== 0 || month.expenseCents !== 0,
  );
  const completeCashflowMonths = monthly.filter(
    (month) => month.incomeCents > 0 && month.expenseCents > 0,
  );
  const expenseMonths = monthly.filter((month) => month.expenseCents > 0);
  const knownBalanceAccounts = accounts.filter((account) => account.balance_known === true);

  return {
    hasActivity: activityMonths.length > 0,
    savingsRate: completeCashflowMonths.length > 0 ? "reliable" : "unavailable",
    averageSpend:
      expenseMonths.length >= 2
        ? "reliable"
        : expenseMonths.length === 1
          ? "estimated"
          : "unavailable",
    netWorth: knownBalanceAccounts.length > 0 ? "reliable" : "unavailable",
    runway: runwayDays == null ? "unavailable" : "reliable",
  };
}

export function normalizeCount(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : 0;
}

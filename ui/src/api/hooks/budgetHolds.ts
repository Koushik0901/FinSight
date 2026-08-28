import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BudgetHold } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

/**
 * Hold for Next Month — Actual-style.
 *
 * A hold parks unassigned money for the next month. It deducts from this
 * month's `toBudget` (`income - budgeted - hold`) and reappears as
 * income-like in `available_funds` for the following month.
 *
 * `month` is "YYYY-MM" (e.g. "2026-09").
 */
export function useHold(month: string | null) {
  return useQuery<BudgetHold | null>({
    queryKey: ["hold", month],
    queryFn: async () => {
      if (!month) return null;
      // In unit tests the openapiClient mock often only stubs a handful of
      // methods (getMonthTotals, getSpendingBreakdown). A missing getHold
      // should be treated as "no hold" rather than a hard failure so older
      // Budget tests that were written before holds don't have to be updated.
      const maybe = (api as unknown as { getHold?: typeof api.getHold }).getHold;
      if (typeof maybe !== "function") return null;
      const res = await unwrap(maybe(month));
      // get_hold returns Option<BudgetHold> → null when absent, object when present.
      return (res as BudgetHold | null) ?? null;
    },
    enabled: isBackendAvailable() && !!month,
  });
}

export function useSetHold() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ month, amountCents }: { month: string; amountCents: number }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      const maybe = (api as unknown as { setHold?: typeof api.setHold }).setHold;
      if (typeof maybe !== "function") throw new Error("setHold not available in this environment");
      return unwrap(maybe(month, amountCents));
    },
    onSuccess: (data) => {
      qc.setQueryData(["hold", data.month], data);
      qc.invalidateQueries({ queryKey: ["hold"] });
      // Hold changes `to_budget` / available_funds, which are derived from
      // income, budgeted, and hold — the budget overview and month totals
      // reflect that, so invalidate them too.
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
    },
  });
}

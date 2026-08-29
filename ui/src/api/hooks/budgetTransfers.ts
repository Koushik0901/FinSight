import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BudgetTransfer } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

/**
 * Atomic Cover Ledger — FinSight's auditable `Cover` as a per-month ledger row.
 *
 * `available = budgeted + carryover + transfers_in - transfers_out - spent`
 * is computed in `finsight-core::repos::budgets::available` and now also
 * reflected in `BudgetEnvelope.transferCents` so `Budget.tsx` shows the
 * post-cover remaining without an extra round-trip.
 *
 * Each row is an auditable move `from_category -> to_category` within a single
 * `month` (YYYY-MM). Either side may be `null` to represent To Budget, but not
 * both. The donor's spare is validated atomically (`BEGIN IMMEDIATE`) via
 * `transfer_budget`, so concurrent covers cannot overdraft the same donor.
 */

export function useBudgetTransfers(month: string | null) {
  return useQuery<BudgetTransfer[]>({
    queryKey: ["budget-transfers", month],
    queryFn: async () => {
      if (!month) return [];
      const maybe = (api as unknown as { listBudgetTransfers?: typeof api.listBudgetTransfers }).listBudgetTransfers;
      if (typeof maybe !== "function") return [];
      return unwrap(maybe(month));
    },
    enabled: isBackendAvailable() && !!month,
    staleTime: 30_000,
  });
}

export function useTransferBudget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      fromCategory: string | null;
      toCategory: string | null;
      amountCents: number;
      month: string;
      note?: string | null;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      const maybe = (api as unknown as { transferBudget?: typeof api.transferBudget }).transferBudget;
      if (typeof maybe !== "function") throw new Error("transferBudget not available in this environment");
      return unwrap(
        maybe(input.fromCategory ?? null, input.toCategory ?? null, input.amountCents, input.month, input.note ?? null),
      );
    },
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["budget-transfers", vars.month] });
      qc.invalidateQueries({ queryKey: ["budget-transfers"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
    },
  });
}

// Alias for plan's `transfer_envelope` name — same ledger, same validation.
export const useTransferEnvelope = useTransferBudget;
export const useBudgetTransfersAlias = useBudgetTransfers;

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type ActionItem, type CounterpartyVerdict, type UnresolvedCounterpartyDto } from "../client";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

export function useActionItems() {
  return useQuery<ActionItem[]>({
    queryKey: ["action-items"],
    queryFn: async () => {
      const result = await commands.getActionItems();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 30_000,
    refetchInterval: 30_000,
    enabled: isBackendAvailable(),
  });
}

/** The undecided transfer-review queue, grouped by counterparty — powers the
 *  "People with unresolved money" review card. */
export function useUnresolvedCounterparties() {
  return useQuery<UnresolvedCounterpartyDto[]>({
    queryKey: ["unresolved-counterparties"],
    queryFn: async () => {
      const result = await commands.listUnresolvedCounterparties();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable(),
  });
}

/** Apply one counterparty verdict (transfer / settle-up / real spending) to
 *  every undecided transaction matching a counterparty pattern. One decision
 *  clears a whole person's history from the review list. */
export function useApplyCounterpartyVerdict() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ pattern, verdict }: { pattern: string; verdict: CounterpartyVerdict }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.applyCounterpartyVerdictToSimilar(pattern, verdict);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    // A verdict moves money in/out of income & spending — every headline
    // number (savings rate, cashflow, budget, inbox) can change, same as the
    // binary transfer-toggle path in useApplyTransferVerdictToSimilar.
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["unresolved-counterparties"] });
      invalidateDomains(qc, "transactions");
      void qc.invalidateQueries();
    },
  });
}

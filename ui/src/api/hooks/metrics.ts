import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type FinancialMetrics, type FinancialAssumptionsInput } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

/**
 * Canonical financial numbers from the shared `finsight-core::metrics` layer —
 * balances, trailing averages, runway, emergency-fund coverage, and the user's
 * targets. Screens read these instead of recomputing, so the UI and the Copilot
 * never disagree.
 */
export function useFinancialMetrics(memberId?: string | null) {
  return useQuery<FinancialMetrics>({
    // memberId in the key so switching person refetches; null/undefined = the
    // whole household (unchanged behaviour).
    queryKey: ["financial-metrics", memberId ?? null],
    queryFn: async () => {
      const result = await commands.getFinancialMetrics(memberId ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isTauriRuntime(),
  });
}

export function useSetFinancialAssumptions() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: FinancialAssumptionsInput) => {
      const result = await commands.setFinancialAssumptions(input);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      // Targets feed the metrics response and the compound projector.
      qc.invalidateQueries({ queryKey: ["financial-metrics"] });
      qc.invalidateQueries({ queryKey: ["goal-projection"] });
    },
  });
}

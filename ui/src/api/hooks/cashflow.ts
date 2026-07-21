import { useQuery } from "@tanstack/react-query";
import { commands, type CashflowForecast } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

export interface CashflowParams {
  horizonDays: number;
  bufferCents: number;
  /** Hypothetical one-off spend to test; 0 = none. */
  extraExpenseCents: number;
}

/**
 * Near-term daily cash-flow forecast + safe-to-spend from the shared
 * `finsight-core::cashflow` layer. The buffer and hypothetical spend are pure
 * what-if parameters — evaluated server-side without persisting anything — so
 * changing them just refetches a fresh projection.
 */
export function useCashflowForecast(params: CashflowParams) {
  return useQuery<CashflowForecast>({
    queryKey: ["cashflow-forecast", params.horizonDays, params.bufferCents, params.extraExpenseCents],
    queryFn: async () => {
      const result = await commands.getCashflowForecast(
        params.horizonDays,
        params.bufferCents,
        params.extraExpenseCents > 0 ? params.extraExpenseCents : null,
        null,
      );
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 30_000,
    // Keep showing the previous forecast while a new buffer/horizon refetches,
    // so the chart doesn't flash empty on every keystroke.
    placeholderData: (prev) => prev,
    enabled: isBackendAvailable(),
  });
}

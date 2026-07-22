import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type RecurringItem } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

export function useRecurring() {
  return useQuery<RecurringItem[]>({
    queryKey: ["recurring"],
    queryFn: async () => {
      const result = await commands.listRecurring();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 5 * 60_000,
    enabled: isBackendAvailable(),
  });
}

/**
 * Confirm or dismiss a detected subscription (#58). `verdict` is "confirmed" |
 * "dismissed", or null to clear. A dismissed series stops producing
 * price-change/renewal alerts. Optimistic so the row updates immediately.
 */
export function useSetSubscriptionVerdict() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (vars: { merchantKey: string; verdict: string | null }) => {
      const result = await commands.setSubscriptionVerdict(vars.merchantKey, vars.verdict);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onMutate: async ({ merchantKey, verdict }) => {
      await qc.cancelQueries({ queryKey: ["recurring"] });
      const prev = qc.getQueryData<RecurringItem[]>(["recurring"]);
      qc.setQueryData<RecurringItem[]>(["recurring"], (old) =>
        (old ?? []).map((i) => (i.merchantKey === merchantKey ? { ...i, verdict } : i)),
      );
      return { prev };
    },
    onError: (_e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(["recurring"], ctx.prev);
    },
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: ["recurring"] });
    },
  });
}

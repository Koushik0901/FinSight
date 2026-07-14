import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type PathBackView } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

/**
 * The "path back" verdict for a period: how it compares to the user's normal,
 * plus the honest plan (self-correcting drivers vs. recurring levers) for
 * getting back there. `period` null defaults to the latest month server-side;
 * `targetMonthlyCents` null omits the target verdict (recent vs. baseline only).
 */
export function usePathBack(period: string | null, targetMonthlyCents: number | null) {
  return useQuery<PathBackView | null>({
    queryKey: ["path-back", period, targetMonthlyCents],
    queryFn: async () => {
      const result = await commands.getSpendingPathBack(period, targetMonthlyCents);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    enabled: isTauriRuntime(),
  });
}

/**
 * Records a sticky user verdict on a spending driver (expected / one_off /
 * reset) so it stops recomputing as a "lever" every time the plan re-runs.
 */
export function useSetSpendingAnnotation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (v: { merchantKey: string; verdict: string }) => {
      const result = await commands.setSpendingAnnotation(v.merchantKey, v.verdict);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["path-back"] });
    },
  });
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type PathBackView } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

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
      return unwrap(api.getSpendingPathBack(period, targetMonthlyCents));
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
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
      await unwrap(api.setSpendingAnnotation(v.merchantKey, v.verdict));
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["path-back"] });
    },
  });
}

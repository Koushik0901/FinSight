import { useQuery } from "@tanstack/react-query";
import { api, type HealthScore } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

export function useRecentAgentActivity(limit: number) {
  return useQuery({
    queryKey: ["agent-activity", limit],
    queryFn: async () => {
      return unwrap(api.listRecentAgentActivity(limit));
    },
    refetchInterval: 30_000,
  });
}

export function useHealthScore() {
  return useQuery<HealthScore>({
    queryKey: ["financial-health-score"],
    queryFn: async () => {
      return unwrap(api.getFinancialHealthScore());
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isBackendAvailable(),
  });
}

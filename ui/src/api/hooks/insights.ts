import { useQuery } from "@tanstack/react-query";
import { commands, type HealthScore } from "../client";
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

export function useRecentAgentActivity(limit: number) {
  return useQuery({
    queryKey: ["agent-activity", limit],
    queryFn: async () => {
      return unwrap(commands.listRecentAgentActivity(limit));
    },
    refetchInterval: 30_000,
  });
}

export function useHealthScore() {
  return useQuery<HealthScore>({
    queryKey: ["financial-health-score"],
    queryFn: async () => {
      return unwrap(commands.getFinancialHealthScore());
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isBackendAvailable(),
  });
}

import { useQuery } from "@tanstack/react-query";
import { commands, type HealthScore } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useRecentAgentActivity(limit: number) {
  return useQuery({
    queryKey: ["agent-activity", limit],
    queryFn: async () => {
      const result = await commands.listRecentAgentActivity(limit);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    refetchInterval: 30_000,
  });
}

export function useHealthScore() {
  return useQuery<HealthScore>({
    queryKey: ["financial-health-score"],
    queryFn: async () => {
      const result = await commands.getFinancialHealthScore();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isTauriRuntime(),
  });
}

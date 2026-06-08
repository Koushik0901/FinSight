import { useQuery } from "@tanstack/react-query";
import { commands } from "../client";

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

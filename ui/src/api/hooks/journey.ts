import { useQuery } from "@tanstack/react-query";
import { commands, type JourneyStatus } from "../client";

export function useJourneyStatus() {
  return useQuery<JourneyStatus>({
    queryKey: ["journey-status"],
    queryFn: async () => {
      const result = await commands.getJourneyStatus();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
}

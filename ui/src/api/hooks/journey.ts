import { useQuery } from "@tanstack/react-query";
import { commands, type JourneyStatus } from "../client";
import { unwrap } from "../client";

export function useJourneyStatus() {
  return useQuery<JourneyStatus>({
    queryKey: ["journey-status"],
    queryFn: async () => {
      return unwrap(commands.getJourneyStatus());
    },
    staleTime: 60_000,
  });
}

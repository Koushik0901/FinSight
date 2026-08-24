import { useQuery } from "@tanstack/react-query";
import { api, type JourneyStatus } from "../openapiClient";
import { unwrap } from "../openapiClient";

export function useJourneyStatus() {
  return useQuery<JourneyStatus>({
    queryKey: ["journey-status"],
    queryFn: async () => {
      return unwrap(api.getJourneyStatus());
    },
    staleTime: 60_000,
  });
}

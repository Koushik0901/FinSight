import { useQuery } from "@tanstack/react-query";
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

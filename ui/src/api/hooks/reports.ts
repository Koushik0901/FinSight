import { useQuery } from "@tanstack/react-query";
import { commands, type MonthTotals } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useMonthTotals() {
  return useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      const result = await commands.getMonthTotals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isTauriRuntime(),
  });
}

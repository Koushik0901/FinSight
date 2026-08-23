import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type MonthCloseListItem,
  type MonthCloseView,
  type MonthTotals,
  type SaveMonthCloseInput,
  type SavingsRatePoint,
} from "../client";
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

export function useMonthTotals() {
  return useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      return unwrap(commands.getMonthTotals());
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isBackendAvailable(),
  });
}

export function useSavingsRateHistory() {
  return useQuery<SavingsRatePoint[]>({
    queryKey: ["savings-rate-history"],
    queryFn: async () => {
      return unwrap(commands.getSavingsRateHistory());
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

/** The guided month-end close (#59) for a given month — live while in progress,
 * frozen once completed. */
export function useMonthClose(year: number, month: number) {
  return useQuery<MonthCloseView>({
    queryKey: ["month-close", year, month],
    queryFn: async () => {
      return unwrap(commands.getMonthClose(year, month));
    },
    enabled: isBackendAvailable(),
  });
}

/** Advance the close lifecycle (start/complete/skip/reopen). */
export function useSaveMonthClose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: SaveMonthCloseInput) => {
      return unwrap(commands.saveMonthClose(input));
    },
    onSuccess: (data) => {
      qc.setQueryData(["month-close", data.year, data.month], data);
      qc.invalidateQueries({ queryKey: ["month-closes"] });
      qc.invalidateQueries({ queryKey: ["notifications"] });
    },
  });
}

/** Past closes, newest first — the "revisit a recorded close" surface. */
export function useMonthCloses() {
  return useQuery<MonthCloseListItem[]>({
    queryKey: ["month-closes"],
    queryFn: async () => {
      return unwrap(commands.listMonthCloses());
    },
    enabled: isBackendAvailable(),
  });
}

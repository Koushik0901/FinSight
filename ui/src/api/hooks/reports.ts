import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  type CustomReportParams,
  type CustomReportResult,
  type MonthCloseListItem,
  type MonthCloseView,
  type MonthTotals,
  type SaveMonthCloseInput,
  type SavingsRatePoint,
} from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

export function useMonthTotals() {
  return useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      return unwrap(api.getMonthTotals());
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
      return unwrap(api.getSavingsRateHistory());
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
      return unwrap(api.getMonthClose(year, month));
    },
    enabled: isBackendAvailable(),
  });
}

/** Advance the close lifecycle (start/complete/skip/reopen). */
export function useSaveMonthClose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: SaveMonthCloseInput) => {
      return unwrap(api.saveMonthClose(input));
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
      return unwrap(api.listMonthCloses());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCustomReport(params: CustomReportParams) {
  return useQuery<CustomReportResult>({
    queryKey: ["custom-report", params],
    queryFn: async () => {
      return unwrap(api.customReport(params));
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

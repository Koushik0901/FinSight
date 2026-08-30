import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";
import type { components } from "../openapi";

export type ReportWidget = components["schemas"]["ReportWidget"];

export function useReportWidgets() {
  return useQuery<ReportWidget[]>({
    queryKey: ["report-widgets"],
    queryFn: async () => unwrap(api.listReportWidgets()),
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

export function useCreateReportWidget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      title: string;
      chartType: string;
      splitBy: string;
      period: string;
      filtersJson?: string | null;
      position?: number | null;
    }) =>
      unwrap(
        api.createReportWidget(
          input.title,
          input.chartType as unknown as "table" | "bar" | "barStacked" | "line" | "area" | "donut",
          input.splitBy as unknown as "category" | "group" | "payee" | "account" | "month" | "spendingType",
          input.period as unknown as "Last1Month" | "Last3Months" | "Last6Months" | "YTD" | "All",
          input.filtersJson ?? null,
          input.position ?? null
        )
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["report-widgets"] });
    },
  });
}

export function useUpdateReportWidget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      id: string;
      title?: string | null;
      chartType?: string | null;
      splitBy?: string | null;
      period?: string | null;
      filtersJson?: string | null;
    }) =>
      unwrap(
        api.updateReportWidget(
          input.id,
          input.title ?? null,
          input.chartType as unknown as "table" | "bar" | "barStacked" | "line" | "area" | "donut" | null,
          input.splitBy as unknown as "category" | "group" | "payee" | "account" | "month" | "spendingType" | null,
          input.period as unknown as "Last1Month" | "Last3Months" | "Last6Months" | "YTD" | "All" | null,
          input.filtersJson ?? null
        )
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["report-widgets"] });
    },
  });
}

export function useDeleteReportWidget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => unwrap(api.deleteReportWidget(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["report-widgets"] });
    },
  });
}

export function useReorderReportWidgets() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (orderedIds: string[]) => unwrap(api.reorderReportWidgets(orderedIds)),
    onSuccess: (data) => {
      qc.setQueryData(["report-widgets"], data);
    },
  });
}

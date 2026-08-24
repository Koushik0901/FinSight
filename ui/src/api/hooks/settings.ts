import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { useTweaks } from "../../state/tweaks";
import { isBackendAvailable } from "../../utils/runtime";
import { downloadBlob } from "../../lib/downloadBlob";

export function useDefaultCurrency() {
  const setCurrencyTweak = useTweaks((s) => s.setCurrency);
  const query = useQuery<string>({
    queryKey: ["currency"],
    queryFn: async () => {
      return unwrap(api.getCurrency());
    },
    // This value is per-user server state and can also be derived from newly
    // added accounts. Do not trust a week-old persisted PWA query forever:
    // refresh on each authenticated app mount, while still using the cached
    // value during offline startup.
    staleTime: 5 * 60 * 1000,
    refetchOnMount: "always",
    enabled: isBackendAvailable(),
  });
  useEffect(() => {
    if (query.data) setCurrencyTweak(query.data);
  }, [query.data, setCurrencyTweak]);
  return query;
}

export function useSetCurrency() {
  const qc = useQueryClient();
  const setCurrencyTweak = useTweaks((s) => s.setCurrency);
  return useMutation({
    mutationFn: async (currency: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setCurrency(currency));
    },
    onSuccess: (_, currency) => {
      setCurrencyTweak(currency);
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

export function useNotificationsEnabled() {
  return useQuery<boolean>({
    queryKey: ["notifications-enabled"],
    queryFn: async () => {
      return unwrap(api.getNotificationsEnabled());
    },
    staleTime: Infinity,
    enabled: isBackendAvailable(),
  });
}

export function useSetNotificationsEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setNotificationsEnabled(enabled));
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["notifications-enabled"] }),
  });
}

export function useAutoCategorizeEnabled() {
  return useQuery<boolean>({
    queryKey: ["auto-categorize-enabled"],
    queryFn: async () => {
      return unwrap(api.getAutoCategorizeEnabled());
    },
    staleTime: Infinity,
    enabled: isBackendAvailable(),
  });
}

export function useSetAutoCategorizeEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setAutoCategorizeEnabled(enabled));
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["auto-categorize-enabled"] }),
  });
}

export function useExportJson() {
  return useMutation({
    mutationFn: async () => {
      const blob = await unwrap(api.exportAllDataJson());
      downloadBlob(blob, "application/json", "finsight-export.json");
    },
  });
}

export function useExportCsv() {
  return useMutation({
    mutationFn: async () => {
      const blob = await unwrap(api.exportAllDataCsv());
      downloadBlob(blob, "text/csv", "finsight-transactions.csv");
    },
  });
}

export function useDeleteAllData() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.deleteAllData());
    },
    onSuccess: () => {
      // Blow away every cached query so no stale dashboard/report/chart/balance
      // /insight data survives the wipe. Cheaper and safer than enumerating keys.
      qc.clear();
    },
  });
}

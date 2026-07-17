import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../client";
import { useTweaks } from "../../state/tweaks";
import { isTauriRuntime } from "../../utils/runtime";
import { downloadBlob } from "../../lib/downloadBlob";

export function useDefaultCurrency() {
  return useQuery<string>({
    queryKey: ["currency"],
    queryFn: async () => {
      const result = await commands.getCurrency();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: Infinity,
    enabled: isTauriRuntime(),
  });
}

export function useSetCurrency() {
  const qc = useQueryClient();
  const setCurrencyTweak = useTweaks((s) => s.setCurrency);
  return useMutation({
    mutationFn: async (currency: string) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setCurrency(currency);
      if (result.status === "error") throw new Error(result.error.message);
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
      const result = await commands.getNotificationsEnabled();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: Infinity,
    enabled: isTauriRuntime(),
  });
}

export function useSetNotificationsEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setNotificationsEnabled(enabled);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["notifications-enabled"] }),
  });
}

export function useAutoCategorizeEnabled() {
  return useQuery<boolean>({
    queryKey: ["auto-categorize-enabled"],
    queryFn: async () => {
      const result = await commands.getAutoCategorizeEnabled();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: Infinity,
    enabled: isTauriRuntime(),
  });
}

export function useSetAutoCategorizeEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setAutoCategorizeEnabled(enabled);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["auto-categorize-enabled"] }),
  });
}

export function useExportJson() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.exportAllDataJson();
      if (result.status === "error") throw new Error(result.error.message);
      downloadBlob(result.data, "application/json", "finsight-export.json");
    },
  });
}

export function useExportCsv() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.exportAllDataCsv();
      if (result.status === "error") throw new Error(result.error.message);
      downloadBlob(result.data, "text/csv", "finsight-transactions.csv");
    },
  });
}

export function useDeleteAllData() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.deleteAllData();
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      // Blow away every cached query so no stale dashboard/report/chart/balance
      // /insight data survives the wipe. Cheaper and safer than enumerating keys.
      qc.clear();
    },
  });
}

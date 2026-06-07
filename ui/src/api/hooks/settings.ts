import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../client";
import { useTweaks } from "../../state/tweaks";

export function useDefaultCurrency() {
  return useQuery<string>({
    queryKey: ["currency"],
    queryFn: async () => {
      const result = await commands.getCurrency();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: Infinity,
  });
}

export function useSetCurrency() {
  const qc = useQueryClient();
  const setCurrencyTweak = useTweaks((s) => s.setCurrency);
  return useMutation({
    mutationFn: async (currency: string) => {
      const result = await commands.setCurrency(currency);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: (_, currency) => {
      setCurrencyTweak(currency);
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

export function useExportJson() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.exportAllDataJson();
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

export function useExportCsv() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.exportAllDataCsv();
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

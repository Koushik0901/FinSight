import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ManualAsset, type NewManualAsset, type ManualAssetPatch,
  type DebtPayoffResult,
} from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useManualAssets() {
  return useQuery<ManualAsset[]>({
    queryKey: ["manual-assets"],
    queryFn: async () => {
      const result = await commands.listManualAssets();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCreateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewManualAsset) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createManualAsset(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
    },
  });
}

export function useUpdateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: ManualAssetPatch }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateManualAsset(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
    },
  });
}

export function useDeleteManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.deleteManualAsset(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
    },
  });
}

export function useDebtPayoff(extraMonthlyCents: number) {
  return useQuery<DebtPayoffResult[]>({
    queryKey: ["debt-payoff", extraMonthlyCents],
    queryFn: async () => {
      const result = await commands.computeDebtPayoff(extraMonthlyCents);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useUncelebratedMilestones() {
  return useQuery<number[]>({
    queryKey: ["networth-milestones"],
    queryFn: async () => {
      const result = await commands.getUncelebratedMilestones();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: Infinity,
    enabled: isTauriRuntime(),
  });
}

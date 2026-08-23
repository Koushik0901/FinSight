import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ManualAsset, type NewManualAsset, type ManualAssetPatch,
  type DebtPayoffResult,
} from "../client";
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

export function useManualAssets() {
  return useQuery<ManualAsset[]>({
    queryKey: ["manual-assets"],
    queryFn: async () => {
      return unwrap(commands.listManualAssets());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewManualAsset) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(commands.createManualAsset(input));
    },
    onSuccess: () => {
      invalidateDomains(qc, "manualAssets");
    },
  });
}

export function useUpdateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: ManualAssetPatch }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(commands.updateManualAsset(id, patch));
    },
    onSuccess: () => {
      invalidateDomains(qc, "manualAssets");
    },
  });
}

export function useDeleteManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(commands.deleteManualAsset(id));
    },
    onSuccess: () => {
      invalidateDomains(qc, "manualAssets");
    },
  });
}

export function useDebtPayoff(extraMonthlyCents: number) {
  return useQuery<DebtPayoffResult[]>({
    queryKey: ["debt-payoff", extraMonthlyCents],
    queryFn: async () => {
      return unwrap(commands.computeDebtPayoff(extraMonthlyCents));
    },
    enabled: isBackendAvailable(),
  });
}

export function useUncelebratedMilestones() {
  return useQuery<number[]>({
    queryKey: ["networth-milestones"],
    queryFn: async () => {
      return unwrap(commands.getUncelebratedMilestones());
    },
    staleTime: Infinity,
    enabled: isBackendAvailable(),
  });
}

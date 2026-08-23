import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  type ManualAsset, type NewManualAsset, type ManualAssetPatch,
  type DebtPayoffResult,
} from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

export function useManualAssets() {
  return useQuery<ManualAsset[]>({
    queryKey: ["manual-assets"],
    queryFn: async () => {
      return unwrap(api.listManualAssets());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewManualAsset) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createManualAsset(input));
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
      return unwrap(api.updateManualAsset(id, patch));
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
      await unwrap(api.deleteManualAsset(id));
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
      return unwrap(api.computeDebtPayoff(extraMonthlyCents));
    },
    enabled: isBackendAvailable(),
  });
}

export function useUncelebratedMilestones() {
  return useQuery<number[]>({
    queryKey: ["networth-milestones"],
    queryFn: async () => {
      return unwrap(api.getUncelebratedMilestones());
    },
    staleTime: Infinity,
    enabled: isBackendAvailable(),
  });
}

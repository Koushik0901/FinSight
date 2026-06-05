import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ManualAsset, type NewManualAsset, type ManualAssetPatch,
  type Liability, type NewLiability, type LiabilityPatch,
} from "../client";

export function useManualAssets() {
  return useQuery<ManualAsset[]>({
    queryKey: ["manual-assets"],
    queryFn: async () => {
      const result = await commands.listManualAssets();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewManualAsset) => {
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
      const result = await commands.deleteManualAsset(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
    },
  });
}

export function useLiabilities() {
  return useQuery<Liability[]>({
    queryKey: ["liabilities"],
    queryFn: async () => {
      const result = await commands.listLiabilities();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateLiability() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewLiability) => {
      const result = await commands.createLiability(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["liabilities"] }); },
  });
}

export function useUpdateLiability() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: LiabilityPatch }) => {
      const result = await commands.updateLiability(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["liabilities"] }); },
  });
}

export function useDeleteLiability() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.deleteLiability(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["liabilities"] }); },
  });
}

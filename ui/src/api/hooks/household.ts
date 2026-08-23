import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type AccountOwner, type AssetOwner, type HouseholdMember, type MemberNetWorth, type OwnerShare } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

export function useHouseholdNetWorthBreakdown() {
  return useQuery<MemberNetWorth[]>({
    queryKey: ["household-net-worth"],
    queryFn: async () => {
      return unwrap(api.householdNetWorthBreakdown());
    },
    enabled: isBackendAvailable(),
  });
}

export function useHouseholdMembers() {
  return useQuery<HouseholdMember[]>({
    queryKey: ["household-members"],
    queryFn: async () => {
      return unwrap(api.listHouseholdMembers());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateHouseholdMember() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ name, color }: { name: string; color?: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createHouseholdMember(name, color ?? null));
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["household-members"] });
    },
  });
}

export function useSetSelfMember() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (memberId: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setSelfMember(memberId));
    },
    // Setting the operator re-runs the classification cascade (their own
    // e-transfers become internal moves), so cashflow, savings rate, anomalies
    // and category totals across the whole app change — invalidate everything.
    onSuccess: () => {
      void qc.invalidateQueries();
    },
  });
}

export function useDeleteHouseholdMember() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.deleteHouseholdMember(id));
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["household-members"] });
      void qc.invalidateQueries({ queryKey: ["account-owners"] });
      void qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useAccountOwners() {
  return useQuery<AccountOwner[]>({
    queryKey: ["account-owners"],
    queryFn: async () => {
      return unwrap(api.listAccountOwners());
    },
    enabled: isBackendAvailable(),
  });
}

export function useSetAccountOwners() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ accountId, memberIds }: { accountId: string; memberIds: string[] }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setAccountOwners(accountId, memberIds));
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["account-owners"] });
      void qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useSetAccountOwnerShares() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ accountId, owners }: { accountId: string; owners: OwnerShare[] }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setAccountOwnerShares(accountId, owners));
    },
    // Explicit shares change every per-member number, so invalidate broadly.
    onSuccess: () => {
      void qc.invalidateQueries();
    },
  });
}

export function useAssetOwners() {
  return useQuery<AssetOwner[]>({
    queryKey: ["asset-owners"],
    queryFn: async () => {
      return unwrap(api.listAssetOwners());
    },
    enabled: isBackendAvailable(),
  });
}

export function useSetAssetOwners() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ assetId, owners }: { assetId: string; owners: OwnerShare[] }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setAssetOwners(assetId, owners));
    },
    onSuccess: () => {
      void qc.invalidateQueries();
    },
  });
}

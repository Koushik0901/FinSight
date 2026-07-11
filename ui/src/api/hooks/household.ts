import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AccountOwner, type HouseholdMember, type OwnerShare } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useHouseholdMembers() {
  return useQuery<HouseholdMember[]>({
    queryKey: ["household-members"],
    queryFn: async () => {
      const result = await commands.listHouseholdMembers();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCreateHouseholdMember() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ name, color }: { name: string; color?: string | null }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createHouseholdMember(name, color ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setSelfMember(memberId);
      if (result.status === "error") throw new Error(result.error.message);
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
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.deleteHouseholdMember(id);
      if (result.status === "error") throw new Error(result.error.message);
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
      const result = await commands.listAccountOwners();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useSetAccountOwners() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ accountId, memberIds }: { accountId: string; memberIds: string[] }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setAccountOwners(accountId, memberIds);
      if (result.status === "error") throw new Error(result.error.message);
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
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setAccountOwnerShares(accountId, owners);
      if (result.status === "error") throw new Error(result.error.message);
    },
    // Explicit shares change every per-member number, so invalidate broadly.
    onSuccess: () => {
      void qc.invalidateQueries();
    },
  });
}

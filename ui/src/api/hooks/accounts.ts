import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AccountSummary, type NewAccount, type AccountPatch } from "../client";

export function useAccounts() {
  return useQuery<AccountSummary[]>({
    queryKey: ["accounts"],
    queryFn: async () => {
      const result = await commands.listAccounts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewAccount) => {
      const result = await commands.createAccount(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["onboarding-state"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

export function useUpdateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: AccountPatch }) => {
      const result = await commands.updateAccount(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

export function useArchiveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.archiveAccount(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

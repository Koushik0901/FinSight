import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type AccountSummary,
  type NewAccount,
  type AccountPatch,
  type AccountBalancePoint,
  type AccountSparkline,
} from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useAccounts() {
  return useQuery<AccountSummary[]>({
    queryKey: ["accounts"],
    queryFn: async () => {
      const result = await commands.listAccounts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewAccount) => {
      if (!isTauriRuntime()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.createAccount(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["onboarding-state"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useUpdateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: AccountPatch }) => {
      if (!isTauriRuntime()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.updateAccount(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useAccountBalanceHistory(accountId: string | undefined, days: number) {
  return useQuery<AccountBalancePoint[]>({
    queryKey: ["account-balance-history", accountId, days],
    queryFn: async () => {
      if (!accountId) return [];
      const result = await commands.listAccountBalanceHistory(accountId, days);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: !!accountId && isTauriRuntime(),
  });
}

export function useAccountBalanceSparklines(days: number) {
  return useQuery<AccountSparkline[]>({
    queryKey: ["account-balance-sparklines", days],
    queryFn: async () => {
      const result = await commands.listAccountBalanceSparklines(days);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useArchiveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isTauriRuntime()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.archiveAccount(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

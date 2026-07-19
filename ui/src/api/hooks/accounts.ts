import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type AccountSummary,
  type NewAccount,
  type AccountPatch,
  type AccountBalancePoint,
  type AccountBalanceTimeline,
  type AccountSparkline,
} from "../client";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

export function useAccounts() {
  return useQuery<AccountSummary[]>({
    queryKey: ["accounts"],
    queryFn: async () => {
      const result = await commands.listAccounts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewAccount) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.createAccount(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      invalidateDomains(qc, "accounts");
      // Creating the first account advances onboarding; not part of the
      // accounts data domain.
      qc.invalidateQueries({ queryKey: ["onboarding-state"] });
    },
  });
}

export function useUpdateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: AccountPatch }) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.updateAccount(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      invalidateDomains(qc, "accounts");
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
    enabled: !!accountId && isBackendAvailable(),
  });
}

/**
 * An account's balance curve reconstructed from its ledger, with the peak and
 * trough over the window.
 *
 * Distinct from `useAccountBalanceHistory`, which reads the STORED balance
 * snapshots — those are written opportunistically, so they are a sparse scatter
 * and the true peak usually falls on a day none of them covers. Pass `since` as
 * an ISO date, or null for all-time.
 */
export function useAccountBalanceTimeline(accountId: string | undefined, since: string | null) {
  return useQuery<AccountBalanceTimeline>({
    queryKey: ["account-balance-timeline", accountId, since],
    queryFn: async () => {
      const result = await commands.getAccountBalanceTimeline(accountId!, since);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: !!accountId && isBackendAvailable(),
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
    enabled: isBackendAvailable(),
  });
}

export function useArchiveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.archiveAccount(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      // Archiving an account removes its transactions from the ledger view
      // (totals, reports, review queue), so invalidate the transaction domain
      // too — not just the account list.
      invalidateDomains(qc, "accounts", "transactions");
    },
  });
}

export function useSetAccountBalance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, balanceCents }: { id: string; balanceCents: number }) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.setAccountBalance(id, balanceCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "accounts");
    },
  });
}

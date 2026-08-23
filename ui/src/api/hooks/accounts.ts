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
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

export function useAccounts() {
  return useQuery<AccountSummary[]>({
    queryKey: ["accounts"],
    queryFn: async () => {
      return unwrap(commands.listAccounts());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewAccount) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs a connected FinSight server.");
      }
      return unwrap(commands.createAccount(input));
    },
    onSuccess: () => {
      invalidateDomains(qc, "accounts");
      // Creating the first account advances onboarding; not part of the
      // accounts data domain.
      qc.invalidateQueries({ queryKey: ["onboarding-state"] });
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

export function useUpdateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: AccountPatch }) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs a connected FinSight server.");
      }
      return unwrap(commands.updateAccount(id, patch));
    },
    onSuccess: () => {
      invalidateDomains(qc, "accounts");
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

export function useAccountBalanceHistory(accountId: string | undefined, days: number) {
  return useQuery<AccountBalancePoint[]>({
    queryKey: ["account-balance-history", accountId, days],
    queryFn: async () => {
      if (!accountId) return [];
      return unwrap(commands.listAccountBalanceHistory(accountId, days));
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
      return unwrap(commands.getAccountBalanceTimeline(accountId!, since));
    },
    enabled: !!accountId && isBackendAvailable(),
    // Keep the current curve on screen while the range selector's refetch is in
    // flight, so switching 3M↔1Y swaps the body smoothly instead of blanking it
    // (same flicker fix as the Today net-worth range selector).
    placeholderData: (prev) => prev,
  });
}

export function useAccountBalanceSparklines(days: number) {
  return useQuery<AccountSparkline[]>({
    queryKey: ["account-balance-sparklines", days],
    queryFn: async () => {
      return unwrap(commands.listAccountBalanceSparklines(days));
    },
    enabled: isBackendAvailable(),
  });
}

export function useArchiveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs a connected FinSight server.");
      }
      await unwrap(commands.archiveAccount(id));
    },
    onSuccess: () => {
      // Archiving an account removes its transactions from the ledger view
      // (totals, reports, review queue), so invalidate the transaction domain
      // too — not just the account list.
      invalidateDomains(qc, "accounts", "transactions");
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

export function useSetAccountBalance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, balanceCents }: { id: string; balanceCents: number }) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs a connected FinSight server.");
      }
      await unwrap(commands.setAccountBalance(id, balanceCents));
    },
    onSuccess: () => {
      invalidateDomains(qc, "accounts");
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

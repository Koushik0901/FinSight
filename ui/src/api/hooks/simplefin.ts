import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type SimpleFinAccountImportRequest,
  type SimpleFinStatus,
  type SimpleFinAccountInfo,
  type SimpleFinConnectionInfo,
  type SyncSummary,
  type SimpleFinSyncSettings,
  type SimpleFinAlert,
  type TransferSuggestionInfo,
  type ImportCandidateWithMatches,
} from "../client";
import { unwrap } from "../client";
import { invalidateDomains } from "../invalidation";

/**
 * Canonical keys for every simplefin query. Exported so screens that need to
 * invalidate one surface (e.g. the Inbox resolving alerts/transfers/import
 * review) reuse these instead of re-typing the shape by hand.
 */
export const simplefinKeys = {
  status: ["simplefin", "status"] as const,
  accounts: ["simplefin", "accounts"] as const,
  connections: ["simplefin", "connections"] as const,
  syncSettings: ["simplefin", "syncSettings"] as const,
  alerts: ["simplefin", "alerts"] as const,
  transfers: ["simplefin", "transfers"] as const,
  importReview: ["simplefin", "importReview"] as const,
};

export function useSimpleFinStatus() {
  return useQuery<SimpleFinStatus>({
    queryKey: simplefinKeys.status,
    queryFn: async () => {
      return unwrap(commands.getSimplefinStatus());
    },
  });
}

export function useSaveSimpleFinToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (token: string) => {
      return unwrap(commands.saveSimplefinSetupToken(token));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
    },
  });
}

export function useSimpleFinConnections() {
  return useQuery<SimpleFinConnectionInfo[]>({
    queryKey: simplefinKeys.connections,
    queryFn: async () => {
      return unwrap(commands.listSimplefinConnections());
    },
  });
}

export function useSimpleFinAccounts() {
  return useQuery<SimpleFinAccountInfo[]>({
    queryKey: simplefinKeys.accounts,
    queryFn: async () => {
      return unwrap(commands.listSimplefinAccounts());
    },
    enabled: false,
  });
}

export function useImportSimpleFinAccounts() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (accounts: SimpleFinAccountImportRequest[]) => {
      return unwrap(commands.importSimplefinAccounts(accounts));
    },
    onSuccess: () => {
      // A committed SimpleFin import touches the whole ledger + accounts +
      // import state (previously under-invalidated month-totals/net-worth).
      invalidateDomains(qc, "simplefin");
      qc.invalidateQueries({ queryKey: simplefinKeys.accounts });
    },
  });
}

export function useSyncSimpleFinAccount() {
  const qc = useQueryClient();
  return useMutation<SyncSummary, Error, string>({
    mutationFn: async (accountId: string) => {
      return unwrap(commands.syncSimplefinAccount(accountId));
    },
    onSuccess: () => {
      // Sync adds/updates rows: full ledger fan-out, not just the two roots.
      invalidateDomains(qc, "simplefin");
    },
  });
}

export function useDisconnectSimpleFin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      return unwrap(commands.disconnectSimplefin());
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
      invalidateDomains(qc, "accounts", "transactions");
    },
  });
}

export function usePurgeSimpleFinData() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      return unwrap(commands.purgeSimplefinData());
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
      qc.invalidateQueries({ queryKey: simplefinKeys.accounts });
      qc.invalidateQueries({ queryKey: simplefinKeys.alerts });
      qc.invalidateQueries({ queryKey: simplefinKeys.transfers });
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
      invalidateDomains(qc, "simplefin");
      qc.invalidateQueries({ queryKey: ["onboarding"] });
    },
  });
}

export function useDeleteSimpleFinConnection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (connectionId: string) => {
      return unwrap(commands.deleteSimplefinConnection(connectionId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      invalidateDomains(qc, "accounts", "transactions");
    },
  });
}

export function useSyncAllSimpleFinAccounts() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      return unwrap(commands.syncAllSimplefinAccounts());
    },
    onSuccess: () => {
      invalidateDomains(qc, "simplefin");
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
      qc.invalidateQueries({ queryKey: simplefinKeys.alerts });
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
    },
  });
}

export function useSimpleFinSyncSettings() {
  return useQuery<SimpleFinSyncSettings>({
    queryKey: simplefinKeys.syncSettings,
    queryFn: async () => {
      return unwrap(commands.getSimplefinSyncSettings());
    },
  });
}

export function useSetSimpleFinSyncSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (settings: SimpleFinSyncSettings) => {
      return unwrap(commands.setSimplefinSyncSettings(settings));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.syncSettings });
    },
  });
}

export function useSimpleFinAlerts() {
  return useQuery<SimpleFinAlert[]>({
    queryKey: simplefinKeys.alerts,
    queryFn: async () => {
      return unwrap(commands.listSimplefinAlerts());
    },
  });
}

export function useAcknowledgeSimpleFinAlert() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (alertId: string) => {
      return unwrap(commands.acknowledgeSimplefinAlert(alertId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.alerts });
    },
  });
}

export function useSimpleFinTransferSuggestions() {
  return useQuery<TransferSuggestionInfo[]>({
    queryKey: simplefinKeys.transfers,
    queryFn: async () => {
      return unwrap(commands.listSimplefinTransferSuggestions());
    },
  });
}

export function useConfirmSimpleFinTransfer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (transferId: string) => {
      return unwrap(commands.confirmSimplefinTransfer(transferId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.transfers });
      invalidateDomains(qc, "transactions");
    },
  });
}

export function useRejectSimpleFinTransfer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (transferId: string) => {
      return unwrap(commands.rejectSimplefinTransfer(transferId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.transfers });
    },
  });
}

export function useImportReviewCandidates() {
  return useQuery<ImportCandidateWithMatches[]>({
    queryKey: simplefinKeys.importReview,
    queryFn: async () => {
      return unwrap(commands.listImportReviewCandidates());
    },
  });
}

export function useAcceptImportCandidateMatch() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ candidateId, transactionId }: { candidateId: string; transactionId: string }) => {
      return unwrap(commands.acceptImportCandidateMatch(candidateId, transactionId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
      invalidateDomains(qc, "simplefin");
    },
  });
}

export function useCreateImportCandidateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (candidateId: string) => {
      return unwrap(commands.createImportCandidateTransaction(candidateId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
      invalidateDomains(qc, "simplefin");
    },
  });
}

export function useDismissImportCandidate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (candidateId: string) => {
      return unwrap(commands.dismissImportCandidate(candidateId));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
    },
  });
}

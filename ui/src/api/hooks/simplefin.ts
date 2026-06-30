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

const simplefinKeys = {
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
      const result = await commands.getSimplefinStatus();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useSaveSimpleFinToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (token: string) => {
      const result = await commands.saveSimplefinSetupToken(token);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
      const result = await commands.listSimplefinConnections();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useSimpleFinAccounts() {
  return useQuery<SimpleFinAccountInfo[]>({
    queryKey: simplefinKeys.accounts,
    queryFn: async () => {
      const result = await commands.listSimplefinAccounts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: false,
  });
}

export function useImportSimpleFinAccounts() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (accounts: SimpleFinAccountImportRequest[]) => {
      const result = await commands.importSimplefinAccounts(accounts);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: simplefinKeys.accounts });
    },
  });
}

export function useSyncSimpleFinAccount() {
  const qc = useQueryClient();
  return useMutation<SyncSummary, Error, string>({
    mutationFn: async (accountId: string) => {
      const result = await commands.syncSimplefinAccount(accountId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
    },
  });
}

export function useDisconnectSimpleFin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.disconnectSimplefin();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useDeleteSimpleFinConnection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (connectionId: string) => {
      const result = await commands.deleteSimplefinConnection(connectionId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.connections });
      qc.invalidateQueries({ queryKey: simplefinKeys.status });
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useSyncAllSimpleFinAccounts() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.syncAllSimplefinAccounts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
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
      const result = await commands.getSimplefinSyncSettings();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useSetSimpleFinSyncSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (settings: SimpleFinSyncSettings) => {
      const result = await commands.setSimplefinSyncSettings(settings);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
      const result = await commands.listSimplefinAlerts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useAcknowledgeSimpleFinAlert() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (alertId: string) => {
      const result = await commands.acknowledgeSimplefinAlert(alertId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
      const result = await commands.listSimplefinTransferSuggestions();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useConfirmSimpleFinTransfer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (transferId: string) => {
      const result = await commands.confirmSimplefinTransfer(transferId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.transfers });
      qc.invalidateQueries({ queryKey: ["transactions"] });
    },
  });
}

export function useRejectSimpleFinTransfer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (transferId: string) => {
      const result = await commands.rejectSimplefinTransfer(transferId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
      const result = await commands.listImportReviewCandidates();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useAcceptImportCandidateMatch() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ candidateId, transactionId }: { candidateId: string; transactionId: string }) => {
      const result = await commands.acceptImportCandidateMatch(candidateId, transactionId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useCreateImportCandidateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (candidateId: string) => {
      const result = await commands.createImportCandidateTransaction(candidateId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useDismissImportCandidate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (candidateId: string) => {
      const result = await commands.dismissImportCandidate(candidateId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: simplefinKeys.importReview });
    },
  });
}

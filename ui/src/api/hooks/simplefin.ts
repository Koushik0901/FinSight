import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type SimpleFinAccountImportRequest,
  type SimpleFinStatus,
  type SimpleFinAccountInfo,
  type SyncSummary,
} from "../bindings";

const simplefinKeys = {
  status: ["simplefin", "status"] as const,
  accounts: ["simplefin", "accounts"] as const,
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
    onSuccess: () => qc.invalidateQueries({ queryKey: simplefinKeys.status }),
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
    },
  });
}

export function useSyncSimpleFinAccount() {
  const qc = useQueryClient();
  return useMutation({
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
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

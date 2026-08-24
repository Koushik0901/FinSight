import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type AgentActionBundle, type AgentExecutionEntry, type AgentSession } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { invalidateDomains } from "../invalidation";
import { actionBundleKeys } from "./_factory";
// Re-export for consumers that historically imported from this module.
export { actionBundleKeys };

export function useAgentSessions() {
  return useQuery<AgentSession[]>({
    queryKey: ["agent-sessions"],
    queryFn: async () => {
      return unwrap(api.listAgentSessions());
    },
  });
}

export function useCreateAgentSession() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ title, taskType }: { title: string; taskType: string }) => {
      return unwrap(api.createAgentSession(title, taskType));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-sessions"] });
    },
  });
}

export function useCloseAgentSession() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      await unwrap(api.closeAgentSession(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-sessions"] });
    },
  });
}

export function useActionBundles(statusFilter?: string | null, limit?: number) {
  return useQuery<AgentActionBundle[]>({
    queryKey: actionBundleKeys.list(statusFilter, null, limit),
    queryFn: async () => {
      return unwrap(api.listActionBundles(statusFilter ?? null, null, limit ?? null));
    },
  });
}

export function useSessionActionBundles(sessionId?: string | null, statusFilter?: string | null, limit?: number) {
  return useQuery<AgentActionBundle[]>({
    queryKey: actionBundleKeys.list(statusFilter, sessionId, limit),
    queryFn: async () => {
      return unwrap(api.listActionBundles(statusFilter ?? null, sessionId ?? null, limit ?? null));
    },
  });
}

export function useActionBundle(id: string | null) {
  return useQuery<AgentActionBundle | null>({
    queryKey: ["action-bundle", id],
    queryFn: async () => {
      if (!id) return null;
      return unwrap(api.getActionBundle(id));
    },
    enabled: id !== null,
  });
}

export function useApproveActionItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (itemId: string) => {
      await unwrap(api.approveActionItem(itemId));
    },
    onSuccess: () => {
      invalidateDomains(qc, "agentActions");
    },
  });
}

export function useRejectActionItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (itemId: string) => {
      await unwrap(api.rejectActionItem(itemId));
    },
    onSuccess: () => {
      invalidateDomains(qc, "agentActions");
    },
  });
}

export function useExecutionLog(bundleId: string | null) {
  return useQuery<AgentExecutionEntry[]>({
    queryKey: ["execution-log", bundleId],
    queryFn: async () => {
      if (!bundleId) return [];
      return unwrap(api.listExecutionLog(bundleId));
    },
    enabled: bundleId !== null,
  });
}

export function useExecuteActionBundle() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (bundleId: string) => {
      return unwrap(api.executeActionBundle(bundleId));
    },
    onSuccess: () => {
      // Applying a bundle mutates the ledger (agentApply = agentActions +
      // transactions fan-out) and may fund goals; plus agent memory.
      void invalidateDomains(qc, "agentApply", "goals");
      void qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}

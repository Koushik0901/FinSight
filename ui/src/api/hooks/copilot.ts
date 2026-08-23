import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AgentActionBundle, type AgentExecutionEntry, type AgentSession } from "../client";
import { unwrap } from "../client";
import { invalidateDomains } from "../invalidation";

export function useAgentSessions() {
  return useQuery<AgentSession[]>({
    queryKey: ["agent-sessions"],
    queryFn: async () => {
      return unwrap(commands.listAgentSessions());
    },
  });
}

export function useCreateAgentSession() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ title, taskType }: { title: string; taskType: string }) => {
      return unwrap(commands.createAgentSession(title, taskType));
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
      await unwrap(commands.closeAgentSession(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-sessions"] });
    },
  });
}

/**
 * Canonical keys for action-bundle queries. Any other module that needs this
 * key shape must use these factories — hand-copied literals silently become a
 * *different* cache entry the day the key gains a segment.
 */
export const actionBundleKeys = {
  /** Root, for prefix invalidation only. */
  all: ["action-bundles"] as const,
  /** List shape used by every bundle-list consumer (session slot may be null). */
  list: (statusFilter?: string | null, sessionId?: string | null, limit?: number) =>
    ["action-bundles", statusFilter ?? null, sessionId ?? null, limit ?? null] as const,
};

export function useActionBundles(statusFilter?: string | null, limit?: number) {
  return useQuery<AgentActionBundle[]>({
    queryKey: actionBundleKeys.list(statusFilter, null, limit),
    queryFn: async () => {
      return unwrap(commands.listActionBundles(statusFilter ?? null, null, limit ?? null));
    },
  });
}

export function useSessionActionBundles(sessionId?: string | null, statusFilter?: string | null, limit?: number) {
  return useQuery<AgentActionBundle[]>({
    queryKey: actionBundleKeys.list(statusFilter, sessionId, limit),
    queryFn: async () => {
      return unwrap(commands.listActionBundles(statusFilter ?? null, sessionId ?? null, limit ?? null));
    },
  });
}

export function useActionBundle(id: string | null) {
  return useQuery<AgentActionBundle | null>({
    queryKey: ["action-bundle", id],
    queryFn: async () => {
      if (!id) return null;
      return unwrap(commands.getActionBundle(id));
    },
    enabled: id !== null,
  });
}

export function useApproveActionItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (itemId: string) => {
      await unwrap(commands.approveActionItem(itemId));
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
      await unwrap(commands.rejectActionItem(itemId));
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
      return unwrap(commands.listExecutionLog(bundleId));
    },
    enabled: bundleId !== null,
  });
}

export function useExecuteActionBundle() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (bundleId: string) => {
      return unwrap(commands.executeActionBundle(bundleId));
    },
    onSuccess: () => {
      // Applying a bundle mutates the ledger (agentApply = agentActions +
      // transactions fan-out) and may fund goals; plus agent memory.
      void invalidateDomains(qc, "agentApply", "goals");
      void qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}

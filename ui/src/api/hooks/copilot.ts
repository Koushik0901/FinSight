import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type AgentActionBundle,
  type AgentExecutionEntry,
  type AgentSession,
  type ExecutionSummary,
} from "../client";
import { invalidateDomains } from "../invalidation";

export function useAgentSessions() {
  return useQuery<AgentSession[]>({
    queryKey: ["agent-sessions"],
    queryFn: async () => {
      const result = await commands.listAgentSessions();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateAgentSession() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ title, taskType }: { title: string; taskType: string }) => {
      const result = await commands.createAgentSession(title, taskType);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
      const result = await commands.closeAgentSession(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-sessions"] });
    },
  });
}

export function useActionBundles(statusFilter?: string | null, limit?: number) {
  return useQuery<AgentActionBundle[]>({
    queryKey: ["action-bundles", statusFilter ?? null, null, limit ?? null],
    queryFn: async () => {
      const result = await commands.listActionBundles(statusFilter ?? null, null, limit ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useSessionActionBundles(sessionId?: string | null, statusFilter?: string | null, limit?: number) {
  return useQuery<AgentActionBundle[]>({
    queryKey: ["action-bundles", statusFilter ?? null, sessionId ?? null, limit ?? null],
    queryFn: async () => {
      const result = await commands.listActionBundles(statusFilter ?? null, sessionId ?? null, limit ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useActionBundle(id: string | null) {
  return useQuery<AgentActionBundle | null>({
    queryKey: ["action-bundle", id],
    queryFn: async () => {
      if (!id) return null;
      const result = await commands.getActionBundle(id);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: id !== null,
  });
}

export function useApproveActionItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (itemId: string) => {
      const result = await commands.approveActionItem(itemId);
      if (result.status === "error") throw new Error(result.error.message);
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
      const result = await commands.rejectActionItem(itemId);
      if (result.status === "error") throw new Error(result.error.message);
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
      const result = await commands.listExecutionLog(bundleId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: bundleId !== null,
  });
}

export function useExecuteActionBundle() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (bundleId: string) => {
      return await invoke<ExecutionSummary>("execute_action_bundle", { bundleId });
    },
    onSuccess: () => {
      // Applying a bundle mutates the ledger (agentApply = agentActions +
      // transactions fan-out) and may fund goals; plus agent memory.
      void invalidateDomains(qc, "agentApply", "goals");
      void qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}

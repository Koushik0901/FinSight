import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type AgentActionBundle,
  type AgentExecutionEntry,
  type AgentSession,
  type ExecutionSummary,
} from "../client";

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
    queryKey: ["action-bundles", statusFilter ?? null, limit ?? null],
    queryFn: async () => {
      const result = await commands.listActionBundles(statusFilter ?? null, limit ?? null);
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
      qc.invalidateQueries({ queryKey: ["action-bundles"] });
      qc.invalidateQueries({ queryKey: ["action-bundle"] });
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
      qc.invalidateQueries({ queryKey: ["action-bundles"] });
      qc.invalidateQueries({ queryKey: ["action-bundle"] });
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
      void qc.invalidateQueries({ queryKey: ["action-bundles"] });
      void qc.invalidateQueries({ queryKey: ["action-bundle"] });
      void qc.invalidateQueries({ queryKey: ["execution-log"] });
      void qc.invalidateQueries({ queryKey: ["transactions"] });
      void qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      void qc.invalidateQueries({ queryKey: ["goals"] });
      void qc.invalidateQueries({ queryKey: ["recurring"] });
      void qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}

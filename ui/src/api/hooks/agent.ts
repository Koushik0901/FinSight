import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type AgentStatus, type CompletionProviderConfig, type ModelRoutingConfig } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

export function useNeedsReviewCount() {
  return useQuery<number>({
    queryKey: ["needs-review-count"],
    queryFn: async () => {
      return unwrap(api.getNeedsReviewCount());
    },
    refetchInterval: 30_000,
    enabled: isBackendAvailable(),
  });
}

export function useAgentStatus() {
  return useQuery<AgentStatus>({
    queryKey: ["agent-status"],
    queryFn: async () => {
      return unwrap(api.getAgentStatus());
    },
    refetchInterval: 30_000,
    staleTime: 15_000,
    enabled: isBackendAvailable(),
  });
}

export function useAskAgent() {
  return useMutation({
    mutationFn: async ({ question, mode }: { question: string; mode?: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.askAgent(question, mode ?? null));
    },
  });
}

export function useCompletionProvider() {
  return useQuery<CompletionProviderConfig>({
    queryKey: ["completionProvider"],
    queryFn: () => unwrap(api.getCompletionProvider()),
    enabled: isBackendAvailable(),
  });
}

export function useModelRouting() {
  return useQuery<ModelRoutingConfig>({
    queryKey: ["modelRouting"],
    queryFn: () => unwrap(api.getModelRouting()),
    enabled: isBackendAvailable(),
  });
}

export function useSetModelRouting() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (config: ModelRoutingConfig) => unwrap(api.setModelRouting(config)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["modelRouting"] });
    },
  });
}



export function useSetCompletionProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (config: CompletionProviderConfig) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setCompletionProvider(config));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["completion-provider"] });
    },
  });
}

export function useSaveProviderApiKey() {
  return useMutation({
    mutationFn: async ({ providerId, key }: { providerId: string; key: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.saveProviderApiKey(providerId, key));
    },
  });
}

export function useListProviderModels(config: CompletionProviderConfig | null) {
  return useQuery<string[]>({
    queryKey: ["provider-models", config],
    queryFn: async () => {
      if (!config) return [];
      return unwrap(api.listProviderModels(config));
    },
    enabled: config !== null && (config as { kind: string }).kind === "ollama" && isBackendAvailable(),
  });
}

export function useTestCompletionProvider() {
  return useMutation({
    mutationFn: async ({
      config,
      apiKey,
    }: {
      config: CompletionProviderConfig;
      apiKey?: string;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.testCompletionProvider(config, apiKey ?? null));
    },
  });
}

export function useTriggerCategorize() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.triggerCategorize());
    },
    onSuccess: () => {
      // The categorize job runs in the background; refresh once it has had a
      // moment to assign categories. Categorization changes transaction
      // categories, so the whole ledger fan-out is affected — not just status.
      setTimeout(() => invalidateDomains(qc, "transactions"), 2000);
    },
  });
}

export function useTriggerRecategorizeLowConfidence() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.triggerRecategorizeLowConfidence());
    },
    onSuccess: () => {
      // Recategorization reassigns transaction categories in the background;
      // refresh the ledger fan-out (this previously only refreshed the review
      // count, leaving spending/budget stale) plus the review action items.
      setTimeout(() => {
        void invalidateDomains(qc, "transactions");
        void qc.invalidateQueries({ queryKey: ["action-items"] });
      }, 2000);
    },
  });
}

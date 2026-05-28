import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type CompletionProviderConfig } from "../client";

export function useNeedsReviewCount() {
  return useQuery<number>({
    queryKey: ["needs-review-count"],
    queryFn: async () => {
      const result = await commands.getNeedsReviewCount();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    refetchInterval: 30_000,
  });
}

export function useSetCompletionProvider() {
  return useMutation({
    mutationFn: async (config: CompletionProviderConfig) => {
      const result = await commands.setCompletionProvider(config);
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

export function useSaveProviderApiKey() {
  return useMutation({
    mutationFn: async ({ providerId, key }: { providerId: string; key: string }) => {
      const result = await commands.saveProviderApiKey(providerId, key);
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

export function useListProviderModels(config: CompletionProviderConfig | null) {
  return useQuery<string[]>({
    queryKey: ["provider-models", config],
    queryFn: async () => {
      if (!config) return [];
      const result = await commands.listProviderModels(config);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: config !== null && (config as { kind: string }).kind === "ollama",
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
      const result = await commands.testCompletionProvider(config, apiKey ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useTriggerCategorize() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.triggerCategorize();
      if (result.status === "error") throw new Error(result.error.message);
    },
  });
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AgentRecipe, type AgentRecipeRun } from "../client";

type AppLikeError = Error & { code?: string };

const recipesKey = ["recipes"] as const;

function toError(message: string, code?: string): never {
  const err = new Error(message) as AppLikeError;
  err.code = code;
  throw err;
}

export function useRecipes(includePaused = false) {
  return useQuery<AgentRecipe[]>({
    queryKey: [...recipesKey, includePaused],
    queryFn: async () => {
      const result = await commands.listRecipes(includePaused);
      if (result.status === "error") toError(result.error.message, result.error.code);
      return result.data;
    },
    staleTime: 30_000,
  });
}

export function useCreateRecipe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      title: string;
      description: string;
      recipeKind: string;
      promptTemplate: string;
      cadence: string;
      dayOfWeek: number | null;
      dayOfMonth: number | null;
    }) => {
      const result = await commands.createRecipe(
        input.title,
        input.description,
        input.recipeKind,
        input.promptTemplate,
        input.cadence,
        input.dayOfWeek,
        input.dayOfMonth,
      );
      if (result.status === "error") toError(result.error.message, result.error.code);
      return result.data;
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: recipesKey });
    },
  });
}

export function useUpdateRecipe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      id: string;
      title: string;
      description: string;
      promptTemplate: string;
      cadence: string;
      dayOfWeek: number | null;
      dayOfMonth: number | null;
    }) => {
      const result = await commands.updateRecipe(
        input.id,
        input.title,
        input.description,
        input.promptTemplate,
        input.cadence,
        input.dayOfWeek,
        input.dayOfMonth,
      );
      if (result.status === "error") toError(result.error.message, result.error.code);
      return result.data;
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: recipesKey });
    },
  });
}

export function usePauseRecipe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.pauseRecipe(id);
      if (result.status === "error") toError(result.error.message, result.error.code);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: recipesKey });
    },
  });
}

export function useResumeRecipe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.resumeRecipe(id);
      if (result.status === "error") toError(result.error.message, result.error.code);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: recipesKey });
    },
  });
}

export function useDeleteRecipe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.deleteRecipe(id);
      if (result.status === "error") toError(result.error.message, result.error.code);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: recipesKey });
    },
  });
}

export function useTriggerRecipe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.triggerRecipe(id);
      if (result.status === "error") toError(result.error.message, result.error.code);
      return result.data;
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: recipesKey });
      void qc.invalidateQueries({ queryKey: ["action-bundles"] });
      void qc.invalidateQueries({ queryKey: ["recipe-runs"] });
    },
  });
}

export function useRecipeRuns(recipeId: string) {
  return useQuery<AgentRecipeRun[]>({
    queryKey: ["recipe-runs", recipeId],
    queryFn: async () => {
      if (!recipeId) return [];
      const result = await commands.listRecipeRuns(recipeId, null);
      if (result.status === "error") toError(result.error.message, result.error.code);
      return result.data;
    },
    enabled: recipeId.length > 0,
    staleTime: 30_000,
  });
}

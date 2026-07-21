import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ScenarioParamsInput,
  type SavedScenarioDetail,
  type ScenarioPlanProposal,
} from "../client";
import { isBackendAvailable } from "../../utils/runtime";

const KEY = ["saved-scenarios"];

/**
 * Active saved scenarios, each recomputed against the CURRENT baseline (so a
 * comparison across them is consistent) with a staleness flag. Legacy
 * result-only rows come back with `recomputable: false`.
 */
export function useSavedScenarios() {
  return useQuery<SavedScenarioDetail[]>({
    queryKey: KEY,
    queryFn: async () => {
      const result = await commands.listSavedScenarios();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable(),
  });
}

export function useRunScenario() {
  return useMutation({
    mutationFn: async ({
      description,
      months,
      params,
    }: {
      description: string;
      months: number;
      params: ScenarioParamsInput | null;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.runScenario(description, months, params);
      if (result.status === "error") {
        const err = new Error(result.error.message) as Error & { code?: string };
        err.code = result.error.code;
        throw err;
      }
      return result.data; // RanScenario { result, params, months }
    },
  });
}

export function useSaveScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({
      description,
      params,
      months,
    }: {
      description: string;
      params: ScenarioParamsInput;
      months: number;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const res = await commands.saveScenario(description, params, months);
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useDuplicateScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const res = await commands.duplicateScenario(id);
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useArchiveScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, archived }: { id: string; archived: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const res = await commands.archiveScenario(id, archived);
      if (res.status === "error") throw new Error(res.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

/** Promote a scenario into a reviewable set of proposed plan changes. This is a
 *  read-only projection — it never mutates live budgets, goals, or debt. */
export function usePromoteScenario() {
  return useMutation({
    mutationFn: async (id: string): Promise<ScenarioPlanProposal> => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const res = await commands.promoteScenario(id);
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
  });
}

export function useDeleteScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const res = await commands.deleteScenario(id);
      if (res.status === "error") throw new Error(res.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

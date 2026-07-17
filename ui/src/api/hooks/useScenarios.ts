import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ScenarioResult,
  type ScenarioParamsInput,
  type SavedScenario,
} from "../client";
import { isBackendAvailable } from "../../utils/runtime";

export function useScenarioHistory() {
  return useQuery<SavedScenario[]>({
    queryKey: ["scenario-history"],
    queryFn: async () => {
      const result = await commands.listScenarioHistory();
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
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const result = await commands.runScenario(description, months, params);
      if (result.status === "error") {
        const err = new Error(result.error.message) as Error & { code?: string };
        err.code = result.error.code;
        throw err;
      }
      return result.data;
    },
  });
}

export function useSaveScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({
      description,
      result,
    }: {
      description: string;
      result: ScenarioResult;
    }) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const res = await commands.saveScenario(description, result);
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["scenario-history"] });
    },
  });
}

export function useDeleteScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) {
        throw new Error("This action needs the desktop app runtime.");
      }
      const res = await commands.deleteScenario(id);
      if (res.status === "error") throw new Error(res.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["scenario-history"] });
    },
  });
}

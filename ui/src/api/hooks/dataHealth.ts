import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type DataHealth, type BackupInfo } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

/** Database integrity, WAL size, and the backup set (P0-4 durability panel). */
export function useDataHealth() {
  return useQuery<DataHealth>({
    queryKey: ["data-health"],
    queryFn: async () => {
      const r = await commands.getDataHealth();
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    enabled: isTauriRuntime(),
    staleTime: 30_000,
  });
}

export function useCreateBackup() {
  const qc = useQueryClient();
  return useMutation<BackupInfo>({
    mutationFn: async () => {
      const r = await commands.createManualBackup();
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["data-health"] }),
  });
}

export function useStageRestore() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (path: string) => {
      const r = await commands.stageRestoreBackup(path);
      if (r.status === "error") throw new Error(r.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["data-health"] }),
  });
}

export function useCancelRestore() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const r = await commands.cancelStagedRestore();
      if (r.status === "error") throw new Error(r.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["data-health"] }),
  });
}

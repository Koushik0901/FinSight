import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type NewPlannedTransaction,
  type PlannedTransaction,
  type PlannedTransactionPatch,
  type PlannedTxnFilter,
} from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function usePlannedTransactions(filter: Partial<PlannedTxnFilter> = {}) {
  return useQuery<PlannedTransaction[]>({
    queryKey: ["planned-transactions", filter],
    queryFn: async () => {
      const result = await commands.listPlannedTransactions({
        status: filter.status ?? null,
        dueBefore: filter.dueBefore ?? null,
      });
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCreatePlannedTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewPlannedTransaction) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createPlannedTransaction(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["planned-transactions"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useUpdatePlannedTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: PlannedTransactionPatch }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updatePlannedTransaction(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["planned-transactions"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useDeletePlannedTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.deletePlannedTransaction(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["planned-transactions"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

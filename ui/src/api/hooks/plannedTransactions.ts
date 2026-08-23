import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type NewPlannedTransaction,
  type PlannedTransaction,
  type PlannedTransactionPatch,
  type PlannedTxnFilter,
} from "../client";
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

export function usePlannedTransactions(filter: Partial<PlannedTxnFilter> = {}) {
  return useQuery<PlannedTransaction[]>({
    queryKey: ["planned-transactions", filter],
    queryFn: async () => {
      return unwrap(commands.listPlannedTransactions({
        status: filter.status ?? null,
        dueBefore: filter.dueBefore ?? null,
      }));
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreatePlannedTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewPlannedTransaction) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(commands.createPlannedTransaction(input));
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
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(commands.updatePlannedTransaction(id, patch));
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
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(commands.deletePlannedTransaction(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["planned-transactions"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

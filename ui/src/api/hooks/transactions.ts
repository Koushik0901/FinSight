import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Transaction, type TxnFilterInput, type NewTransaction, type CsvImportMapping, type ImportSummary, type TxnPatch, type UpdateTxnResult, type CategoryWithSpending, type RuleWithCategory } from "../client";

const DEFAULT_FILTER: TxnFilterInput = { accountId: null, limit: null, offset: null, search: null, filterPreset: null };

export function useTransactions(filter: TxnFilterInput = DEFAULT_FILTER) {
  return useQuery<Transaction[]>({
    queryKey: ["transactions", filter],
    queryFn: async () => {
      const result = await commands.listTransactions(filter);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewTransaction) => {
      const result = await commands.createTransaction(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

export function useImportCsv() {
  const qc = useQueryClient();
  return useMutation<ImportSummary, Error, { path: string; account_id: string; mapping: CsvImportMapping }>({
    mutationFn: async (args) => {
      const result = await commands.importCsv(args.path, args.account_id, args.mapping);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["unfinished-imports"] });
    },
  });
}

export function useUpdateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: TxnPatch }) => {
      const result = await commands.updateTransaction(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data as UpdateTxnResult;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
    },
  });
}

export function useDeleteTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.deleteTransaction(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
    },
  });
}

export function useCreateRule() {
  return useMutation({
    mutationFn: async ({ pattern, categoryId }: { pattern: string; categoryId: string }) => {
      const result = await commands.createRule(pattern, categoryId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCategories() {
  return useQuery({
    queryKey: ["categories"],
    queryFn: async () => {
      const result = await commands.listCategories();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCategoriesWithSpending() {
  return useQuery<CategoryWithSpending[]>({
    queryKey: ["categories-with-spending"],
    queryFn: async () => {
      const result = await commands.listCategoriesWithSpending();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useRulesWithCategories() {
  return useQuery<RuleWithCategory[]>({
    queryKey: ["rules"],
    queryFn: async () => {
      const result = await commands.listRulesWithCategories();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useToggleRule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, enabled }: { id: string; enabled: boolean }) => {
      const result = await commands.toggleRule(id, enabled);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
  });
}

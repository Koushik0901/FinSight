import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Transaction, type TxnFilterInput, type NewTransaction, type CsvImportMapping, type ImportSummary, type TxnPatch, type UpdateTxnResult, type CategoryDto, type CategoryWithSpending, type RuleWithCategory, type SplitInputDto } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

const DEFAULT_FILTER: TxnFilterInput = {
  accountId: null,
  limit: null,
  offset: null,
  search: null,
  filterPreset: null,
  startDate: null,
  endDate: null,
};

export function useTransactions(filter: TxnFilterInput = DEFAULT_FILTER) {
  return useQuery<Transaction[]>({
    queryKey: ["transactions", filter],
    queryFn: async () => {
      const result = await commands.listTransactions(filter);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

/** Page size for the paginated transactions list. */
export const TXN_PAGE_SIZE = 50;

/**
 * Paginated transactions via infinite query. Filters/sort/search all flow
 * through `filter`; changing any of them starts a fresh paged query (the filter
 * is part of the query key), so older transactions stay reachable via
 * `fetchNextPage` without ever loading thousands of rows at once.
 */
export function useInfiniteTransactions(
  filter: Omit<TxnFilterInput, "limit" | "offset">,
) {
  return useInfiniteQuery({
    queryKey: ["transactions-infinite", filter],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) => {
      const result = await commands.listTransactions({
        ...filter,
        limit: TXN_PAGE_SIZE,
        offset: pageParam * TXN_PAGE_SIZE,
      } as TxnFilterInput);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    getNextPageParam: (lastPage, allPages) =>
      lastPage.length < TXN_PAGE_SIZE ? undefined : allPages.length,
    enabled: isTauriRuntime(),
  });
}

export function useCreateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewTransaction) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createTransaction(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useImportCsv() {
  const qc = useQueryClient();
  return useMutation<ImportSummary, Error, { path: string; account_id: string; mapping: CsvImportMapping }>({
    mutationFn: async (args) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.importCsv(args.path, args.account_id, args.mapping);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["unfinished-imports"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
      // The import just persisted this account's mapping — drop the cached
      // lookup so a repeat import reflects the freshly-saved settings.
      qc.invalidateQueries({ queryKey: ["csv-saved-mapping"] });
    },
  });
}

export function useUpdateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: TxnPatch }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateTransaction(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data as UpdateTxnResult;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useDeleteTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.deleteTransaction(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useCreateRule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ pattern, categoryId }: { pattern: string; categoryId: string }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createRule(pattern, categoryId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
  });
}

export function useCategories() {
  return useQuery<CategoryDto[]>({
    queryKey: ["categories"],
    queryFn: async () => {
      const result = await commands.listCategories();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
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
    enabled: isTauriRuntime(),
  });
}

export function useSetCategorySpendingType() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, spendingType }: { id: string; spendingType: string | null }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setCategorySpendingType(id, spendingType);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["categories"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
    },
  });
}

const invalidateCategoryQueries = (qc: ReturnType<typeof useQueryClient>) => {
  qc.invalidateQueries({ queryKey: ["categories"] });
  qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
  qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
  qc.invalidateQueries({ queryKey: ["transactions"] });
};

export function useCreateCategory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ label, groupId, color }: { label: string; groupId: string | null; color: string }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createCategory(label, groupId, color);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useRenameCategory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, label }: { id: string; label: string }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.renameCategory(id, label);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useArchiveCategory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.archiveCategory(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateCategoryQueries(qc);
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
  });
}

export function useSetCategoryGuidance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, guidance }: { id: string; guidance: string | null }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setCategoryGuidance(id, guidance);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useUpdateCategoryColor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, color }: { id: string; color: string }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateCategoryColor(id, color);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["categories"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["recurring"] });
      qc.invalidateQueries({ queryKey: ["rules"] });
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
    enabled: isTauriRuntime(),
  });
}

export function useToggleRule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, enabled }: { id: string; enabled: boolean }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.toggleRule(id, enabled);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
  });
}

export function useSetTransactionFlags() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, isReimbursable, isSplit }: { id: string; isReimbursable: boolean; isSplit: boolean }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setTransactionFlags(id, isReimbursable, isSplit);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

export function useTransactionSplits(txnId: string | undefined) {
  return useQuery({
    queryKey: ["splits", txnId],
    queryFn: async () => {
      if (!txnId) return [];
      const result = await commands.getTransactionSplits(txnId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: !!txnId && isTauriRuntime(),
  });
}

export function useSetTransactionSplits() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ txnId, splits }: {
      txnId: string;
      splits: Array<{ categoryId: string | null; amountCents: number }>;
    }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setTransactionSplits(
        txnId,
        splits.map((s): SplitInputDto => ({ categoryId: s.categoryId, amountCents: s.amountCents }))
      );
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["splits", vars.txnId] });
      qc.invalidateQueries({ queryKey: ["categories"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

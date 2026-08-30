import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Transaction, type TxnFilterInput, type NewTransaction, type CsvImportMapping, type ImportResult, type TxnPatch, type UpdateTxnResult, type CategoryDto, type CategoryWithSpending, type CategoryGroup, type RuleWithCategory, type SplitInputDto } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

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
      return unwrap(api.listTransactions(filter));
    },
    enabled: isBackendAvailable(),
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
      return unwrap(api.listTransactions({
        ...filter,
        limit: TXN_PAGE_SIZE,
        offset: pageParam * TXN_PAGE_SIZE,
      } as TxnFilterInput));
    },
    getNextPageParam: (lastPage, allPages) =>
      lastPage.length < TXN_PAGE_SIZE ? undefined : allPages.length,
    enabled: isBackendAvailable(),
  });
}

export function useCreateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewTransaction) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createTransaction(input));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
    },
  });
}

export function useImportCsv() {
  const qc = useQueryClient();
  return useMutation<ImportResult, Error, { path: string; account_id: string; mapping: CsvImportMapping }>({
    mutationFn: async (args) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.importCsv(args.path, args.account_id, args.mapping));
    },
    onSuccess: () => {
      // A CSV commit touches the ledger, account balances, and import state
      // (mapping + any cached speculative preview).
      invalidateDomains(qc, "importCommit");
    },
  });
}

export function useUpdateTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: TxnPatch }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return (await unwrap(api.updateTransaction(id, patch))) as UpdateTxnResult;
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
    },
  });
}

export function useDeleteTransaction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.deleteTransaction(id));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
    },
  });
}

/** Mark a flagged anomaly as reviewed-and-fine (dismiss) or restore it. The
 *  detector won't re-flag a dismissed charge, so the anomaly feed stays clean. */
export function useSetAnomalyDismissed() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ txnId, dismissed }: { txnId: string; dismissed: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setAnomalyDismissed(txnId, dismissed));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
    },
  });
}

export function useCreateRule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ pattern, categoryId }: { pattern: string; categoryId: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createRule(pattern, categoryId));
    },
    onSuccess: () => {
      invalidateDomains(qc, "rules");
    },
  });
}

export function useSetTransactionOwner() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ transactionId, memberId }: { transactionId: string; memberId: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setTransactionOwner(transactionId, memberId));
    },
    // Attribution changes per-member cashflow — refresh transactions and metrics.
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
      void qc.invalidateQueries();
    },
  });
}

export function useCategories() {
  return useQuery<CategoryDto[]>({
    queryKey: ["categories"],
    queryFn: async () => {
      return unwrap(api.listCategories());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCategoriesWithSpending() {
  return useQuery<CategoryWithSpending[]>({
    queryKey: ["categories-with-spending"],
    queryFn: async () => {
      return unwrap(api.listCategoriesWithSpending());
    },
    enabled: isBackendAvailable(),
  });
}

export function useSetCategorySpendingType() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, spendingType }: { id: string; spendingType: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setCategorySpendingType(id, spendingType));
    },
    onSuccess: () => {
      invalidateDomains(qc, "categories");
    },
  });
}

const invalidateCategoryQueries = (qc: ReturnType<typeof useQueryClient>) =>
  invalidateDomains(qc, "categories");

export function useCreateCategory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ label, groupId, color }: { label: string; groupId: string | null; color: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createCategory(label, groupId, color));
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useRenameCategory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, label }: { id: string; label: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.renameCategory(id, label));
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useArchiveCategory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.archiveCategory(id));
    },
    onSuccess: () => {
      // The categories domain already includes rules (archiving a category can
      // disable its rules).
      invalidateCategoryQueries(qc);
    },
  });
}

export function useSetCategoryGuidance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, guidance }: { id: string; guidance: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setCategoryGuidance(id, guidance));
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useUpdateCategoryColor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, color }: { id: string; color: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.updateCategoryColor(id, color));
    },
    onSuccess: () => {
      invalidateDomains(qc, "categories");
    },
  });
}

export function useCategoryGroups() {
  return useQuery<CategoryGroup[]>({
    queryKey: ["category-groups"],
    queryFn: async () => {
      return unwrap(api.listCategoryGroups());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateCategoryGroup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ label, hint }: { label: string; hint?: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createCategoryGroup(label, hint ?? null));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["category-groups"] });
    },
  });
}

export function useSetCategoryGroup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ categoryId, groupId }: { categoryId: string; groupId: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setCategoryGroup(categoryId, groupId));
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}

export function useSetCategoryRollover() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, rolloverEnabled }: { id: string; rolloverEnabled: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setCategoryRollover(id, rolloverEnabled));
    },
    onSuccess: () => {
      // Rollover flips carryover for the next month, so budget envelopes also change.
      invalidateDomains(qc, "categories");
      invalidateDomains(qc, "budget");
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
    },
  });
}

export function useRulesWithCategories() {
  return useQuery<RuleWithCategory[]>({
    queryKey: ["rules"],
    queryFn: async () => {
      return unwrap(api.listRulesWithCategories());
    },
    enabled: isBackendAvailable(),
  });
}

export function useToggleRule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, enabled }: { id: string; enabled: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.toggleRule(id, enabled));
    },
    onSuccess: () => {
      invalidateDomains(qc, "rules");
    },
  });
}

export function useSetTransactionFlags() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, isReimbursable, isSplit }: { id: string; isReimbursable: boolean; isSplit: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.setTransactionFlags(id, isReimbursable, isSplit));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
    },
  });
}

/** Record the user's verdict on whether a transaction is a transfer between
 *  their own accounts. Sticky — survives re-imports and categorizer re-runs.
 *  The result reports undecided siblings with the same counterparty so the UI
 *  can offer a one-click bulk verdict. */
export function useSetTransactionTransfer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, isTransfer }: { id: string; isTransfer: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.setTransactionTransfer(id, isTransfer));
    },
    // A transfer verdict moves money in/out of income & spending — every
    // headline number (savings rate, cashflow, budget, inbox) can change.
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
      void qc.invalidateQueries();
    },
  });
}

/** Apply a transfer verdict to every undecided transaction with the same
 *  counterparty (pattern from `useSetTransactionTransfer`'s result). */
export function useApplyTransferVerdictToSimilar() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ pattern, isTransfer }: { pattern: string; isTransfer: boolean }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.applyTransferVerdictToSimilar(pattern, isTransfer));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
      void qc.invalidateQueries();
    },
  });
}

export function useTransactionSplits(txnId: string | undefined) {
  return useQuery({
    queryKey: ["splits", txnId],
    queryFn: async () => {
      if (!txnId) return [];
      return unwrap(api.getTransactionSplits(txnId));
    },
    enabled: !!txnId && isBackendAvailable(),
  });
}

export function useSetTransactionSplits() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ txnId, splits }: {
      txnId: string;
      splits: Array<{ categoryId: string | null; amountCents: number }>;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setTransactionSplits(
        txnId,
        splits.map((s): SplitInputDto => ({ categoryId: s.categoryId, amountCents: s.amountCents }))
      ));
    },
    onSuccess: (_data, vars) => {
      // A split reassigns amounts across categories, so it touches both the
      // ledger and category spending; plus this txn's own split rows.
      invalidateDomains(qc, "transactions", "categories");
      qc.invalidateQueries({ queryKey: ["splits", vars.txnId] });
    },
  });
}

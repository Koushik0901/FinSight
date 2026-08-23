import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type CategoryProposal, type UpdateTxnResult } from "../client";
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

/**
 * The categorization review queue: the CURRENT outstanding automated suggestion
 * per transaction (`category_proposals.status = 'pending'`).
 *
 * This is the source of truth for queue membership — not the old
 * `ai_confidence < 0.6 AND latest source='llm'` predicate that issue #87
 * replaced. The `needs_review` transaction-filter preset and the
 * `get_needs_review_count` badge both read the same table, so this list, the
 * filtered ledger and the badge can never drift apart.
 */
export function useCategoryProposals() {
  return useQuery<CategoryProposal[]>({
    queryKey: ["category-proposals"],
    queryFn: async () => {
      return unwrap(commands.listCategoryProposals());
    },
    enabled: isBackendAvailable(),
  });
}

/**
 * Query roots that a resolved proposal invalidates on top of whatever the
 * canonical write itself touched. `needs-review-count` is already inside the
 * `transactions` domain, but `reject` never touches the ledger — so it must be
 * listed here or the sidebar badge would keep counting a dismissed item.
 */
function invalidateQueue(qc: ReturnType<typeof useQueryClient>) {
  void qc.invalidateQueries({ queryKey: ["category-proposals"] });
  void qc.invalidateQueries({ queryKey: ["needs-review-count"] });
  void qc.invalidateQueries({ queryKey: ["action-items"] });
  void qc.invalidateQueries({ queryKey: ["inbox-badge-count"] });
}

/**
 * The user agrees with the proposed category.
 *
 * The backend delegates to `repos::transactions::update`, which writes
 * `category_id`, appends a `source='user'` categorizations audit row and calls
 * `agent_memory::upsert_correction`. None of that is reimplemented here — the
 * frontend only re-reads.
 */
export function useAcceptCategoryProposal() {
  const qc = useQueryClient();
  return useMutation<UpdateTxnResult, Error, string>({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a running FinSight backend.");
      return unwrap(commands.acceptCategoryProposal(id));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
      invalidateQueue(qc);
    },
  });
}

/** The user picks a DIFFERENT category than the one that was proposed. */
export function useCorrectCategoryProposal() {
  const qc = useQueryClient();
  return useMutation<UpdateTxnResult, Error, { id: string; categoryId: string }>({
    mutationFn: async ({ id, categoryId }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a running FinSight backend.");
      return unwrap(commands.correctCategoryProposal(id, categoryId));
    },
    onSuccess: () => {
      invalidateDomains(qc, "transactions");
      invalidateQueue(qc);
    },
  });
}

/**
 * Dismiss the suggestion without naming a replacement.
 *
 * Deliberately does NOT touch `transactions.category_id`: for an `applied`
 * proposal the category the automated pass already wrote stays exactly where it
 * is, and only the review item goes away. That is why this mutation invalidates
 * the queue but not the ledger.
 */
export function useRejectCategoryProposal() {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a running FinSight backend.");
      await unwrap(commands.rejectCategoryProposal(id));
    },
    onSuccess: () => {
      invalidateQueue(qc);
    },
  });
}

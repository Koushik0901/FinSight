import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BudgetChange, type FundingTemplate } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

/**
 * Declarative Funding Templates — Actual's `#template` as a table.
 *
 * Each template funds one category for a month, ordered by `priority` ASC.
 * `applyTemplates` computes `amountCents` capped by remaining `available` (to_budget).
 */

export function useFundingTemplates() {
  return useQuery<FundingTemplate[]>({
    queryKey: ["funding-templates"],
    queryFn: async () => unwrap(api.listFundingTemplates()),
    enabled: isBackendAvailable(),
    staleTime: 60_000,
  });
}

export function useCreateFundingTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      categoryId: string;
      kind: string;
      paramsJson?: string | null;
      priority?: number | null;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(
        api.createFundingTemplate(input.categoryId, input.kind, input.paramsJson ?? null, input.priority ?? null),
      );
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["funding-templates"] });
    },
  });
}

export function useUpdateFundingTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: {
      id: string;
      categoryId?: string | null;
      kind?: string | null;
      paramsJson?: string | null;
      priority?: number | null;
    }) =>
      unwrap(
        api.updateFundingTemplate(
          input.id,
          input.categoryId ?? null,
          input.kind ?? null,
          input.paramsJson ?? null,
          input.priority ?? null,
        ),
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["funding-templates"] }),
  });
}

export function useDeleteFundingTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.deleteFundingTemplate(id));
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["funding-templates"] }),
  });
}

export function useApplyTemplates() {
  return useMutation({
    mutationFn: async (month: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.applyTemplates(month)) as Promise<BudgetChange[]>;
    },
  });
}

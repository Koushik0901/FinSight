import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type RuleProposal } from "../openapiClient";
import { unwrap } from "../openapiClient";

export function useRuleProposals() {
  return useQuery<RuleProposal[]>({
    queryKey: ["rule-proposals"],
    queryFn: async () => {
      return unwrap(api.listRuleProposals());
    },
  });
}

export function useAcceptRuleProposal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      await unwrap(api.acceptRuleProposal(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rule-proposals"] });
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
  });
}

export function useDeclineRuleProposal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      await unwrap(api.declineRuleProposal(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rule-proposals"] });
    },
  });
}

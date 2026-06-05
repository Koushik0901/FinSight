import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type RuleProposal } from "../client";

export function useRuleProposals() {
  return useQuery<RuleProposal[]>({
    queryKey: ["rule-proposals"],
    queryFn: async () => {
      const result = await commands.listRuleProposals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useAcceptRuleProposal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.acceptRuleProposal(id);
      if (result.status === "error") throw new Error(result.error.message);
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
      const result = await commands.declineRuleProposal(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rule-proposals"] });
    },
  });
}

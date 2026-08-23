import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type RuleProposal } from "../client";
import { unwrap } from "../client";

export function useRuleProposals() {
  return useQuery<RuleProposal[]>({
    queryKey: ["rule-proposals"],
    queryFn: async () => {
      return unwrap(commands.listRuleProposals());
    },
  });
}

export function useAcceptRuleProposal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      await unwrap(commands.acceptRuleProposal(id));
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
      await unwrap(commands.declineRuleProposal(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rule-proposals"] });
    },
  });
}

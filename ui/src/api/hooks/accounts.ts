import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AccountSummary, type NewAccount } from "../client";

export function useAccounts() {
  return useQuery<AccountSummary[]>({
    queryKey: ["accounts"],
    queryFn: async () => {
      const result = await commands.listAccounts();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewAccount) => {
      const result = await commands.createAccount(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["onboarding-state"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

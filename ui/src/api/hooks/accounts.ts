import { useQuery } from "@tanstack/react-query";
import { commands, type AccountSummary } from "../client";

export function useAccounts() {
  return useQuery<AccountSummary[]>({
    queryKey: ["accounts"],
    queryFn: async () => {
      const result = await commands.listAccounts();
      if (result.status === "error") {
        throw new Error(result.error.message);
      }
      return result.data;
    },
  });
}

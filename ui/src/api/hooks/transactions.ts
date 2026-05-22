import { useQuery } from "@tanstack/react-query";
import { commands, type Transaction, type TxnFilterInput } from "../client";

const DEFAULT_FILTER: TxnFilterInput = {
  accountId: null,
  limit: null,
  offset: null,
};

export function useTransactions(filter: TxnFilterInput = DEFAULT_FILTER) {
  return useQuery<Transaction[]>({
    queryKey: ["transactions", filter],
    queryFn: async () => {
      const result = await commands.listTransactions(filter);
      if (result.status === "error") {
        throw new Error(result.error.message);
      }
      return result.data;
    },
  });
}

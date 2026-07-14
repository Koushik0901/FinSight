import { useQuery } from "@tanstack/react-query";
import { commands, type Position, type InvestmentSummary } from "../client";

/** Open positions for an investment account, derived from imported trade rows. */
export function useAccountPositions(accountId: string | null, enabled = true) {
  return useQuery<Position[]>({
    queryKey: ["investment-positions", accountId],
    queryFn: async () => {
      const r = await commands.listAccountPositions(accountId!);
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    enabled: !!accountId && enabled,
    staleTime: 30_000,
  });
}

/** Cash + positions-at-last-trade-price estimate for an investment account. */
export function useInvestmentSummary(accountId: string | null, enabled = true) {
  return useQuery<InvestmentSummary>({
    queryKey: ["investment-summary", accountId],
    queryFn: async () => {
      const r = await commands.getInvestmentSummary(accountId!);
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    enabled: !!accountId && enabled,
    staleTime: 30_000,
  });
}

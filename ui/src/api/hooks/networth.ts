import { useQuery } from "@tanstack/react-query";
import { commands, type NetWorthPoint } from "../client";
import { useManualAssets, useLiabilities } from "./assets";
import { useAccounts } from "./accounts";

/** Net-worth snapshot history for the §3a chart. */
export function useNetWorthHistory(days: number) {
  return useQuery<NetWorthPoint[]>({
    queryKey: ["networth-history", days],
    queryFn: async () => {
      const result = await commands.listNetWorthHistory(days);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

/** Live net worth = accounts + manual assets − liabilities.
 *  `useAccounts` returns only non-archived accounts (matches the backend
 *  snapshot, which also excludes archived accounts), so the chart and this
 *  headline agree. */
export function useNetWorth(): number {
  const { data: accounts = [] } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const { data: liabilities = [] } = useLiabilities();
  const accountCents = accounts.reduce((s, a) => s + a.balance_cents, 0);
  const assetCents = assets.reduce((s, a) => s + a.valueCents, 0);
  const liabilityCents = liabilities.reduce((s, l) => s + l.balanceCents, 0);
  return accountCents + assetCents - liabilityCents;
}

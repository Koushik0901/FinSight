import { useQuery } from "@tanstack/react-query";
import { commands, type NetWorthPoint } from "../client";
import { useManualAssets, useLiabilities } from "./assets";
import { useAccounts } from "./accounts";
import { isTauriRuntime } from "../../utils/runtime";

/** Net-worth snapshot history for the §3a chart. */
export function useNetWorthHistory(days: number) {
  return useQuery<NetWorthPoint[]>({
    queryKey: ["networth-history", days],
    queryFn: async () => {
      const result = await commands.listNetWorthHistory(days);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
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
  // Accounts with no confirmed balance (e.g. CSV-imported history with no
  // balance field) are excluded rather than silently counted as $0 — a
  // fabricated zero would understate or overstate net worth without saying
  // so. Mirrors the same exclusion in net_worth::record_today on the backend.
  const accountCents = accounts
    .filter((a) => a.balance_known)
    .reduce((s, a) => s + a.balance_cents, 0);
  const assetCents = assets.reduce((s, a) => s + a.valueCents, 0);
  const liabilityCents = liabilities.reduce((s, l) => s + l.balanceCents, 0);
  return accountCents + assetCents - liabilityCents;
}

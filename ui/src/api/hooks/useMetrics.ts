import { useQuery } from "@tanstack/react-query";
import { client } from "../openapiClient";
export const explain = (b: "displayMedian"|"recentMean90"|"safetyConservative") => ({
  displayMedian: "Smooth average — ignores one-offs so your budget doesn't spike.",
  recentMean90: "Recent average — catches step-ups like rent quickly.",
  safetyConservative: "Conservative — the higher of yearly and recent, so safety is never overstated.",
}[b]);
export const useReconcile = (a: string, b: string, scope?: string) => useQuery({
  queryKey: ["reconcile", a, b, scope],
  queryFn: async () => {
    const r = await client.POST("/api/rpc/reconcileBases" as never, {
      body: { basisA: a, basisB: b, scope },
    } as never);
    return (r as { data?: unknown }).data;
  },
});

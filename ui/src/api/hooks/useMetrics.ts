import { useQuery } from "@tanstack/react-query";
import { client } from "../openapiClient";

export type Basis = "displayMedian" | "recentMean90" | "safetyConservative";

export const explain = (b: Basis) =>
  ({
    displayMedian: "Smooth average — ignores one-offs so your budget doesn't spike.",
    recentMean90: "Recent average — catches step-ups like rent quickly.",
    safetyConservative:
      "Conservative — the higher of yearly and recent, so safety is never overstated.",
  })[b];

export const useReconcile = (a: Basis, b: Basis, scope?: string) =>
  useQuery({
    queryKey: ["reconcile", a, b, scope ?? null],
    queryFn: async () => {
      const r = await client.POST("/api/rpc/reconcile_bases", {
        // generated openapi.ts types body as Record<string, never> — cast needed until spec has typed schema; ExpenseBasis deserializes via serde rename_all camelCase
        body: { basisA: a, basisB: b, scope } as any,
      });
      if ((r as { error?: unknown }).error) {
        throw new Error(String((r as { error?: unknown }).error));
      }
      return (r as { data?: unknown }).data;
    },
    enabled: !!a && !!b,
  });

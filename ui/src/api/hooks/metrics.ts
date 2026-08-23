import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  type FinancialMetrics,
  type FinancialAssumptionsInput,
  type FinancialPhilosophyDto,
  type MetricExplanation,
} from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

/**
 * Canonical financial numbers from the shared `finsight-core::metrics` layer —
 * balances, trailing averages, runway, emergency-fund coverage, and the user's
 * targets. Screens read these instead of recomputing, so the UI and the Copilot
 * never disagree.
 */
export function useFinancialMetrics(memberId?: string | null) {
  return useQuery<FinancialMetrics>({
    // memberId in the key so switching person refetches; null/undefined = the
    // whole household (unchanged behaviour).
    queryKey: ["financial-metrics", memberId ?? null],
    queryFn: async () => {
      return unwrap(api.getFinancialMetrics(memberId ?? null));
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isBackendAvailable(),
  });
}

/**
 * Structured "explain this number" provenance for the dashboard metrics —
 * definition, inputs, exclusions, assumptions, period, and data-quality
 * warnings. The values come from the same shared metrics layer as
 * {@link useFinancialMetrics}, so an explanation can never disagree with the
 * number shown. Returned as a lookup keyed by the stable metric `key`
 * (net_worth, savings_rate, runway_days, …) so a card can grab just its own.
 */
export function useMetricExplanations(memberId?: string | null) {
  return useQuery<Record<string, MetricExplanation>>({
    queryKey: ["metric-explanations", memberId ?? null],
    queryFn: async () => {
      const explanations = await unwrap(api.explainFinancialMetrics(memberId ?? null));
      return Object.fromEntries(explanations.map((e) => [e.key, e]));
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

/**
 * Structured "explain this goal" provenance (#71) — one explanation per active
 * goal, keyed `goal:{id}`, from the same completion projection the plan and the
 * Copilot use. A goal with no monthly contribution withholds a date (its value
 * is `withheld`) rather than inventing one. Same shape as
 * {@link useMetricExplanations}, so the shared inspector renders it.
 */
export function useGoalExplanations() {
  return useQuery<Record<string, MetricExplanation>>({
    queryKey: ["goal-explanations"],
    queryFn: async () => {
      const explanations = await unwrap(api.explainGoals());
      return Object.fromEntries(explanations.map((e) => [e.key, e]));
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

export function useSetFinancialAssumptions() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: FinancialAssumptionsInput) => {
      await unwrap(api.setFinancialAssumptions(input));
    },
    onSuccess: () => {
      // Targets feed the metrics response and the compound projector.
      qc.invalidateQueries({ queryKey: ["financial-metrics"] });
      qc.invalidateQueries({ queryKey: ["goal-projection"] });
    },
  });
}

/**
 * The user's stated financial philosophy — which debt-payoff school they
 * subscribe to, and where they draw the line between paying debt down and
 * investing instead.
 *
 * These reach the deterministic engines and the Copilot's live prompt, not just
 * the wording, so changing one changes the advice.
 */
export function useFinancialPhilosophy() {
  return useQuery<FinancialPhilosophyDto>({
    queryKey: ["financial-philosophy"],
    queryFn: async () => {
      return unwrap(api.getFinancialPhilosophy());
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

export function useSetFinancialPhilosophy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: FinancialPhilosophyDto) => {
      await unwrap(api.setFinancialPhilosophy(input));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["financial-philosophy"] });
      // The philosophy changes debt ranking and the high-interest threshold,
      // so anything derived from those is now stale.
      qc.invalidateQueries({ queryKey: ["financial-metrics"] });
      qc.invalidateQueries({ queryKey: ["inbox"] });
    },
  });
}

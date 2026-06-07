import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type OnboardingState, type SeedSummary } from "../client";

const KEY = ["onboarding-state"] as const;

export function useOnboardingState() {
  return useQuery<OnboardingState>({
    queryKey: KEY,
    queryFn: async () => {
      const result = await commands.getOnboardingState();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 5_000,
  });
}

export function useSeedSampleHousehold() {
  const qc = useQueryClient();
  return useMutation<SeedSummary>({
    mutationFn: async () => {
      const result = await commands.seedSampleHousehold();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEY });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}

export function useMarkOnboardingComplete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.markOnboardingComplete();
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useResetOnboarding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.resetOnboardingCompletion();
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useSeedDevDemo() {
  const qc = useQueryClient();
  return useMutation<SeedSummary>({
    mutationFn: async () => {
      const result = await commands.seedDevDemo();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      // Invalidate every query key so all screens reflect the new data.
      qc.invalidateQueries({ queryKey: KEY });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["categories-spending"] });
      qc.invalidateQueries({ queryKey: ["goals"] });
      qc.invalidateQueries({ queryKey: ["recurring"] });
      qc.invalidateQueries({ queryKey: ["assets"] });
      qc.invalidateQueries({ queryKey: ["liabilities"] });
      qc.invalidateQueries({ queryKey: ["net-worth-history"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
    },
  });
}

export function useClearSampleData() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.clearSampleData();
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEY });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
    },
  });
}

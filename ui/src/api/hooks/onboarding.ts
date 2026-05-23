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

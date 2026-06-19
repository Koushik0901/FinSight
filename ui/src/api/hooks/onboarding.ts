import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type OnboardingState, type SeedSummary } from "../client";
import { isTauriRuntime, userErrorMessage } from "../../utils/runtime";

const KEY = ["onboarding-state"] as const;

export function useOnboardingState() {
  return useQuery<OnboardingState>({
    queryKey: KEY,
    queryFn: async () => {
      const result = await commands.getOnboardingState();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
    staleTime: 5_000,
  });
}

export function useSeedSampleHousehold() {
  const qc = useQueryClient();
  return useMutation<SeedSummary>({
    mutationFn: async () => {
      if (!isTauriRuntime()) {
        throw new Error(userErrorMessage(new Error("missing tauri invoke")));
      }
      const result = await commands.seedSampleHousehold();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEY });
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
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
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["goals"] });
      qc.invalidateQueries({ queryKey: ["recurring"] });
      qc.invalidateQueries({ queryKey: ["assets"] });
      qc.invalidateQueries({ queryKey: ["liabilities"] });
      qc.invalidateQueries({ queryKey: ["net-worth-history"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
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
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      qc.invalidateQueries({ queryKey: ["categories-with-spending"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["spending-breakdown"] });
      qc.invalidateQueries({ queryKey: ["journey-status"] });
    },
  });
}

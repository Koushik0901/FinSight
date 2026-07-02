import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type OnboardingState } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

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

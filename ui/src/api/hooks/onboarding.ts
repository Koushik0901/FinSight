import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type OnboardingState } from "../client";
import { unwrap } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

const KEY = ["onboarding-state"] as const;

export function useOnboardingState() {
  return useQuery<OnboardingState>({
    queryKey: KEY,
    queryFn: async () => {
      return unwrap(commands.getOnboardingState());
    },
    enabled: isBackendAvailable(),
    staleTime: 5_000,
  });
}

export function useMarkOnboardingComplete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await unwrap(commands.markOnboardingComplete());
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useResetOnboarding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await unwrap(commands.resetOnboardingCompletion());
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

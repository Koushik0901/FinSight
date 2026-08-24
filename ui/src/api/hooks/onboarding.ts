import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type OnboardingState } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";

const KEY = ["onboarding-state"] as const;

export function useOnboardingState() {
  return useQuery<OnboardingState>({
    queryKey: KEY,
    queryFn: async () => {
      return unwrap(api.getOnboardingState());
    },
    enabled: isBackendAvailable(),
    staleTime: 5_000,
  });
}

export function useMarkOnboardingComplete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await unwrap(api.markOnboardingComplete());
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useResetOnboarding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await unwrap(api.resetOnboardingCompletion());
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

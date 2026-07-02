import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import type { OnboardingState } from "../api/client";

export function shouldShowOnboarding(
  onboarding: OnboardingState | undefined
): boolean {
  if (!onboarding) return false;
  return onboarding.account_count === 0 && !onboarding.completion_marked;
}

export function useOnboardingRedirect(
  onboarding: OnboardingState | undefined
) {
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    if (shouldShowOnboarding(onboarding) && location.pathname !== "/onboarding") {
      navigate("/onboarding", { replace: true });
    }
  }, [onboarding, location.pathname, navigate]);
}

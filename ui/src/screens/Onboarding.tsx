import { lazy, Suspense } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useMarkOnboardingComplete } from "../api/hooks/onboarding";
import StepAccounts from "./onboarding/StepAccounts";
import type { OnboardingStep } from "../state/onboarding";

const StepHistory = lazy(() => import("./onboarding/StepHistory"));
const StepCategories = lazy(() => import("./onboarding/StepCategories"));
const StepAgent = lazy(() => import("./onboarding/StepAgent"));

const STEP_TITLES: Record<OnboardingStep, string> = {
  accounts: "Accounts",
  history: "History",
  categories: "Categories",
  agent: "Agent",
};

export default function Onboarding() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const markComplete = useMarkOnboardingComplete();
  const requestedStep = searchParams.get("focus") as OnboardingStep | null;
  const step: OnboardingStep = requestedStep && requestedStep in STEP_TITLES ? requestedStep : "accounts";
  const isFocusedTask = step !== "accounts" || searchParams.has("focus");

  const exitToToday = () => navigate("/", { replace: true });
  const finishSetup = async () => {
    try {
      await markComplete.mutateAsync();
    } finally {
      exitToToday();
    }
  };
  const openTask = (next: Exclude<OnboardingStep, "accounts">) => {
    navigate(`/onboarding?focus=${next}`);
  };

  return (
    <div className="onboarding-shell onb-shell onb-fullscreen" data-testid="onboarding-shell">
      <header className="onb-top">
        <div className="brand" style={{ padding: 0 }}>
          <div className="mark" aria-hidden="true" />
          <div className="wm">FinSight</div>
        </div>
        <div className="onb-context-label">
          {isFocusedTask ? `Optional setup · ${STEP_TITLES[step]}` : "First step · Accounts"}
        </div>
        <div className="onb-header-actions">
          {step !== "accounts" && (
            <button className="btn ghost sm" type="button" onClick={() => navigate("/onboarding")}>
              Back to setup
            </button>
          )}
          <button className="btn ghost sm onb-exit" type="button" onClick={exitToToday}>
            Exit setup
          </button>
        </div>
      </header>

      <main id="main" tabIndex={-1} className="onboarding-step onb-stage" aria-label="Onboarding steps">
        <Suspense fallback={<div className="onb-loading">Loading setup…</div>}>
          {step === "accounts" && (
            <StepAccounts onNext={finishSetup} onOptional={openTask} />
          )}
          {step === "history" && (
            <StepHistory onBack={() => navigate("/onboarding?focus=accounts")} onNext={exitToToday} />
          )}
          {step === "categories" && (
            <StepCategories onNext={() => navigate("/categories")} />
          )}
          {step === "agent" && <StepAgent onDone={exitToToday} />}
        </Suspense>
      </main>
    </div>
  );
}

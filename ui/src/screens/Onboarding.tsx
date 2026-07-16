import { useNavigate } from "react-router-dom";
import { STEP_ORDER, useOnboardingStore } from "../state/onboarding";
import { useOnboardingState } from "../api/hooks/onboarding";
import StepWelcome from "./onboarding/StepWelcome";
import StepAccounts from "./onboarding/StepAccounts";
import StepHistory from "./onboarding/StepHistory";
import StepCategories from "./onboarding/StepCategories";
import StepAgent from "./onboarding/StepAgent";

const STEP_TITLES: Record<string, string> = {
  welcome: "Welcome",
  accounts: "Accounts",
  history: "History",
  categories: "Categories",
  agent: "Agent",
};

export default function Onboarding() {
  const navigate = useNavigate();
  const { step, setStep, reachedSteps } = useOnboardingStore();
  const { data: _state } = useOnboardingState();
  const stepIndex = STEP_ORDER.indexOf(step);

  return (
    <div className="onboarding-shell onb-shell onb-fullscreen" data-testid="onboarding-shell">
      <header className="onb-top">
        <div className="brand" style={{ padding: 0 }}>
          <div className="mark" aria-hidden="true" />
          <div className="wm">FinSight</div>
        </div>
        <nav className="onb-steps" aria-label="Onboarding progress">
          {STEP_ORDER.map((s) => {
            const reached = reachedSteps.has(s);
            const isCurrent = s === step;
            return (
              <button
                key={s}
                className={`onb-step-pip ${isCurrent ? "cur" : ""} ${reached ? "done" : ""}`}
                disabled={!reached}
                onClick={() => reached && setStep(s)}
                aria-current={isCurrent ? "step" : undefined}
                aria-label={`Go to ${STEP_TITLES[s]} step`}
                title={STEP_TITLES[s]}
                type="button"
              />
            );
          })}
        </nav>
        <div className="onb-step-label">
          Step <span className="num">{stepIndex + 1}</span> of {STEP_ORDER.length} · {STEP_TITLES[step]}
        </div>
      </header>

      <section className="onboarding-step onb-stage" aria-label="Onboarding steps">
        {step === "welcome"    && <StepWelcome onNext={() => setStep("accounts")} onSkipToToday={() => navigate("/", { replace: true })} />}
        {step === "accounts"   && <StepAccounts onNext={() => setStep("history")} />}
        {step === "history"    && <StepHistory onBack={() => setStep("accounts")} onNext={() => setStep("categories")} />}
        {step === "categories" && <StepCategories onNext={() => setStep("agent")} />}
        {step === "agent"      && <StepAgent onDone={() => navigate("/", { replace: true })} />}
      </section>
    </div>
  );
}

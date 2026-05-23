import { useState } from "react";
import { useSeedSampleHousehold, useMarkOnboardingComplete } from "../../api/hooks/onboarding";

interface Props {
  onNext: () => void;
  onSkipToToday: () => void;
}

export default function StepWelcome({ onNext, onSkipToToday }: Props) {
  const seedSample = useSeedSampleHousehold();
  const markComplete = useMarkOnboardingComplete();
  const [seedError, setSeedError] = useState<string | null>(null);

  async function trySample() {
    setSeedError(null);
    try {
      await seedSample.mutateAsync();
      await markComplete.mutateAsync();
      onSkipToToday();
    } catch (err) {
      setSeedError(err instanceof Error ? err.message : "Something went wrong. Please try again.");
    }
  }

  return (
    <div className="step-welcome">
      <h1>A quiet way to understand your money</h1>
      <p>
        FinSight is a local, encrypted notebook for your accounts. Nothing leaves
        your machine. We'll help you import a statement, add accounts by hand, or
        explore with realistic sample data — whichever feels right today.
      </p>
      <div className="actions">
        <button className="primary" onClick={onNext}>Get started →</button>
        <button
          className="tertiary"
          onClick={trySample}
          disabled={seedSample.isPending}
          data-testid="try-sample-data"
        >
          {seedSample.isPending ? "Seeding…" : "Try with sample data"}
        </button>
      </div>
      {seedError && <p role="alert" className="error-message">{seedError}</p>}
    </div>
  );
}

import { useState } from "react";
import { useSeedSampleHousehold, useMarkOnboardingComplete } from "../../api/hooks/onboarding";
import { userErrorMessage } from "../../utils/runtime";
import Button from "../../components/Button";

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
      setSeedError(userErrorMessage(err, "Could not load sample data. Try again from the desktop app."));
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
        <Button variant="primary" onClick={onNext}>Get started →</Button>
        <Button
          variant="ghost"
          onClick={trySample}
          disabled={seedSample.isPending}
          loading={seedSample.isPending}
          data-testid="try-sample-data"
        >
          {seedSample.isPending ? "Seeding…" : "Try with sample data"}
        </Button>
      </div>
      {seedError && <p role="alert" className="error-message">{seedError}</p>}
    </div>
  );
}

import { useState } from "react";
import Button from "../../components/Button";
import Input from "../../components/Input";
import { useMarkOnboardingComplete } from "../../api/hooks/onboarding";
import { userErrorMessage } from "../../utils/runtime";
import { useCreateHouseholdMember, useSetSelfMember } from "../../api/hooks/household";

interface Props {
  onNext: () => void;
  onSkipToToday: () => void;
}

export default function StepWelcome({ onNext, onSkipToToday }: Props) {
  const [name, setName] = useState("");
  const createMember = useCreateHouseholdMember();
  const setSelf = useSetSelfMember();
  const markComplete = useMarkOnboardingComplete();
  const [skipError, setSkipError] = useState<string | null>(null);

  // Capturing the operator's name up front (optional) lets FinSight recognize
  // THEIR own e-transfers ("To/From: <you>") as internal moves from the very
  // first import, instead of miscounting them as income/spending until the user
  // later marks themselves via "This is me". Best-effort: never blocks setup.
  const handleGetStarted = async () => {
    const trimmed = name.trim();
    if (trimmed) {
      try {
        const member = await createMember.mutateAsync({ name: trimmed });
        await setSelf.mutateAsync(member.id);
      } catch {
        // A name is optional — proceed to setup regardless.
      }
    }
    onNext();
  };

  const handleSkip = async () => {
    setSkipError(null);
    try {
      await markComplete.mutateAsync();
      onSkipToToday();
    } catch (err) {
      setSkipError(userErrorMessage(err, "Could not skip setup. Please try again."));
    }
  };

  return (
    <div className="step-welcome onb-split">
      <div className="onb-left">
        <div className="num-step">001 · Welcome</div>
        <h1>A quiet way to understand your money.</h1>
        <p className="lead">
          FinSight is local-first and encrypted. Nothing leaves your machine. Start with the accounts you want to track,
          then bring in history from statements or secure bank sync.
        </p>
        <div className="row row-sm wrap" style={{ marginBottom: 20 }}>
          <span className="chip"><span className="dot" /> Local-first</span>
          <span className="chip">Encrypted</span>
          <span className="chip">No ads</span>
        </div>
        <Input
          className="onb-name-field"
          label={
            <span>
              Your name <span className="muted">(optional)</span>
            </span>
          }
          hint="Helps recognize transfers between your own accounts."
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. John Doe"
          aria-label="Your name"
          autoComplete="name"
        />
        <div className="onb-actions">
          <Button variant="primary" onClick={() => void handleGetStarted()}>
            Get started →
          </Button>
          <Button
            variant="outline"
            onClick={() => void handleSkip()}
            loading={markComplete.isPending}
          >
            {markComplete.isPending ? "Skipping…" : "Skip setup"}
          </Button>
        </div>
        {skipError && (
          <p role="alert" className="err onb-action-error">
            {skipError}
          </p>
        )}
      </div>

      <div className="onb-right">
        <div className="onb-art-grid">
          <div className="card">
            <div className="eyebrow"><span className="dot" />Example · Today snapshot</div>
            <h2 className="h1" style={{ fontSize: 30, marginTop: 14, lineHeight: 1.15, fontWeight: 500 }}>
              See <span className="figure" style={{ color: "var(--accent)" }}>net worth</span> across every account.
            </h2>
            <p className="muted" style={{ fontSize: 14, marginTop: 14, lineHeight: 1.55 }}>
              Once you import a statement, this is where your real balances, runway, and recurring costs appear.
            </p>
          </div>
          <div className="onb-kpis">
            <div className="card tight">
              <div className="eyebrow">Runway</div>
              <div className="figure muted" style={{ fontSize: 26, marginTop: 6 }}>—</div>
            </div>
            <div className="card tight">
              <div className="eyebrow">Recurring</div>
              <div className="figure muted" style={{ fontSize: 26, marginTop: 6 }}>—</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

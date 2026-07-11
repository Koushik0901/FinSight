import { useState } from "react";
import Button from "../../components/Button";
import { useCreateHouseholdMember, useSetSelfMember } from "../../api/hooks/household";

interface Props {
  onNext: () => void;
  onSkipToToday: () => void;
}

export default function StepWelcome({ onNext, onSkipToToday }: Props) {
  const [name, setName] = useState("");
  const createMember = useCreateHouseholdMember();
  const setSelf = useSetSelfMember();

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

  return (
    <div className="step-welcome onb-split">
      <div className="onb-left">
        <div className="num-step">001 · Welcome</div>
        <h1>A quiet way to understand your money.</h1>
        <p className="lead">
          FinSight is local-first and encrypted. Nothing leaves your machine. Start by importing a statement,
          connecting SimpleFIN, or adding accounts manually.
        </p>
        <div className="row row-sm wrap" style={{ marginBottom: 20 }}>
          <span className="chip"><span className="dot" /> Local-first</span>
          <span className="chip">Encrypted</span>
          <span className="chip">No ads</span>
        </div>
        <label style={{ display: "block", marginBottom: 16, maxWidth: 340 }}>
          <span className="muted" style={{ fontSize: 13 }}>
            Your name <span style={{ opacity: 0.7 }}>(optional — so your own transfers between accounts aren’t counted as income or spending)</span>
          </span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Koushik"
            aria-label="Your name"
            style={{ marginTop: 6, width: "100%" }}
          />
        </label>
        <div className="onb-actions">
          <Button variant="primary" onClick={() => void handleGetStarted()}>Get started →</Button>
          <Button variant="outline" onClick={onSkipToToday}>Skip setup</Button>
        </div>
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

import Button from "../../components/Button";

interface Props {
  onNext: () => void;
  onSkipToToday: () => void;
}

export default function StepWelcome({ onNext, onSkipToToday }: Props) {
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
        <div className="onb-actions">
          <Button variant="primary" onClick={onNext}>Get started →</Button>
          <Button variant="outline" onClick={onSkipToToday}>Skip setup</Button>
        </div>
      </div>

      <div className="onb-right">
        <div className="onb-art-grid">
          <div className="card">
            <div className="eyebrow"><span className="dot" />Today snapshot</div>
            <h2 className="h1" style={{ fontSize: 30, marginTop: 14, lineHeight: 1.15, fontWeight: 500 }}>
              You have <span className="figure" style={{ color: "var(--accent)" }}>$48,920</span> across 6 accounts.
            </h2>
            <p className="muted" style={{ fontSize: 14, marginTop: 14, lineHeight: 1.55 }}>
              You are tracking below last month and one subscription likely needs your attention.
            </p>
          </div>
          <div className="onb-kpis">
            <div className="card tight">
              <div className="eyebrow">Runway</div>
              <div className="figure" style={{ fontSize: 26, marginTop: 6 }}>134 <span className="muted" style={{ fontSize: 14 }}>days</span></div>
            </div>
            <div className="card tight">
              <div className="eyebrow">Recurring</div>
              <div className="figure" style={{ fontSize: 26, marginTop: 6 }}>$2,584<span className="muted" style={{ fontSize: 14 }}>/mo</span></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

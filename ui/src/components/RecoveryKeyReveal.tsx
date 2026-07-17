import { useState } from "react";
import { toast } from "sonner";
import Button from "./Button";

/**
 * Shows a freshly-issued recovery key exactly once. The key is never
 * persisted client-side — it lives only in this component's props for the
 * lifetime of the render. Used by SetupScreen (Task 7) and the admin
 * "add user" flow (Task 8).
 */
export function RecoveryKeyReveal({
  recoveryKey,
  onContinue,
}: {
  recoveryKey: string;
  onContinue: () => void;
}) {
  const [confirmed, setConfirmed] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(recoveryKey);
      setCopied(true);
      toast.success("Recovery key copied");
    } catch {
      toast.error("Could not copy — select and copy the key manually");
    }
  };

  return (
    <div className="card recovery-key-reveal">
      <p className="eyebrow">Save your recovery key</p>
      <h1 className="h1" style={{ fontSize: 22 }}>This is shown once</h1>
      <p className="muted" style={{ marginTop: 8, fontSize: 13.5, lineHeight: 1.55 }}>
        If you ever forget your password, this recovery key is the only way back into your data. Store it
        somewhere safe — a password manager or a printed copy. FinSight does not store it anywhere.
      </p>

      <div
        className="recovery-key-block"
        style={{
          marginTop: 16,
          padding: "14px 16px",
          background: "var(--surface-2)",
          border: "1px solid var(--line)",
          borderRadius: "var(--radius, 10px)",
          fontFamily: "var(--mono)",
          fontSize: 15,
          letterSpacing: 0.5,
          wordBreak: "break-all",
        }}
      >
        {recoveryKey}
      </div>

      <Button type="button" variant="outline" size="sm" style={{ marginTop: 12 }} onClick={() => void handleCopy()}>
        {copied ? "Copied" : "Copy to clipboard"}
      </Button>

      <label className="row row-sm" style={{ marginTop: 22, alignItems: "center", gap: 8 }}>
        <input
          type="checkbox"
          checked={confirmed}
          onChange={(e) => setConfirmed(e.target.checked)}
          aria-label="I saved my recovery key"
        />
        <span>I saved my recovery key</span>
      </label>

      <Button
        type="button"
        variant="primary"
        style={{ marginTop: 16, width: "100%" }}
        disabled={!confirmed}
        onClick={onContinue}
      >
        Continue
      </Button>
    </div>
  );
}

export default RecoveryKeyReveal;

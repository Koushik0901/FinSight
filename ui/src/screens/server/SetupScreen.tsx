import { useState, type FormEvent } from "react";
import { toast } from "sonner";
import Button from "../../components/Button";
import Input from "../../components/Input";
import { RecoveryKeyReveal } from "../../components/RecoveryKeyReveal";
import { setup } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";

/**
 * Server-mode-only first-run setup wizard. Rendered by AuthGate when
 * `GET /api/auth/status` reports `needsSetup: true` (no users exist yet).
 * Two steps: create the admin account, then reveal the recovery key exactly
 * once (via RecoveryKeyReveal) before handing off to the app.
 */
export default function SetupScreen({ onComplete }: { onComplete: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!username.trim() || !password) {
      setError("Enter a username and password.");
      return;
    }
    if (password !== confirmPassword) {
      setError("Passwords don't match.");
      return;
    }

    setSubmitting(true);
    try {
      const result = await setup(username.trim(), password);
      setRecoveryKey(result.recoveryKey);
    } catch (err) {
      const code = (err as { code?: string } | null)?.code;
      if (code === "auth.already_setup") {
        setError("Setup has already been completed on this server. Try signing in instead.");
      } else {
        const message = userErrorMessage(err, "Could not complete setup.");
        setError(message);
        toast.error("Setup failed", { description: message });
      }
    } finally {
      setSubmitting(false);
    }
  };

  if (recoveryKey) {
    return (
      <div className="screen server-auth-screen">
        <RecoveryKeyReveal recoveryKey={recoveryKey} onContinue={onComplete} />
      </div>
    );
  }

  return (
    <div className="screen server-auth-screen">
      <form className="card server-auth-card" onSubmit={(e) => void handleSubmit(e)}>
        <p className="eyebrow">Welcome to FinSight</p>
        <h1 className="h1" style={{ fontSize: 26 }}>Create your account</h1>
        <p className="muted" style={{ marginTop: 8 }}>
          This is the first run on this server — create an admin account to secure your data.
        </p>

        <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 12 }}>
          <Input
            label="Username"
            id="setup-username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            autoFocus
          />
          <Input
            label="Password"
            id="setup-password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
          />
          <Input
            label="Confirm password"
            id="setup-confirm-password"
            type="password"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            autoComplete="new-password"
          />
        </div>

        {error && (
          <p role="alert" className="err" style={{ marginTop: 12 }}>
            {error}
          </p>
        )}

        <Button type="submit" variant="primary" style={{ marginTop: 18, width: "100%" }} disabled={submitting}>
          {submitting ? "Creating…" : "Create account"}
        </Button>
      </form>
    </div>
  );
}

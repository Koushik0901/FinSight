import { useState, type FormEvent } from "react";
import { toast } from "sonner";
import { setup } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";
import { AuthShell, Field, Ico, PasswordStrength, RecoveryReveal } from "./authScene";

const MIN_PASSWORD_LEN = 10;

/**
 * Server-mode-only first-run setup. Rendered by AuthGate when
 * `GET /api/auth/status` reports `needsSetup: true` (no users exist yet).
 * Two steps: create the admin account, then reveal the recovery key exactly
 * once before handing off to the app. Presented in the shared {@link AuthShell}.
 */
export default function SetupScreen({ onComplete }: { onComplete: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPw, setShowPw] = useState(false);
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
    if (password.length < MIN_PASSWORD_LEN) {
      setError(`Password must be at least ${MIN_PASSWORD_LEN} characters.`);
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
      <AuthShell
        eyebrow="Save your recovery key"
        title={<>This is shown <em>once</em>.</>}
        subtitle="If you ever forget your password, this recovery key is the only way back into your data. Store it somewhere safe — a password manager or a printed copy. FinSight never keeps a copy."
      >
        <RecoveryReveal recoveryKey={recoveryKey} onContinue={onComplete} />
      </AuthShell>
    );
  }

  return (
    <AuthShell
      eyebrow="Create your account"
      title={<>Money, made <em>legible</em>.</>}
      subtitle="This is the first run on this server. Create the admin account that secures your data — everything is encrypted at rest with a key only you hold."
    >
      <form onSubmit={(e) => void handleSubmit(e)} noValidate>
        <Field
          icon={Ico.user()}
          id="setup-username"
          label="Username"
          value={username}
          onChange={setUsername}
          autoComplete="username"
          required
          autoFocus
        />
        <Field
          icon={Ico.lock()}
          id="setup-password"
          label="Password"
          type={showPw ? "text" : "password"}
          value={password}
          onChange={setPassword}
          autoComplete="new-password"
          required
          trailing={
            <button type="button" className="toggle-eye" onClick={() => setShowPw((s) => !s)} aria-label="Toggle visibility">
              {showPw ? Ico.eyeoff() : Ico.eye()}
            </button>
          }
        />
        <PasswordStrength pw={password} open={password.length > 0} />
        <Field
          icon={Ico.lock()}
          id="setup-confirm-password"
          label="Confirm password"
          type={showPw ? "text" : "password"}
          value={confirmPassword}
          onChange={setConfirmPassword}
          autoComplete="new-password"
          required
        />

        {error && (
          <div className="auth-alert" role="alert">{Ico.warn()} {error}</div>
        )}

        <button className={"submit" + (submitting ? " busy" : "")} type="submit" disabled={submitting}>
          {submitting
            ? <><span className="spinner" /> Creating…</>
            : <>Create account <span className="arw">{Ico.arrow()}</span></>}
        </button>

        <p className="legal">
          A recovery key is shown next — it's the only way back in if you forget your password. FinSight uses local, encrypted storage and never moves your money.
        </p>
      </form>
    </AuthShell>
  );
}

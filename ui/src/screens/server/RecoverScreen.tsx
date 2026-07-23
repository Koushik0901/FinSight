import { useState, type FormEvent } from "react";
import { recoverAccount } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";
import { AuthShell, Field, Ico, RecoveryReveal } from "./authScene";

/**
 * Server-mode-only password recovery. Reached from LoginScreen's "Forgot your
 * password?" link, presented in the shared {@link AuthShell}.
 *
 * `POST /api/auth/recover` exchanges a username + recovery key for a new
 * password. On success the server rotates the recovery key and establishes a
 * session — so the user is already signed in by the time we render the new key.
 * That key is shown exactly once (RecoveryReveal) before handing off to the app.
 *
 * The `auth.bad_recovery_key` message is deliberately generic: it must not
 * reveal whether the username exists or only the key was wrong.
 */
export default function RecoverScreen({
  onComplete,
  onCancel,
}: {
  onComplete: () => void;
  onCancel: () => void;
}) {
  const [username, setUsername] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [newRecoveryKey, setNewRecoveryKey] = useState<string | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!username.trim() || !recoveryKey.trim() || !newPassword) {
      setError("Fill in every field to continue.");
      return;
    }
    if (newPassword !== confirmPassword) {
      setError("Passwords don't match.");
      return;
    }
    setSubmitting(true);
    try {
      const result = await recoverAccount(username.trim(), recoveryKey.trim(), newPassword);
      setNewRecoveryKey(result.recoveryKey);
    } catch (err) {
      const code = (err as { code?: string } | null)?.code;
      if (code === "auth.bad_recovery_key") {
        setError("That username and recovery key don't match.");
      } else if (code === "auth.weak_password") {
        setError("Choose a password with at least 10 characters.");
      } else if (code === "auth.too_many_attempts") {
        setError("Too many attempts. Wait a moment, then try again.");
      } else {
        setError(userErrorMessage(err, "Could not reset your password. Try again."));
      }
    } finally {
      setSubmitting(false);
    }
  };

  if (newRecoveryKey) {
    return (
      <AuthShell
        eyebrow="Save your new recovery key"
        title={<>Your key has <em>rotated</em>.</>}
        subtitle="Your old recovery key no longer works. Store this new one somewhere safe — it's the only way back in next time."
      >
        <RecoveryReveal recoveryKey={newRecoveryKey} onContinue={onComplete} />
      </AuthShell>
    );
  }

  return (
    <AuthShell
      eyebrow="Account recovery"
      title={<>Reset your <em>password</em>.</>}
      subtitle="Enter the recovery key you saved when your account was created. You'll get a fresh key to replace it."
    >
      <form onSubmit={(e) => void handleSubmit(e)} noValidate>
        <Field
          icon={Ico.user()}
          id="recover-username"
          label="Username"
          value={username}
          onChange={setUsername}
          autoComplete="username"
          autoFocus
        />
        <Field
          icon={Ico.key()}
          id="recover-key"
          label="Recovery key"
          value={recoveryKey}
          onChange={setRecoveryKey}
          autoComplete="off"
        />
        <Field
          icon={Ico.lock()}
          id="recover-new-password"
          label="New password"
          type={showPw ? "text" : "password"}
          value={newPassword}
          onChange={setNewPassword}
          autoComplete="new-password"
          trailing={
            <button type="button" className="toggle-eye" onClick={() => setShowPw((s) => !s)} aria-label="Toggle visibility">
              {showPw ? Ico.eyeoff() : Ico.eye()}
            </button>
          }
        />
        <Field
          icon={Ico.lock()}
          id="recover-confirm-password"
          label="Confirm new password"
          type={showPw ? "text" : "password"}
          value={confirmPassword}
          onChange={setConfirmPassword}
          autoComplete="new-password"
        />

        {error && (
          <div className="auth-alert" role="alert">{Ico.warn()} {error}</div>
        )}

        <button className={"submit" + (submitting ? " busy" : "")} type="submit" disabled={submitting}>
          {submitting
            ? <><span className="spinner" /> Resetting…</>
            : <>Reset password <span className="arw">{Ico.arrow()}</span></>}
        </button>

        <div className="form-row" style={{ justifyContent: "center", marginTop: 10 }}>
          <button type="button" onClick={onCancel} disabled={submitting} style={{ color: "var(--ink-mute)" }}>
            Back to sign in
          </button>
        </div>
      </form>
    </AuthShell>
  );
}

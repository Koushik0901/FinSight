import { useState, type FormEvent } from "react";
import Button from "../../components/Button";
import Input from "../../components/Input";
import { RecoveryKeyReveal } from "../../components/RecoveryKeyReveal";
import { recoverAccount } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";

/**
 * Server-mode-only password recovery. Reached from LoginScreen's "Forgot your
 * password?" link.
 *
 * `POST /api/auth/recover` exchanges a username + recovery key for a new
 * password. On success the server rotates the recovery key and establishes a
 * session — so the user is already signed in by the time we render the new
 * key. That key is shown exactly once (RecoveryKeyReveal, same treatment as
 * first-run setup) before handing off to the app.
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
      <div className="screen server-auth-screen">
        <RecoveryKeyReveal recoveryKey={newRecoveryKey} onContinue={onComplete} />
      </div>
    );
  }

  return (
    <div className="screen server-auth-screen">
      <form className="card server-auth-card" onSubmit={(e) => void handleSubmit(e)}>
        <p className="eyebrow">FinSight</p>
        <h1 className="h1" style={{ fontSize: 26 }}>Reset your password</h1>
        <p className="muted" style={{ marginTop: 8 }}>
          Enter the recovery key you saved when your account was created. You&apos;ll get a new
          recovery key to replace it.
        </p>

        <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 12 }}>
          <Input
            label="Username"
            id="recover-username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            autoFocus
          />
          <Input
            label="Recovery key"
            id="recover-key"
            type="text"
            value={recoveryKey}
            onChange={(e) => setRecoveryKey(e.target.value)}
            autoComplete="off"
            spellCheck={false}
            placeholder="aaaaaaaa-bbbbbbbb-…"
          />
          <Input
            label="New password"
            id="recover-new-password"
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            autoComplete="new-password"
            hint="At least 10 characters."
          />
          <Input
            label="Confirm new password"
            id="recover-confirm-password"
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
          {submitting ? "Resetting…" : "Reset password"}
        </Button>

        <Button
          type="button"
          variant="text"
          style={{ marginTop: 10, width: "100%" }}
          onClick={onCancel}
          disabled={submitting}
        >
          Back to sign in
        </Button>
      </form>
    </div>
  );
}

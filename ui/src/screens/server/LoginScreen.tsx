import { useState, type FormEvent } from "react";
import RecoverScreen from "./RecoverScreen";
import { lastAuthedUser, login } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";
import { AuthShell, Field, Ico } from "./authScene";

/**
 * Server-mode-only login screen. Rendered by AuthGate when the app is running
 * against finsight-server and the session cookie is missing or expired (either
 * at boot or after a `finsight:auth-required` event). Presented in the shared
 * {@link AuthShell}.
 */
export default function LoginScreen({ onComplete }: { onComplete: () => void }) {
  // Actual-style identity switching: the last signed-in username is only a
  // display convenience (the password still gates everything), so returning
  // users and shared-device switchers pick an identity instead of retyping.
  const [username, setUsername] = useState(() => lastAuthedUser() ?? "");
  // Whether the field came from the last-session marker (vs typed fresh).
  // Drives the "someone else" affordance and initial focus placement.
  const [prefilled] = useState(() => lastAuthedUser() !== null);
  const [password, setPassword] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [recovering, setRecovering] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await login(username.trim(), password);
      onComplete();
    } catch (err) {
      const code = (err as { code?: string } | null)?.code;
      setError(
        code === "auth.bad_credentials"
          ? "Wrong username or password."
          : userErrorMessage(err, "Could not sign in. Try again."),
      );
    } finally {
      setSubmitting(false);
    }
  };

  // Recovery reuses this screen's mount point (AuthGate renders LoginScreen
  // directly; there's no router at the gate) and shares its onComplete — a
  // successful recovery leaves the user signed in, exactly like a login.
  if (recovering) {
    return <RecoverScreen onComplete={onComplete} onCancel={() => setRecovering(false)} />;
  }

  return (
    <AuthShell
      eyebrow="Welcome back"
      title={<>Pick up where you <em>left off</em>.</>}
      subtitle="Sign in to your dashboard, agent insights and shared budgets."
    >
      <form onSubmit={(e) => void handleSubmit(e)} noValidate>
        <Field
          icon={Ico.user()}
          id="login-username"
          label="Username"
          value={username}
          onChange={setUsername}
          autoComplete="username"
          autoFocus={username === ""}
        />
        <Field
          icon={Ico.lock()}
          id="login-password"
          label="Password"
          type={showPw ? "text" : "password"}
          value={password}
          onChange={setPassword}
          autoComplete="current-password"
          autoFocus={prefilled && username !== ""}
          trailing={
            <button type="button" className="toggle-eye" onClick={() => setShowPw((s) => !s)} aria-label="Toggle visibility">
              {showPw ? Ico.eyeoff() : Ico.eye()}
            </button>
          }
        />

        <div className="form-row">
          <button type="button" onClick={() => setRecovering(true)} disabled={submitting} style={{ color: "var(--ink-mute)" }}>
            Forgot your password?
          </button>
          {prefilled && username !== "" && (
            <button
              type="button"
              onClick={() => setUsername("")}
              disabled={submitting}
              style={{ color: "var(--ink-mute)" }}
            >
              Someone else?
            </button>
          )}
        </div>

        {error && (
          <div className="auth-alert" role="alert">{Ico.warn()} {error}</div>
        )}

        <button className={"submit" + (submitting ? " busy" : "")} type="submit" disabled={submitting}>
          {submitting
            ? <><span className="spinner" /> Signing in…</>
            : <>Sign in <span className="arw">{Ico.arrow()}</span></>}
        </button>
      </form>
    </AuthShell>
  );
}

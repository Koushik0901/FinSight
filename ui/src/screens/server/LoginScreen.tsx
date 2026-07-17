import { useState, type FormEvent } from "react";
import Button from "../../components/Button";
import Input from "../../components/Input";
import { login } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";

/**
 * Server-mode-only login screen. Rendered by AuthGate when the app is
 * running against finsight-server and the session cookie is missing or
 * expired (either at boot or after a `finsight:auth-required` event).
 */
export default function LoginScreen({ onComplete }: { onComplete: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

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
          : userErrorMessage(err, "Could not sign in. Try again.")
      );
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="screen server-auth-screen">
      <form className="card server-auth-card" onSubmit={(e) => void handleSubmit(e)}>
        <p className="eyebrow">FinSight</p>
        <h1 className="h1" style={{ fontSize: 26 }}>Sign in</h1>
        <p className="muted" style={{ marginTop: 8 }}>Enter your username and password to continue.</p>

        <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 12 }}>
          <Input
            label="Username"
            id="login-username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            autoFocus
          />
          <Input
            label="Password"
            id="login-password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
          />
        </div>

        {error && (
          <p role="alert" className="err" style={{ marginTop: 12 }}>
            {error}
          </p>
        )}

        <Button type="submit" variant="primary" style={{ marginTop: 18, width: "100%" }} disabled={submitting}>
          {submitting ? "Signing in…" : "Sign in"}
        </Button>
      </form>
    </div>
  );
}

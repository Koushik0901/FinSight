import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import Button from "../../components/Button";
import { approveOAuth, fetchOAuthClientName, oauthErrorMessage } from "../../api/oauth";
import type { TokenScope } from "../../api/tokens";

/**
 * OAuth consent screen — the one interactive step when a cloud connector
 * (claude.ai, ChatGPT) links itself to this server. The client sends the
 * browser here with the authorization-request parameters; we identify the app,
 * let the user choose an access level, and hand the browser onward to the
 * client's callback with a single-use code.
 *
 * Reached through the SPA fallback, so `AuthGate` has already required a login
 * by the time this renders — no auth handling is needed here.
 */
export default function OAuthAuthorize() {
  const [params] = useSearchParams();
  const clientId = params.get("client_id");
  const redirectUri = params.get("redirect_uri");
  const state = params.get("state");
  const codeChallenge = params.get("code_challenge");
  const codeChallengeMethod = params.get("code_challenge_method") ?? "S256";
  const requestedScope = params.get("scope");

  const [clientName, setClientName] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScope] = useState<TokenScope>(requestedScope === "full" ? "full" : "read");
  const [submitting, setSubmitting] = useState(false);

  // Validated before any network call: a request missing these can never be
  // completed, and — critically — must NOT be redirected anywhere, since an
  // unvalidated redirect_uri is exactly what an attacker would supply.
  const paramsValid =
    Boolean(clientId) &&
    Boolean(redirectUri) &&
    Boolean(codeChallenge) &&
    codeChallengeMethod === "S256";

  useEffect(() => {
    if (!paramsValid || !clientId) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    fetchOAuthClientName(clientId)
      .then((name) => {
        if (!cancelled) setClientName(name);
      })
      .catch((err) => {
        if (!cancelled) setError(oauthErrorMessage(err, "This app is not registered with your server."));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [clientId, paramsValid]);

  const handleApprove = async () => {
    if (!clientId || !redirectUri || !codeChallenge) return;
    setSubmitting(true);
    setError(null);
    try {
      const redirectTo = await approveOAuth({
        clientId,
        redirectUri,
        scope,
        state,
        codeChallenge,
        codeChallengeMethod,
      });
      window.location.assign(redirectTo);
    } catch (err) {
      setError(oauthErrorMessage(err, "Could not authorize this app."));
      setSubmitting(false);
    }
  };

  const handleDeny = () => {
    if (!redirectUri) return;
    // Only reachable once redirect_uri passed validation above.
    const sep = redirectUri.includes("?") ? "&" : "?";
    const stateParam = state ? `&state=${encodeURIComponent(state)}` : "";
    window.location.assign(`${redirectUri}${sep}error=access_denied${stateParam}`);
  };

  if (!paramsValid) {
    return (
      <div className="screen oauth-authorize-screen">
        <div className="card">
          <p className="eyebrow">Authorization request</p>
          <h1 className="h1" style={{ fontSize: 22 }}>This request isn&apos;t valid</h1>
          <p className="muted" style={{ marginTop: 8 }}>
            It&apos;s missing information FinSight needs to identify the app safely, so nothing was
            authorized. Start the connection again from the app you were setting up.
          </p>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="screen oauth-authorize-screen">
        <div className="card">
          <p className="muted">Checking the request…</p>
        </div>
      </div>
    );
  }

  if (error && !clientName) {
    return (
      <div className="screen oauth-authorize-screen">
        <div className="card">
          <p className="eyebrow">Authorization request</p>
          <h1 className="h1" style={{ fontSize: 22 }}>Couldn&apos;t verify this app</h1>
          <p role="alert" className="err" style={{ marginTop: 12 }}>
            {error}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="screen oauth-authorize-screen">
      <div className="card">
        <p className="eyebrow">Authorization request</p>
        <h1 className="h1" style={{ fontSize: 22 }}>
          Connect {clientName} to FinSight?
        </h1>
        <p className="muted" style={{ marginTop: 8, lineHeight: 1.55 }}>
          <strong>{clientName}</strong> is asking to read your financial data through this server. It will be
          able to see your accounts, transactions, budgets, and goals.
        </p>

        <div style={{ marginTop: 20 }}>
          <div className="label" id="oauth-scope-label">
            Access level
          </div>
          <div className="toolbar" style={{ marginTop: 8 }} role="group" aria-labelledby="oauth-scope-label">
            <button
              type="button"
              className={scope === "read" ? "on" : ""}
              aria-pressed={scope === "read"}
              onClick={() => setScope("read")}
            >
              Read only
            </button>
            <button
              type="button"
              className={scope === "full" ? "on" : ""}
              aria-pressed={scope === "full"}
              onClick={() => setScope("full")}
            >
              Read and write
            </button>
          </div>
          <p className="desc" style={{ marginTop: 8 }}>
            {scope === "read"
              ? "It can analyse your finances but cannot change anything."
              : "It can also propose changes and apply them once you agree in the conversation. Every proposal is recorded in FinSight, where you can review what was done."}
          </p>
        </div>

        {error && (
          <p role="alert" className="err" style={{ marginTop: 16 }}>
            {error}
          </p>
        )}

        <div className="toolbar" style={{ marginTop: 24 }}>
          <Button type="button" variant="primary" disabled={submitting} onClick={() => void handleApprove()}>
            {submitting ? "Connecting…" : `Allow ${clientName}`}
          </Button>
          <Button type="button" variant="outline" disabled={submitting} onClick={handleDeny}>
            Deny
          </Button>
        </div>

        <p className="muted" style={{ marginTop: 16, fontSize: 12.5 }}>
          You can revoke this at any time in Settings → Connections.
        </p>
      </div>
    </div>
  );
}

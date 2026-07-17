import { useEffect, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Toaster } from "sonner";
import { fetchAuthStatus, isServerMode } from "../api/auth";
import { purgePersistedCache } from "../pwa/persist";
import SetupScreen from "../screens/server/SetupScreen";
import LoginScreen from "../screens/server/LoginScreen";

type GateState =
  | { kind: "ready" }
  | { kind: "checking" }
  | { kind: "error" }
  | { kind: "needsSetup" }
  | { kind: "needsLogin" };

/**
 * Server-mode-only boot gate. Wraps the normal app root (BrowserRouter/App)
 * in main.tsx.
 *
 * Desktop/Tauri builds: `isServerMode()` is false (the httpBackend shim that
 * sets `window.__FINSIGHT_HTTP__` is never installed there), so this renders
 * `children` synchronously with no effects, no fetches, and no listeners —
 * completely inert. Zero behavior change for the desktop app.
 *
 * Server mode: resolves `/api/auth/status` once at boot and swaps in the
 * Setup or Login screen as needed. A `finsight:auth-required` event —
 * dispatched by the httpBackend shim on any RPC 401 `auth.required`, and by
 * an explicit logout (Settings) — routes back to the login screen from
 * anywhere in the app (session expired mid-use). That same path clears the
 * in-memory tanstack-query cache AND purges the IndexedDB-persisted copy
 * (`pwa/persist.ts`) so a shared device never leaks a prior user's cached
 * financials. On successful setup/login, the in-memory cache is cleared
 * again before handing off to `children` so the newly-authenticated user
 * never sees a stale/previous user's cached data.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const serverMode = isServerMode();
  const queryClient = useQueryClient();
  const [state, setState] = useState<GateState>(serverMode ? { kind: "checking" } : { kind: "ready" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    if (!serverMode) return;
    let cancelled = false;
    setState({ kind: "checking" });
    fetchAuthStatus()
      .then((status) => {
        if (cancelled) return;
        if (status.needsSetup) setState({ kind: "needsSetup" });
        else if (!status.authenticated) setState({ kind: "needsLogin" });
        else setState({ kind: "ready" });
      })
      .catch(() => {
        if (!cancelled) setState({ kind: "error" });
      });
    return () => {
      cancelled = true;
    };
    // `attempt` is retry-only churn (Retry button); `serverMode` is stable
    // for the life of the app (set once at boot by installHttpBackend).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [serverMode, attempt]);

  useEffect(() => {
    if (!serverMode) return;
    const onAuthRequired = () => {
      // Session ended (explicit logout or a 401 mid-use) — the persisted
      // IndexedDB cache must not outlive the session on a shared device.
      queryClient.clear();
      // Failure here (quota/blocked-DB in private browsing) must not be silent:
      // a swallowed rejection would leave stale financial data in IndexedDB
      // past the session that's supposed to purge it.
      purgePersistedCache().catch((err) => console.error("purgePersistedCache failed", err));
      setState({ kind: "needsLogin" });
    };
    window.addEventListener("finsight:auth-required", onAuthRequired);
    return () => window.removeEventListener("finsight:auth-required", onAuthRequired);
  }, [serverMode, queryClient]);

  if (!serverMode || state.kind === "ready") return <>{children}</>;

  const handleAuthenticated = () => {
    queryClient.clear();
    setState({ kind: "ready" });
  };

  return (
    <>
      {state.kind === "checking" && (
        <div className="stub server-auth-loading" role="status" aria-label="Loading">
          <span className="spinner" aria-hidden="true" />
        </div>
      )}
      {state.kind === "error" && (
        <div className="screen server-auth-screen">
          <div className="card server-auth-card">
            <p className="eyebrow">Connection problem</p>
            <h1 className="h1" style={{ fontSize: 22 }}>Can&apos;t reach the FinSight server.</h1>
            <p className="muted" style={{ marginTop: 8 }}>
              Check that the server is running, then try again.
            </p>
            <button
              type="button"
              className="btn primary"
              style={{ marginTop: 16 }}
              onClick={() => setAttempt((n) => n + 1)}
            >
              Retry
            </button>
          </div>
        </div>
      )}
      {state.kind === "needsSetup" && <SetupScreen onComplete={handleAuthenticated} />}
      {state.kind === "needsLogin" && <LoginScreen onComplete={handleAuthenticated} />}
      <Toaster richColors position="bottom-right" />
    </>
  );
}

export default AuthGate;

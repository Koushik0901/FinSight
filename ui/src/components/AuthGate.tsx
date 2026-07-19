import { useEffect, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Toaster } from "sonner";
import {
  clearSessionMarker,
  fetchAuthStatus,
  hadPriorSession,
  isAuthFailure,
  isNetworkFailure,
  isServerMode,
  lastAuthedUser,
  markSessionEstablished,
} from "../api/auth";
import { purgePersistedCache } from "../pwa/persist";
import { clearAppBadge } from "../pwa/badge";
import { purgeSharedFiles } from "../pwa/shareTarget";
import SetupScreen from "../screens/server/SetupScreen";
import LoginScreen from "../screens/server/LoginScreen";

type GateState =
  | { kind: "ready" }
  | { kind: "checking" }
  | { kind: "error" }
  | { kind: "offline" }
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
 *
 * OFFLINE BOOT: when the status probe rejects, the gate distinguishes two
 * failures. An invalid-session verdict (`auth.required`) always routes to the
 * login screen — authentication is never weakened. A NETWORK
 * failure (fetch never got a response) falls back to `children` in an
 * `offline` state *only if* this device has a prior-session marker
 * (`api/auth.ts`), so the 7-day IndexedDB-persisted query cache and the
 * `OfflineBanner` — both nested inside `children` in main.tsx — actually
 * render. Without a marker it's still the connection-problem wall.
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
        // The server answered — its verdict is authoritative, so keep the
        // offline marker in sync with it in both directions.
        if (status.needsSetup) {
          clearSessionMarker();
          setState({ kind: "needsSetup" });
        } else if (!status.authenticated) {
          clearSessionMarker();
          setState({ kind: "needsLogin" });
        } else {
          markSessionEstablished(status.username);
          setState({ kind: "ready" });
        }
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        // Order matters: an auth verdict wins over any offline fallback.
        if (isAuthFailure(err)) {
          clearSessionMarker();
          setState({ kind: "needsLogin" });
        } else if (isNetworkFailure(err) && hadPriorSession()) {
          setState({ kind: "offline" });
        } else {
          setState({ kind: "error" });
        }
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
      // IndexedDB cache must not outlive the session on a shared device, and
      // the offline marker must not resurrect it on the next boot.
      clearSessionMarker();
      queryClient.clear();
      // Failure here (quota/blocked-DB in private browsing) must not be silent:
      // a swallowed rejection would leave stale financial data in IndexedDB
      // past the session that's supposed to purge it.
      purgePersistedCache().catch((err) => console.error("purgePersistedCache failed", err));
      // Same reasoning one level out: an icon badge left showing "6 items" after
      // sign-out advertises the previous user's activity on a shared device.
      // App's unmount cleanup also clears it — this is the explicit path, not a
      // duplicate of it, because logout must not depend on unmount ordering.
      void clearAppBadge();
      // A CSV parked by the OS share sheet is raw financial data and is NOT
      // part of the query cache, so purgePersistedCache does not reach it.
      void purgeSharedFiles();
      setState({ kind: "needsLogin" });
    };
    window.addEventListener("finsight:auth-required", onAuthRequired);
    return () => window.removeEventListener("finsight:auth-required", onAuthRequired);
  }, [serverMode, queryClient]);

  if (!serverMode || state.kind === "ready") return <>{children}</>;

  if (state.kind === "offline") {
    return (
      <>
        <div
          className="card offline-banner"
          role="status"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: "var(--space-3)",
            borderRadius: 0,
            padding: "var(--space-2) var(--space-4)",
            background: "var(--surface-2)",
            color: "var(--ink-mute)",
          }}
        >
          <span>
            Can&apos;t reach the FinSight server — showing the last data synced
            {lastAuthedUser() ? ` for ${lastAuthedUser()}` : ""}. Sign-in and changes resume when the
            connection returns.
          </span>
          <button type="button" className="btn sm" onClick={() => setAttempt((n) => n + 1)}>
            Retry
          </button>
        </div>
        {children}
      </>
    );
  }

  // Symmetric with the logout/401 path above: clear the in-memory cache AND
  // purge the IndexedDB copy before handing off. Without the purge, a pending
  // persister restore of the PREVIOUS user's cache can settle after the new
  // user is already in — on a shared device that leaks A's balances into B's
  // session. Render only once the purge has settled; `.finally` so a purge
  // failure still lets the app through rather than trapping the user.
  const handleAuthenticated = () => {
    queryClient.clear();
    // Same shared-device reasoning as the cache purge below, applied to a file
    // parked by the share sheet: someone can share a statement, abandon the
    // login screen, and a DIFFERENT person can then sign in. Without this, that
    // second person's session would silently import the first person's bank
    // statement. ShareTargetImport then reports "no longer available", which is
    // exactly right — re-sharing while signed in works normally.
    void purgeSharedFiles();
    purgePersistedCache()
      .catch((err) => console.error("purgePersistedCache failed", err))
      .finally(() => setState({ kind: "ready" }));
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

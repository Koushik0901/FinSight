import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PersistQueryClientProvider } from "@tanstack/react-query-persist-client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { AuthGate } from "./components/AuthGate";
import DesktopConnectGate from "./components/DesktopConnectGate";
import VersionBanner from "./components/VersionBanner";
import OfflineBanner from "./components/OfflineBanner";
import { createIdbPersister } from "./pwa/persist";
import { sweepStaleSharedFiles } from "./pwa/shareTarget";
import { isServerMode } from "./api/auth";
import { selectBackend } from "./api/selectBackend";
import { instrumentQueryCache } from "./utils/perf";
import "./styles/reset.css";
import "./styles/tokens.css";
import "./styles/app.css";
import "./styles/onboarding.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 5_000,
      // FinSight is a local-first desktop app: the SQLite ledger only changes
      // via in-app actions (mutations, imports, sync), and each of those
      // already invalidates precisely. Refetching every active query whenever
      // the window regains focus would just replay that whole query set as an
      // IPC + SQL storm for no new data — so turn it off.
      refetchOnWindowFocus: false,
    },
  },
});

// Opt-in perf instrumentation (localStorage.finsightPerf="1" or ?perf=1) for
// real-desktop before/after measurement. Zero overhead when off.
instrumentQueryCache(queryClient.getQueryCache());

// IndexedDB-backed persister for the query cache — server/PWA mode only (see
// pwa/persist.ts). Constructed unconditionally (cheap, no I/O until used) so
// renderApp() can pick the provider without re-creating it per render.
const persister = createIdbPersister();

function renderApp() {
  const tree = (
    <AuthGate>
      <VersionBanner />
      <OfflineBanner />
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </AuthGate>
  );

  createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <DesktopConnectGate>
        {isServerMode() ? (
          <PersistQueryClientProvider
            client={queryClient}
            persistOptions={{ persister, maxAge: 1000 * 60 * 60 * 24 * 7 }}
          >
            {tree}
          </PersistQueryClientProvider>
        ) : (
          <QueryClientProvider client={queryClient}>{tree}</QueryClientProvider>
        )}
      </DesktopConnectGate>
    </React.StrictMode>
  );
}

// Transport selection lives in selectBackend() (api/selectBackend.ts) so the
// bridge/origin decision is unit-testable. In short:
// - DEV `?mock=…` → fixture backend (plain-browser design harness);
// - not a real desktop-IPC context (isTauriRuntime(), origin-aware) → the
//   production HTTP/SSE shim — this is the transport for the browser, the PWA,
//   AND the thin desktop shell once it has navigated to a remote server (the
//   Tauri bridge persists at the remote origin, so we must gate on the origin-
//   aware check, not raw bridge presence);
// - otherwise → leave the native Tauri bridge in place (shell pre-navigation).
async function boot() {
  if (typeof window !== "undefined") {
    const params = new URLSearchParams(window.location.search);
    const backend = selectBackend(params);
    if (backend === "mock") {
      const { installMockBackend } = await import("./dev/mockBackend");
      installMockBackend(params.get("mock"));
    } else if (backend === "http") {
      const { installHttpBackend } = await import("./api/httpBackend");
      installHttpBackend();
    }

    // Discard a stale CSV parked by the OS share sheet. Deliberately here and
    // not inside AuthGate: a share received while signed out never reaches the
    // app tree at all (AuthGate renders the login screen instead of children),
    // so this is the only place guaranteed to run and clean it up. Fire-and-
    // forget — nothing about rendering depends on it.
    void sweepStaleSharedFiles();
  }
  renderApp();
}

void boot();

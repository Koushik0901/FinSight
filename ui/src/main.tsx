import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClient } from "@tanstack/react-query";
import { PersistQueryClientProvider } from "@tanstack/react-query-persist-client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { AuthGate } from "./components/AuthGate";
import VersionBanner from "./components/VersionBanner";
import OfflineBanner from "./components/OfflineBanner";
import { createIdbPersister } from "./pwa/persist";
import { sweepStaleSharedFiles } from "./pwa/shareTarget";
import { instrumentQueryCache } from "./utils/perf";
import "@fontsource-variable/geist/wght.css";
import "@fontsource-variable/geist-mono/wght.css";
import "./styles/reset.css";
import "./styles/tokens.css";
import "./styles/app.css";
import "./styles/copilot-shell.css";
import "./styles/onboarding.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 60_000,
      refetchOnWindowFocus: false,
    },
  },
});

instrumentQueryCache(queryClient.getQueryCache());

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
      <PersistQueryClientProvider
        client={queryClient}
        persistOptions={{ persister, maxAge: 1000 * 60 * 60 * 24 * 7 }}
      >
        {tree}
      </PersistQueryClientProvider>
    </React.StrictMode>,
  );
}

// Pure self-hosted boot: no Tauri shell.
// Browser, PWA, and any future split `web` container all use the HTTP/SSE
// shim. The `?mock` harness remains for design work.
async function boot() {
  if (typeof window !== "undefined") {
    const params = new URLSearchParams(window.location.search);
    const hasMock = import.meta.env.DEV && params.has("mock");
    if (hasMock) {
      const { installMockBackend } = await import("./dev/mockBackend");
      installMockBackend(params.get("mock"));
    }
    void sweepStaleSharedFiles();
  }
  renderApp();
}

void boot();

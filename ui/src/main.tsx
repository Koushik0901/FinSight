import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
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

function renderApp() {
  createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </QueryClientProvider>
    </React.StrictMode>
  );
}

// DEV-ONLY design harness: `?mock=rich|empty|partial|large|multi` installs a
// fixture-backed __TAURI_INTERNALS__ so the app renders full data in a plain
// browser (no Tauri). Tree-shaken from production (import.meta.env.DEV) and
// never touches a real desktop runtime or the vitest suite. See dev/mockBackend.
async function boot() {
  if (import.meta.env.DEV && typeof window !== "undefined") {
    const params = new URLSearchParams(window.location.search);
    const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
    if (params.has("mock") && !w.__TAURI_INTERNALS__) {
      const { installMockBackend } = await import("./dev/mockBackend");
      installMockBackend(params.get("mock"));
    }
  }
  renderApp();
}

void boot();

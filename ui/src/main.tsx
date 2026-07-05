import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
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

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </QueryClientProvider>
  </React.StrictMode>
);

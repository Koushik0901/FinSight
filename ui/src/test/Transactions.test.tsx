import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Transactions from "../screens/Transactions";
import type { ReactNode } from "react";

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={qc}>
      <BrowserRouter>{node}</BrowserRouter>
    </QueryClientProvider>
  );
}

describe("Transactions", () => {
  it("lists transactions with merchant and amount", async () => {
    vi.mocked(invoke).mockResolvedValue([
      {
        id: "t1",
        account_id: "a1",
        posted_at: "2026-05-20T10:00:00Z",
        amount_cents: -1599,
        merchant_raw: "Netflix",
        merchant_id: "m_netflix",
        merchant_label: "Netflix",
        merchant_color: "#F472B6",
        merchant_initials: "NF",
        category_id: "subs",
        category_label: "Subscriptions",
        category_color: "#F472B6",
        status: "cleared",
        notes: null,
        ai_confidence: null,
        ai_explanation: null,
        is_anomaly: false,
        created_at: "2026-05-20T10:00:00Z",
      },
    ]);

    render(wrap(<Transactions />));
    await waitFor(() => {
      expect(screen.getByText("Netflix")).toBeInTheDocument();
      expect(screen.getByText(/-\$15\.99/)).toBeInTheDocument();
    });
  });
});

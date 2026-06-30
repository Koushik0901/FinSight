import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import type { ReactNode } from "react";
import Transactions from "../screens/Transactions";

vi.mock("../api/client", () => ({
  commands: {
    listTransactions: vi.fn().mockResolvedValue({
      status: "ok",
      data: [
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
          is_reimbursable: false,
          is_split: false,
          imported_id: null,
          source: null,
          raw_synced_data: null,
          pending: false,
          external_tx_id: null,
          external_account_id: null,
        },
      ],
    }),
    listAccounts: vi.fn().mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "a1",
          owner: "Me",
          bank: "Bank",
          type: "Checking",
          name: "ACT-db288194-14e1-4b85-b63c-68f109943901",
          balance_cents: 1000,
          currency: "USD",
          color: "#3b82f6",
          source: "simplefin",
          liquidity_type: "liquid",
          emergency_fund_eligible: true,
          goal_earmark: null,
          apy_pct: null,
          simplefin_account_id: "db288194-14e1-4b85-b63c-68f109943901",
          last_synced_at: null,
          nickname: "Daily Checking",
          connection_id: null,
          institution_id: null,
          external_account_id: null,
          official_name: null,
          mask: null,
          subtype: null,
          account_group: "cash",
          available_balance_cents: null,
          balance_date: null,
          extra_json: null,
          raw_json: null,
          import_pending: false,
        },
      ],
    }),
    listCategoriesWithSpending: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    getNeedsReviewCount: vi.fn().mockResolvedValue({ status: "ok", data: 0 }),
    getAgentStatus: vi.fn().mockResolvedValue({
      status: "ok",
      data: {
        uncategorizedCount: 0,
        anomalyCount: 0,
        overBudgetCount: 0,
        upcomingBillsCount: 0,
        lastScanAt: null,
        lastScanCategorized: null,
      },
    }),
    exportTransactionsCsv: vi.fn().mockResolvedValue({ status: "ok", data: "" }),
  },
  ResultStatus: { Ok: "ok", Error: "error" },
}));

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
    render(wrap(<Transactions />));
    await waitFor(() => {
      expect(screen.getByText("Netflix")).toBeInTheDocument();
      expect(screen.getByText(/-\$15\.99/)).toBeInTheDocument();
    });
  });

  it("renders account nickname instead of raw SimpleFin account id", async () => {
    render(wrap(<Transactions />));
    await waitFor(() => {
      expect(screen.getByText("Daily Checking")).toBeInTheDocument();
    });
    expect(screen.queryByText("ACT-db288194-14e1-4b85-b63c-68f109943901")).not.toBeInTheDocument();
  });
});

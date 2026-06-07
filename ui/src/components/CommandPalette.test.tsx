import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { CommandPalette } from "./CommandPalette";
import type { ReactNode } from "react";

vi.mock("../api/hooks/networth", () => ({
  useNetWorth: () => 5000000,
}));

vi.mock("../api/client", () => ({
  commands: {
    getMonthTotals: vi.fn().mockResolvedValue({
      status: "ok",
      data: { incomeCents: 600000, expenseCents: 400000, netCents: 200000, savingsRatePct: 33, txnCount: 20 },
    }),
    listCategoriesWithSpending: vi.fn().mockResolvedValue({
      status: "ok",
      data: [
        { id: "c1", label: "Groceries", color: "#4ade80", groupId: "g1", groupLabel: "Food",
          thisMonthCents: 30000, lastMonthCents: 20000, txnCount: 5, yearTotalCents: 300000, budgetCents: 40000 },
      ],
    }),
  },
}));

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("CommandPalette — Ask the agent mode", () => {
  it("shows 'Ask the agent' section once data is loaded", async () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    await waitFor(() => {
      expect(screen.getByText("Ask the agent")).toBeInTheDocument();
    });
  });

  it("switches to answer mode when a question is clicked", async () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    await waitFor(() => screen.getByText("Ask the agent"));
    fireEvent.click(screen.getByText(/What's my top spending category/i));
    await waitFor(() => {
      expect(screen.getByText(/← Back/)).toBeInTheDocument();
    });
  });

  it("returns to list mode when Back is clicked", async () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    await waitFor(() => screen.getByText("Ask the agent"));
    fireEvent.click(screen.getByText(/What's my top spending category/i));
    await waitFor(() => screen.getByText(/← Back/));
    fireEvent.click(screen.getByText(/← Back/));
    await waitFor(() => {
      expect(screen.getByText("Ask the agent")).toBeInTheDocument();
    });
  });

  it("shows 'Run a what-if scenario' action", () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    expect(screen.getByText("Run a what-if scenario")).toBeInTheDocument();
  });

  it("Escape in answer mode returns to list without closing", async () => {
    const onClose = vi.fn();
    render(wrap(<CommandPalette open={true} onClose={onClose} />));
    await waitFor(() => screen.getByText("Ask the agent"));
    fireEvent.click(screen.getByText(/What's my top spending category/i));
    await waitFor(() => screen.getByText(/← Back/));
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.getByText("Ask the agent")).toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
    });
  });
});

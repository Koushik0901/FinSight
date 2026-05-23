import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import TransactionDrawer from "../components/TransactionDrawer";

vi.mock("react-focus-lock", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

vi.mock("../api/client", () => ({
  commands: {
    listAccounts: vi.fn().mockResolvedValue({
      status: "ok",
      data: [{ id: "a1", bank: "Chase", name: "Joint Checking", type: "Checking",
               owner: "joint", currency: "USD", color: "#000", balance_cents: 0, source: "manual" }],
    }),
    createTransaction: vi.fn().mockResolvedValue({ status: "ok", data: { id: "t1" } }),
    listTransactions: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

function renderDrawer() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <TransactionDrawer open onClose={() => {}} />
    </QueryClientProvider>
  );
}

describe("TransactionDrawer", () => {
  beforeEach(() => vi.clearAllMocks());

  it("submits outflow as negative cents", async () => {
    renderDrawer();
    await waitFor(() => expect(screen.getByText(/Chase · Joint Checking/)).toBeInTheDocument());
    fireEvent.change(screen.getByLabelText(/Account/i), { target: { value: "a1" } });
    fireEvent.change(screen.getByLabelText(/Amount/i), { target: { value: "8.42" } });
    fireEvent.change(screen.getByLabelText(/Merchant/i), { target: { value: "Safeway" } });
    fireEvent.click(screen.getByRole("button", { name: /save transaction/i }));
    const { commands } = await import("../api/client");
    await waitFor(() => {
      expect(commands.createTransaction).toHaveBeenCalledWith(expect.objectContaining({
        account_id: "a1",
        amount_cents: -842,
        merchant_raw: "Safeway",
        status: "manual",
      }));
    });
  });
});

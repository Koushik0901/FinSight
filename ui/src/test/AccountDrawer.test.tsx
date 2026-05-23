import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import AccountDrawer from "../components/AccountDrawer";

vi.mock("react-focus-lock", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

vi.mock("../api/client", () => ({
  commands: {
    createAccount: vi.fn().mockResolvedValue({ status: "ok", data: { id: "a1" } }),
    listAccounts: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

function renderDrawer(props: { open?: boolean; onClose?: () => void; onCreated?: () => void } = {}) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <AccountDrawer open={props.open ?? true} onClose={props.onClose ?? (() => {})} onCreated={props.onCreated} />
    </QueryClientProvider>
  );
}

describe("AccountDrawer", () => {
  beforeEach(() => vi.clearAllMocks());

  it("blocks submission when bank/name empty", async () => {
    renderDrawer();
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));
    const { commands } = await import("../api/client");
    await waitFor(() => {
      expect(commands.createAccount).not.toHaveBeenCalled();
    });
  });

  it("submits with cents conversion", async () => {
    renderDrawer();
    fireEvent.change(screen.getByLabelText(/Bank/i), { target: { value: "Chase" } });
    fireEvent.change(screen.getByLabelText(/Name/i), { target: { value: "Joint Checking" } });
    fireEvent.change(screen.getByLabelText(/Opening balance/i), { target: { value: "100.50" } });
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));
    const { commands } = await import("../api/client");
    await waitFor(() => {
      expect(commands.createAccount).toHaveBeenCalledWith(expect.objectContaining({
        bank: "Chase",
        name: "Joint Checking",
        opening_balance_cents: 10050,
        source: "manual",
      }));
    });
  });
});

import type { ReactNode } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { axe } from "vitest-axe";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import Drawer from "../components/Drawer";
import AccountDrawer from "../components/AccountDrawer";
import TransactionDrawer from "../components/TransactionDrawer";
import Onboarding from "../screens/Onboarding";

vi.mock("react-focus-lock", () => ({
  default: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("../api/client", () => ({
  commands: {
    listAccounts: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    listTransactions: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    createAccount: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    createTransaction: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    getOnboardingState: vi.fn().mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: false },
    }),
    seedSampleHousehold: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    markOnboardingComplete: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("a11y sweep", () => {
  it("Drawer has no axe violations", async () => {
    wrap(<Drawer open onClose={() => {}} title="Test drawer"><p>body</p></Drawer>);
    // Drawer portals to document.body — scan the whole body
    const results = await axe(document.body);
    expect(results.violations).toEqual([]);
  });

  it("AccountDrawer has no axe violations", async () => {
    wrap(<AccountDrawer open onClose={() => {}} />);
    const results = await axe(document.body);
    expect(results.violations).toEqual([]);
  });

  it("TransactionDrawer has no axe violations", async () => {
    wrap(<TransactionDrawer open onClose={() => {}} />);
    // Wait for the accounts query to settle before scanning
    await waitFor(() => {});
    const results = await axe(document.body);
    expect(results.violations).toEqual([]);
  });

  it("Onboarding shell (welcome step) has no axe violations", async () => {
    wrap(<Onboarding />);
    await waitFor(() => {});
    const results = await axe(document.body);
    expect(results.violations).toEqual([]);
  });
});

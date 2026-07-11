import type { ReactNode } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { axe } from "vitest-axe";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import Drawer from "../components/Drawer";
import AccountDrawer from "../components/AccountDrawer";
import TransactionDrawer from "../components/TransactionDrawer";
import CategoryPicker from "../components/CategoryPicker";
import AgentActivityFeed from "../components/AgentActivityFeed";
import Onboarding from "../screens/Onboarding";

vi.mock("react-focus-lock", () => ({
  default: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

// Hook-level mocks so components don't call unmocked Tauri commands
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));

vi.mock("../api/hooks/transactions", () => ({
  useTransactions: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useCreateTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useUpdateTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({ transaction: {}, proposed_rule: null }) })),
  useDeleteTransaction: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useCreateRule: vi.fn(() => ({ mutate: vi.fn() })),
  useSetTransactionFlags: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSetAnomalyDismissed: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSetTransactionOwner: vi.fn(() => ({ mutate: vi.fn() })),
  useTransactionSplits: vi.fn(() => ({ data: [] })),
  useSetTransactionSplits: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useCategories: vi.fn(() => ({
    data: [
      { id: "cat-1", label: "Groceries", color: "#4ade80", group_id: "g1", group_label: "Food" },
      { id: "cat-2", label: "Rent", color: "#60a5fa", group_id: "g2", group_label: "Housing" },
    ],
    isLoading: false,
  })),
}));

vi.mock("../api/hooks/onboarding", () => ({
  useOnboardingState: vi.fn(() => ({ data: { account_count: 0, category_count: 0, completion_marked: false }, isLoading: false })),
  useMarkOnboardingComplete: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useResetOnboarding: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
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
    markOnboardingComplete: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("a11y sweep", () => {
  it("Drawer has no axe violations", async () => {
    wrap(<Drawer open onClose={() => {}} title="Test drawer"><p>body</p></Drawer>);
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

  it("CategoryPicker has no axe violations", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <CategoryPicker value={null} onChange={() => {}} />
      </QueryClientProvider>
    );
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("AgentActivityFeed has no axe violations", async () => {
    const { container } = render(<AgentActivityFeed />);
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });
});

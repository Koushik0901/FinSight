import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";

vi.mock("../api/client", () => ({
  commands: {
    getOnboardingState: vi.fn(),
    listAccounts: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    listTransactions: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    appReady: vi.fn().mockResolvedValue({ status: "ok", data: { version: "0.0.0" } }),
    listUnfinishedImports: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    seedSampleHousehold: vi.fn().mockResolvedValue({ status: "ok", data: { accounts_created: 6, transactions_created: 250, import_id: "abc" } }),
    markOnboardingComplete: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    resetOnboardingCompletion: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    clearSampleData: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

async function renderApp(initialPath: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const { App } = await import("../App");
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("App onboarding redirect", () => {
  beforeEach(() => { vi.resetModules(); });

  it("redirects empty DB to /onboarding", async () => {
    const { commands } = await import("../api/client");
    (commands.getOnboardingState as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: false },
    });
    await renderApp("/");
    await waitFor(() => {
      expect(screen.getByTestId("onboarding-shell")).toBeInTheDocument();
    });
  });

  it("does not redirect when accounts exist", async () => {
    const { commands } = await import("../api/client");
    (commands.getOnboardingState as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: { account_count: 3, category_count: 5, completion_marked: false },
    });
    await renderApp("/");
    await waitFor(() => {
      expect(screen.getByText(/no accounts yet\./i)).toBeInTheDocument();
    });
    expect(screen.queryByTestId("onboarding-shell")).not.toBeInTheDocument();
  });

  it("does not redirect when completion_marked even if accounts empty", async () => {
    const { commands } = await import("../api/client");
    (commands.getOnboardingState as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: true },
    });
    await renderApp("/");
    await waitFor(() => {
      expect(screen.getByText(/no accounts yet\./i)).toBeInTheDocument();
    });
    expect(screen.queryByTestId("onboarding-shell")).not.toBeInTheDocument();
  });
});

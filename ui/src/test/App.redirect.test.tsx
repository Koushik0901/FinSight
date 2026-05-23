import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { App } from "../App";

vi.mock("../api/client", () => ({
  commands: {
    getOnboardingState: vi.fn(),
    listAccounts: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    listTransactions: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    appReady: vi.fn().mockResolvedValue({ status: "ok", data: { version: "0.0.0" } }),
    listUnfinishedImports: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

function renderApp(initialPath: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("App onboarding redirect", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("redirects empty DB to /onboarding", async () => {
    const { commands } = await import("../api/client");
    (commands.getOnboardingState as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: false },
    });
    renderApp("/");
    // The Stub component renders "{name} — coming in a later phase."
    // Match the full phrase so we don't hit the Sidebar NavLink text.
    await waitFor(() => {
      expect(screen.getByText(/onboarding — coming in a later phase\./i)).toBeInTheDocument();
    });
  });

  it("does not redirect when accounts exist", async () => {
    const { commands } = await import("../api/client");
    (commands.getOnboardingState as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: { account_count: 3, category_count: 5, completion_marked: false },
    });
    renderApp("/");
    // Stayed on / (Today) — Today renders "No accounts yet." with an empty mock,
    // and the Onboarding stub must NOT appear.
    await waitFor(() => {
      expect(screen.getByText(/no accounts yet\./i)).toBeInTheDocument();
    });
    expect(screen.queryByText(/onboarding — coming in a later phase\./i)).not.toBeInTheDocument();
  });

  it("does not redirect when completion_marked even if accounts empty", async () => {
    const { commands } = await import("../api/client");
    (commands.getOnboardingState as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: true },
    });
    renderApp("/");
    // Stayed on / (Today) — Today renders "No accounts yet." with an empty mock,
    // and the Onboarding stub must NOT appear.
    await waitFor(() => {
      expect(screen.getByText(/no accounts yet\./i)).toBeInTheDocument();
    });
    expect(screen.queryByText(/onboarding — coming in a later phase\./i)).not.toBeInTheDocument();
  });
});

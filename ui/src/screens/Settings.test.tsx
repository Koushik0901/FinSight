import React from "react";
import { afterEach, describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Settings from "./Settings";
import { createWrapper } from "../test-utils";
import { useCompletionProvider, useSaveProviderApiKey, useSetCompletionProvider } from "../api/hooks/agent";
import { fetchAuthStatus, logout } from "../api/auth";

vi.mock("../api/auth", async () => {
  const actual = await vi.importActual<typeof import("../api/auth")>("../api/auth");
  return {
    ...actual,
    logout: vi.fn(),
    fetchAuthStatus: vi.fn().mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: false,
    }),
  };
});

const { mockNavigate } = vi.hoisted(() => ({ mockNavigate: vi.fn() }));
vi.mock("react-router-dom", () => ({
  useNavigate: () => mockNavigate,
  MemoryRouter: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));
vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/onboarding", () => ({
  useResetOnboarding: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useOnboardingState: vi.fn(() => ({ data: { completion_marked: true, account_count: 0, category_count: 0 } })),
}));
vi.mock("../api/hooks/agent", () => ({
  useCompletionProvider: vi.fn(() => ({ data: { kind: "unconfigured" } })),
  useSetCompletionProvider: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSaveProviderApiKey: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useTestCompletionProvider: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue({ ok: true, latency_ms: 120, error: null }),
    isPending: false,
  })),
  useTriggerCategorize: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useListProviderModels: vi.fn(() => ({ data: ["llama3.2"] })),
}));
vi.mock("../api/client", () => ({
  commands: {
    getNeedsReviewCount: vi.fn().mockResolvedValue({ status: "ok", data: 0 }),
  },
}));
vi.mock("../state/tweaks", () => ({
  useTweaks: vi.fn(() => ({
    theme: "dark", density: "cozy", accent: "lime",
    setTheme: vi.fn(), setDensity: vi.fn(), setAccent: vi.fn(),
    privacy: false, setPrivacy: vi.fn(),
  })),
  ACCENTS: { lime: { hex: "#84cc16", ink: "#fff" }, emerald: { hex: "#10b981", ink: "#fff" } },
}));
vi.mock("../api/hooks/settings", () => ({
  useDefaultCurrency: vi.fn(() => ({ data: "USD" })),
  useSetCurrency: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useExportJson: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useExportCsv: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useNotificationsEnabled: vi.fn(() => ({ data: true })),
  useSetNotificationsEnabled: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useAutoCategorizeEnabled: vi.fn(() => ({ data: true })),
  useSetAutoCategorizeEnabled: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useDeleteAllData: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
}));
vi.mock("../api/hooks/notifications", () => ({
  useNotificationPrefs: vi.fn(() => ({
    data: {
      masterEnabled: true,
      categories: [
        { key: "cashflow_risk", label: "Cash-flow risk", enabled: true },
        { key: "account_activity", label: "Account activity", enabled: false },
      ],
      quietHours: { start: 22, end: 7 },
      utcOffsetMinutes: 0,
      privacy: "full",
    },
  })),
  useSetNotificationPrefs: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
}));
vi.mock("../api/hooks/metrics", () => ({
  useFinancialMetrics: vi.fn(() => ({ data: { targetSavingsRatePct: 20, emergencyFundTargetMonths: 6, expectedAnnualReturnPct: 7 } })),
  useSetFinancialAssumptions: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useFinancialPhilosophy: vi.fn(() => ({ data: { debtStrategy: "avalanche", riskTolerance: "balanced", highInterestAprPct: 8 } })),
  useSetFinancialPhilosophy: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
}));
vi.mock("../api/hooks/agentMemory", () => ({
  useAgentMemory: vi.fn(() => ({ data: [{ id: "m1", kind: "correction", description: "Amazon is Shopping, not Uncategorized", merchantKey: "amazon", createdAt: "2026-01-01" }] })),
  useForgetAgentMemory: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
}));
vi.mock("../api/hooks/simplefin", () => ({
  useSimpleFinStatus: vi.fn(() => ({ data: { configured: false } })),
  useDisconnectSimpleFin: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  usePurgeSimpleFinData: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useSimpleFinConnections: vi.fn(() => ({ data: [] })),
  useDeleteSimpleFinConnection: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useSimpleFinSyncSettings: vi.fn(() => ({ data: { backgroundSyncEnabled: true, backgroundSyncIntervalMinutes: 360 } })),
  useSetSimpleFinSyncSettings: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useSaveSimpleFinToken: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useSimpleFinAccounts: vi.fn(() => ({ data: [], refetch: vi.fn(), isFetching: false })),
  useImportSimpleFinAccounts: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("Settings — Appearance section", () => {
  it("renders theme, density, accent, currency controls", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /dark/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /light/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /cozy/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /compact/i })).toBeInTheDocument();
  });

  it("renders data export section with both buttons", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText("Export data")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /export as json/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /export as csv/i })).toBeInTheDocument();
  });
});

describe("Settings — Agent section", () => {
  it("renders Auto-categorize toggle and Agent nav item", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText("Auto-categorize new transactions")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Agent" })).toBeInTheDocument();
  });

  it("shows agent memory (relocated from Insights) and forgets on click", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText("What the agent has learned")).toBeInTheDocument();
    expect(screen.getByText(/Amazon is Shopping/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Forget: Amazon is Shopping/ }));
    // Optimistically removed from the list immediately (before the delayed write).
    expect(screen.queryByText(/Amazon is Shopping/)).toBeNull();
  });
});

describe("Settings — Notifications section", () => {
  it("renders the master switch, category toggles, and quiet-hours window", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByRole("switch", { name: "Notifications enabled" })).toBeInTheDocument();
    // Category rows come from the server's list — a per-category opt-out.
    expect(screen.getByRole("switch", { name: "Cash-flow risk" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Account activity" })).toBeInTheDocument();
    // Quiet hours is on (22→07 in the fixture) so the window pickers show.
    expect(screen.getByRole("combobox", { name: "Quiet hours start" })).toHaveValue("22");
    expect(screen.getByRole("combobox", { name: "Quiet hours end" })).toHaveValue("7");
  });
});

describe("Settings — AI Provider panel", () => {
  beforeEach(() => {
    vi.mocked(useCompletionProvider).mockReturnValue({
      data: { kind: "unconfigured" },
    } as ReturnType<typeof useCompletionProvider>);
  });

  it("shows 'AI Provider' section", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: "AI Provider" })).toBeInTheDocument();
  });

  it("expands config panel on Configure click", async () => {
    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /configure/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /ollama/i })).toBeInTheDocument()
    );
  });

  it("shows Test connection button when Ollama selected", async () => {
    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /configure/i }));
    await waitFor(() => screen.getByRole("button", { name: /ollama/i }));
    fireEvent.click(screen.getByRole("button", { name: /ollama/i }));
    expect(screen.getByRole("button", { name: /test connection/i })).toBeInTheDocument();
  });

  it("shows configured provider summary when panel is closed", () => {
    vi.mocked(useCompletionProvider).mockReturnValue({
      data: { kind: "openai_compat", preset: "openrouter", base_url: "https://openrouter.ai/api/v1", model: "gpt-4o-mini" },
    } as ReturnType<typeof useCompletionProvider>);
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText(/configured — openrouter/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /edit/i })).toBeInTheDocument();
  });

  it("pre-populates cloud provider form when configured", async () => {
    vi.mocked(useCompletionProvider).mockReturnValue({
      data: { kind: "openai_compat", preset: "openrouter", base_url: "https://openrouter.ai/api/v1", model: "gpt-4o-mini" },
    } as ReturnType<typeof useCompletionProvider>);
    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /edit/i }));
    await waitFor(() => {
      expect(screen.getByDisplayValue("gpt-4o-mini")).toBeInTheDocument();
    });
  });

  it("saves the API key before setting the provider", async () => {
    const saveKey = vi.fn().mockResolvedValue(undefined);
    const setProvider = vi.fn().mockResolvedValue(undefined);
    vi.mocked(useSaveProviderApiKey).mockReturnValue({
      mutateAsync: saveKey,
      isPending: false,
    } as unknown as ReturnType<typeof useSaveProviderApiKey>);
    vi.mocked(useSetCompletionProvider).mockReturnValue({
      mutateAsync: setProvider,
      isPending: false,
    } as unknown as ReturnType<typeof useSetCompletionProvider>);

    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /configure/i }));
    await waitFor(() => screen.getByRole("button", { name: /cloud/i }));
    fireEvent.click(screen.getByRole("button", { name: /cloud/i }));
    fireEvent.click(screen.getByText(/openrouter/i));

    fireEvent.change(screen.getByPlaceholderText(/e\.g\. gpt-4o-mini/i), { target: { value: "gpt-4o-mini" } });
    fireEvent.change(screen.getByPlaceholderText(/sk-…/i), { target: { value: "sk-or-test" } });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));

    await waitFor(() => {
      expect(saveKey).toHaveBeenCalledWith({ providerId: "openrouter", key: "sk-or-test" });
      expect(setProvider).toHaveBeenCalled();
      const saveOrder = saveKey.mock.invocationCallOrder[0] ?? Infinity;
      const setOrder = setProvider.mock.invocationCallOrder[0] ?? Infinity;
      expect(saveOrder).toBeLessThan(setOrder);
    });
  });
});

describe("Settings — server-mode Account section", () => {
  afterEach(() => {
    vi.clearAllMocks();
    delete (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__;
  });

  it("desktop mode: no Account section, no Sign out button", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.queryByRole("heading", { name: "Account" })).toBeNull();
    expect(screen.queryByRole("button", { name: /sign out/i })).toBeNull();
  });

  it("server mode: renders the Account section with a Sign out button", () => {
    (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__ = true;
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: "Account" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Account" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /sign out/i })).toBeInTheDocument();
  });

  it("Sign out calls logout() and dispatches finsight:auth-required", async () => {
    (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__ = true;
    vi.mocked(logout).mockResolvedValue(undefined);
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /sign out/i }));

    await waitFor(() => expect(logout).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(dispatchSpy).toHaveBeenCalledWith(expect.objectContaining({ type: "finsight:auth-required" }))
    );
  });

  it("still dispatches finsight:auth-required even if the logout request fails", async () => {
    (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__ = true;
    vi.mocked(logout).mockRejectedValue({ code: "rpc.transport", message: "network down" });
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    render(<Settings />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /sign out/i }));

    await waitFor(() =>
      expect(dispatchSpy).toHaveBeenCalledWith(expect.objectContaining({ type: "finsight:auth-required" }))
    );
  });

  it("does not show a Manage users entry for a non-admin session", async () => {
    (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "sam",
      isAdmin: false,
    });

    render(<Settings />, { wrapper: createWrapper() });

    await waitFor(() => expect(fetchAuthStatus).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /manage users/i })).toBeNull();
  });

  it("shows a Manage users entry for an admin session and navigates to /settings/users on click", async () => {
    (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });

    render(<Settings />, { wrapper: createWrapper() });

    const manageUsers = await screen.findByRole("button", { name: /manage users/i });
    fireEvent.click(manageUsers);

    expect(mockNavigate).toHaveBeenCalledWith("/settings/users");
  });
});

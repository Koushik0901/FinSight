import React from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Settings from "./Settings";
import { createWrapper } from "../test-utils";
import { useCompletionProvider, useSaveProviderApiKey, useSetCompletionProvider } from "../api/hooks/agent";

vi.mock("react-router-dom", () => ({
  useNavigate: vi.fn(() => vi.fn()),
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

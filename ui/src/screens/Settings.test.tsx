import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Settings from "./Settings";
import { createWrapper } from "../test-utils";

vi.mock("react-router-dom", () => ({
  useNavigate: vi.fn(() => vi.fn()),
}));
vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/onboarding", () => ({
  useResetOnboarding: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useClearSampleData: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useSeedDevDemo: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({ transactions_created: 142, accounts_created: 6, import_id: "test" }), isPending: false })),
  useOnboardingState: vi.fn(() => ({ data: { completion_marked: true, account_count: 0, category_count: 0 } })),
}));
vi.mock("../api/hooks/agent", () => ({
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
}));

describe("Settings — Appearance section", () => {
  it("renders theme, density, accent, currency controls", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText("Appearance")).toBeInTheDocument();
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

describe("Settings — AI Provider panel", () => {
  it("shows 'AI Provider' section", () => {
    render(<Settings />, { wrapper: createWrapper() });
    expect(screen.getByText("AI Provider")).toBeInTheDocument();
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
});

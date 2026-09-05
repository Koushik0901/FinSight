import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import StepAgent from "./StepAgent";
import { createWrapper } from "../../test-utils";
import { useSaveProviderApiKey, useSetCompletionProvider } from "../../api/hooks/agent";

const { probeOllama } = vi.hoisted(() => ({
  probeOllama: vi.fn().mockResolvedValue({ status: "ok", data: { reachable: false, models: [], has_nomic_embed: false } }),
}));

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../../api/hooks/onboarding", () => ({
  useMarkOnboardingComplete: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));
vi.mock("../../api/hooks/agent", () => ({
  useSetCompletionProvider: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSaveProviderApiKey: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useTestCompletionProvider: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue({ ok: true, latency_ms: 80, error: null }),
    isPending: false,
  })),
  useListProviderModels: vi.fn(() => ({ data: [] })),
}));
vi.mock("../../api/openapiClient", () => ({
  unwrap: async (p: Promise<{ status: "ok" | "error"; data?: unknown; error?: { message: string } }>) => { const r = await p; if (r.status === "error") throw new Error(r.error?.message ?? "command failed"); return r.data; },
  api: {
    probeOllama,
    saveLlmProvider: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("StepAgent", () => {
  it("shows two-path choice: Local + Cloud", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /self-hosted ollama/i })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /cloud/i })).toBeInTheDocument();
    });
    expect(screen.getByText(/private server-side inference/i)).toBeInTheDocument();
  });

  it("lets a self-hosted deployment enter the server-reachable Ollama URL", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /self-hosted ollama/i }));

    const url = await screen.findByRole("textbox", { name: /ollama url/i });
    expect(url).toHaveValue("http://ollama:11434");
    expect(screen.getByText(/http:\/\/ollama:11434/i)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /ollama setup guide/i })).toHaveAttribute("href", "https://ollama.com");
  });

  it("degrades a partial Ollama probe response to the connection form", async () => {
    probeOllama.mockResolvedValueOnce({ status: "ok", data: [] } as never);
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /self-hosted ollama/i }));

    expect(await screen.findByRole("textbox", { name: /ollama url/i })).toBeInTheDocument();
    expect(screen.queryByText(/this screen failed to load/i)).not.toBeInTheDocument();
  });

  it("shows cloud provider tiles after clicking Cloud path", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    await waitFor(() => screen.getByRole("button", { name: /cloud/i }));
    fireEvent.click(screen.getByRole("button", { name: /cloud/i }));
    await waitFor(() => expect(screen.getByText(/openai/i)).toBeInTheDocument());
  });

  it("shows Configure later button at all times", async () => {
    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByRole("button", { name: /configure later/i })).toBeInTheDocument());
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

    render(<StepAgent onDone={() => {}} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /cloud/i }));
    await waitFor(() => screen.getByText(/openrouter/i));
    fireEvent.click(screen.getByText(/openrouter/i));

    fireEvent.change(screen.getByPlaceholderText(/e\.g\. gpt-4o-mini/i), { target: { value: "gpt-4o-mini" } });
    fireEvent.change(screen.getByPlaceholderText(/sk-…/i), { target: { value: "sk-or-test" } });
    fireEvent.click(screen.getByRole("button", { name: /test & save/i }));

    await waitFor(() => {
      expect(saveKey).toHaveBeenCalledWith({ providerId: "openrouter", key: "sk-or-test" });
      expect(setProvider).toHaveBeenCalled();
      const saveOrder = saveKey.mock.invocationCallOrder[0] ?? Infinity;
      const setOrder = setProvider.mock.invocationCallOrder[0] ?? Infinity;
      expect(saveOrder).toBeLessThan(setOrder);
    });
  });
});

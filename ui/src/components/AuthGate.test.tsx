import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthGate } from "./AuthGate";
import { fetchAuthStatus } from "../api/auth";
import { purgePersistedCache } from "../pwa/persist";

type AnyRec = Record<string, unknown>;

vi.mock("../api/auth", async () => {
  const actual = await vi.importActual<typeof import("../api/auth")>("../api/auth");
  return {
    ...actual,
    fetchAuthStatus: vi.fn(),
  };
});

vi.mock("../pwa/persist", () => ({
  purgePersistedCache: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../screens/server/SetupScreen", () => ({
  default: ({ onComplete }: { onComplete: () => void }) => (
    <div>
      <span>SETUP_SCREEN</span>
      <button onClick={onComplete}>complete-setup</button>
    </div>
  ),
}));

vi.mock("../screens/server/LoginScreen", () => ({
  default: ({ onComplete }: { onComplete: () => void }) => (
    <div>
      <span>LOGIN_SCREEN</span>
      <button onClick={onComplete}>complete-login</button>
    </div>
  ),
}));

function renderGate() {
  const queryClient = new QueryClient();
  const clearSpy = vi.spyOn(queryClient, "clear");
  const result = render(
    <QueryClientProvider client={queryClient}>
      <AuthGate>
        <div>APP_CONTENT</div>
      </AuthGate>
    </QueryClientProvider>
  );
  return { ...result, clearSpy };
}

describe("AuthGate — boot gating", () => {
  afterEach(() => {
    vi.clearAllMocks();
    delete (window as unknown as AnyRec).__FINSIGHT_HTTP__;
  });

  it("desktop mode (no __FINSIGHT_HTTP__): renders children immediately, never calls fetchAuthStatus", () => {
    renderGate();
    expect(screen.getByText("APP_CONTENT")).toBeInTheDocument();
    expect(fetchAuthStatus).not.toHaveBeenCalled();
  });

  it("server mode + needsSetup: renders SetupScreen, not children", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: true,
      authenticated: false,
      username: null,
      isAdmin: null,
    });
    renderGate();

    expect(await screen.findByText("SETUP_SCREEN")).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
  });

  it("server mode + !authenticated: renders LoginScreen, not children", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: false,
      username: null,
      isAdmin: null,
    });
    renderGate();

    expect(await screen.findByText("LOGIN_SCREEN")).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
  });

  it("server mode + authenticated: renders children", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    renderGate();

    expect(await screen.findByText("APP_CONTENT")).toBeInTheDocument();
  });

  it("clears the query cache and shows children after Login completes", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: false,
      username: null,
      isAdmin: null,
    });
    const { clearSpy } = renderGate();
    await screen.findByText("LOGIN_SCREEN");

    act(() => {
      screen.getByText("complete-login").click();
    });

    expect(clearSpy).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("APP_CONTENT")).toBeInTheDocument();
  });

  it("clears the query cache and shows children after Setup completes", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: true,
      authenticated: false,
      username: null,
      isAdmin: null,
    });
    const { clearSpy } = renderGate();
    await screen.findByText("SETUP_SCREEN");

    act(() => {
      screen.getByText("complete-setup").click();
    });

    expect(clearSpy).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("APP_CONTENT")).toBeInTheDocument();
  });

  it("routes back to LoginScreen on a finsight:auth-required event fired from anywhere, clearing and purging the cache", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: false,
    });
    const { clearSpy } = renderGate();
    await screen.findByText("APP_CONTENT");

    act(() => {
      window.dispatchEvent(new CustomEvent("finsight:auth-required"));
    });

    expect(await screen.findByText("LOGIN_SCREEN")).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
    expect(clearSpy).toHaveBeenCalled();
    expect(purgePersistedCache).toHaveBeenCalledTimes(1);
  });

  it("desktop mode never registers a finsight:auth-required listener that changes rendered content", async () => {
    renderGate();
    expect(screen.getByText("APP_CONTENT")).toBeInTheDocument();

    act(() => {
      window.dispatchEvent(new CustomEvent("finsight:auth-required"));
    });

    // Give any stray microtask/listener a tick, then assert nothing changed.
    await waitFor(() => expect(screen.getByText("APP_CONTENT")).toBeInTheDocument());
    expect(fetchAuthStatus).not.toHaveBeenCalled();
    expect(purgePersistedCache).not.toHaveBeenCalled();
  });

  it("shows a retry option when fetchAuthStatus fails, and re-checks on retry", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(fetchAuthStatus).mockRejectedValueOnce(new TypeError("Failed to fetch"));
    renderGate();

    const retryButton = await screen.findByRole("button", { name: /retry/i });

    vi.mocked(fetchAuthStatus).mockResolvedValueOnce({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: false,
    });
    act(() => {
      retryButton.click();
    });

    expect(await screen.findByText("APP_CONTENT")).toBeInTheDocument();
    expect(fetchAuthStatus).toHaveBeenCalledTimes(2);
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthGate } from "./AuthGate";
import OfflineBanner from "./OfflineBanner";
import { fetchAuthStatus, hadPriorSession, markSessionEstablished } from "../api/auth";
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
    // The offline-boot marker lives in localStorage and now survives across
    // tests — a leaked marker silently flips the "connection problem" wall
    // into the offline branch and would test the wrong path.
    localStorage.clear();
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
    // Symmetric with the logout/401 path: the IndexedDB copy must go too, or
    // a late persister restore can leak the PREVIOUS user's cache into this
    // freshly-authenticated session on a shared device.
    expect(purgePersistedCache).toHaveBeenCalledTimes(1);
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
    expect(purgePersistedCache).toHaveBeenCalledTimes(1);
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

/**
 * The PWA offline story: install → sign in → go offline → relaunch must show
 * the last synced data, not a connection wall. That only works if the gate
 * renders `children` (which is where main.tsx nests OfflineBanner AND where
 * the IndexedDB-persisted query cache is consumed) on a NETWORK failure.
 *
 * The load-bearing invariant in the other direction: a real auth verdict must
 * never be absorbed by this fallback.
 */
describe("AuthGate — offline boot", () => {
  afterEach(() => {
    vi.clearAllMocks();
    delete (window as unknown as AnyRec).__FINSIGHT_HTTP__;
    localStorage.clear();
  });

  function goServerMode() {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
  }

  it("network failure + a prior session: renders children so the cache and OfflineBanner show", async () => {
    goServerMode();
    markSessionEstablished("koushik");
    // `fetch` rejecting outright — no HTTP response, so no `code` on the error.
    vi.mocked(fetchAuthStatus).mockRejectedValue(new TypeError("Failed to fetch"));
    renderGate();

    expect(await screen.findByText("APP_CONTENT")).toBeInTheDocument();
    expect(screen.queryByText("LOGIN_SCREEN")).toBeNull();
    expect(screen.queryByText(/Can't reach the FinSight server/i)).toBeInTheDocument();
  });

  // The literal claim in the bug report: OfflineBanner is nested INSIDE
  // <AuthGate> in main.tsx, so while the gate withheld `children` the banner
  // could never mount — the offline story was unreachable exactly when
  // needed. This renders the real banner as a child to prove it now mounts.
  it("mounts the real OfflineBanner (nested in children, as main.tsx does)", async () => {
    goServerMode();
    markSessionEstablished("koushik");
    // Restore this spy specifically — `vi.restoreAllMocks()` would also strip
    // the `purgePersistedCache` module mock's resolved-value implementation,
    // breaking every later test that awaits it.
    const onLineSpy = vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
    vi.mocked(fetchAuthStatus).mockRejectedValue(new TypeError("Failed to fetch"));

    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <AuthGate>
          <OfflineBanner />
          <div>APP_CONTENT</div>
        </AuthGate>
      </QueryClientProvider>
    );

    expect(await screen.findByText("APP_CONTENT")).toBeInTheDocument();
    expect(screen.getByText(/showing your last synced data\. Changes are paused/i)).toBeInTheDocument();
    onLineSpy.mockRestore();
  });

  it("keeps a retry affordance in the offline state, and recovers when the server returns", async () => {
    goServerMode();
    markSessionEstablished("koushik");
    vi.mocked(fetchAuthStatus).mockRejectedValueOnce(new TypeError("Failed to fetch"));
    renderGate();

    const retry = await screen.findByRole("button", { name: /retry/i });

    vi.mocked(fetchAuthStatus).mockResolvedValueOnce({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: false,
    });
    act(() => {
      retry.click();
    });

    await waitFor(() => expect(screen.queryByText(/Can't reach the FinSight server/i)).toBeNull());
    expect(screen.getByText("APP_CONTENT")).toBeInTheDocument();
  });

  it("network failure WITHOUT a prior session: still the connection-problem wall", async () => {
    goServerMode();
    vi.mocked(fetchAuthStatus).mockRejectedValue(new TypeError("Failed to fetch"));
    renderGate();

    expect(await screen.findByRole("button", { name: /retry/i })).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
  });

  it("a real auth rejection routes to login even WITH a prior session, and clears the marker", async () => {
    goServerMode();
    markSessionEstablished("koushik");
    // A genuine 401 comes back as the parsed AppError shape, not a TypeError.
    vi.mocked(fetchAuthStatus).mockRejectedValue({
      code: "auth.required",
      message: "Authentication required.",
    });
    renderGate();

    expect(await screen.findByText("LOGIN_SCREEN")).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
    expect(hadPriorSession()).toBe(false);
  });

  it("a server that says we're logged out clears the marker (no offline fallback next boot)", async () => {
    goServerMode();
    markSessionEstablished("koushik");
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: false,
      username: null,
      isAdmin: null,
    });
    renderGate();

    await screen.findByText("LOGIN_SCREEN");
    expect(hadPriorSession()).toBe(false);
  });

  it("a finsight:auth-required event (sign-out / 401) clears the marker", async () => {
    goServerMode();
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: false,
    });
    renderGate();
    await screen.findByText("APP_CONTENT");
    expect(hadPriorSession()).toBe(true);

    act(() => {
      window.dispatchEvent(new CustomEvent("finsight:auth-required"));
    });

    await screen.findByText("LOGIN_SCREEN");
    expect(hadPriorSession()).toBe(false);
  });
});

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import OAuthAuthorize from "./OAuthAuthorize";
import { approveOAuth, fetchOAuthClientName } from "../../api/oauth";

vi.mock("../../api/oauth", async () => {
  const actual = await vi.importActual<typeof import("../../api/oauth")>("../../api/oauth");
  return {
    ...actual,
    fetchOAuthClientName: vi.fn(),
    approveOAuth: vi.fn(),
  };
});

const CHALLENGE = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const REDIRECT = "https://claude.ai/api/mcp/auth_callback";

function query(overrides: Record<string, string | null> = {}) {
  const base: Record<string, string | null> = {
    client_id: "client-123",
    redirect_uri: REDIRECT,
    state: "opaque-state",
    code_challenge: CHALLENGE,
    code_challenge_method: "S256",
    scope: "read",
    ...overrides,
  };
  const params = new URLSearchParams();
  for (const [k, v] of Object.entries(base)) if (v !== null) params.set(k, v);
  return `/oauth/authorize?${params.toString()}`;
}

function renderAt(url: string) {
  return render(
    <MemoryRouter initialEntries={[url]}>
      <OAuthAuthorize />
    </MemoryRouter>
  );
}

let assign: ReturnType<typeof vi.fn>;

beforeEach(() => {
  assign = vi.fn();
  // jsdom won't navigate; capture where the screen tried to send the browser.
  Object.defineProperty(window, "location", {
    configurable: true,
    value: { ...window.location, assign },
  });
  vi.mocked(fetchOAuthClientName).mockResolvedValue("Claude");
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("OAuthAuthorize", () => {
  it("names the app requesting access", async () => {
    renderAt(query());
    expect(await screen.findByText(/Connect Claude to FinSight\?/)).toBeInTheDocument();
    expect(fetchOAuthClientName).toHaveBeenCalledWith("client-123");
  });

  it("sends the browser to the callback after approval", async () => {
    vi.mocked(approveOAuth).mockResolvedValue(`${REDIRECT}?code=abc&state=opaque-state`);
    renderAt(query());

    fireEvent.click(await screen.findByRole("button", { name: /allow claude/i }));

    await waitFor(() =>
      expect(approveOAuth).toHaveBeenCalledWith({
        clientId: "client-123",
        redirectUri: REDIRECT,
        scope: "read",
        state: "opaque-state",
        codeChallenge: CHALLENGE,
        codeChallengeMethod: "S256",
      })
    );
    await waitFor(() =>
      expect(assign).toHaveBeenCalledWith(`${REDIRECT}?code=abc&state=opaque-state`)
    );
  });

  it("honours a full-access request but lets the user downgrade it", async () => {
    vi.mocked(approveOAuth).mockResolvedValue(`${REDIRECT}?code=abc`);
    renderAt(query({ scope: "full" }));

    expect(await screen.findByRole("button", { name: "Read and write" })).toHaveAttribute(
      "aria-pressed",
      "true"
    );

    // The user's choice wins over what the client asked for.
    fireEvent.click(screen.getByRole("button", { name: "Read only" }));
    fireEvent.click(screen.getByRole("button", { name: /allow claude/i }));

    await waitFor(() =>
      expect(approveOAuth).toHaveBeenCalledWith(expect.objectContaining({ scope: "read" }))
    );
  });

  it("returns access_denied to the client on deny, preserving state", async () => {
    renderAt(query());
    fireEvent.click(await screen.findByRole("button", { name: /deny/i }));

    expect(approveOAuth).not.toHaveBeenCalled();
    expect(assign).toHaveBeenCalledWith(
      `${REDIRECT}?error=access_denied&state=opaque-state`
    );
  });

  /**
   * The security-critical case: with parameters missing there is no verified
   * redirect target, so the screen must refuse rather than bounce the browser
   * to whatever it was handed.
   */
  it.each([
    ["no client_id", { client_id: null }],
    ["no redirect_uri", { redirect_uri: null }],
    ["no code_challenge", { code_challenge: null }],
    ["plain PKCE", { code_challenge_method: "plain" }],
  ])("refuses and does not redirect when there is %s", async (_label, overrides) => {
    renderAt(query(overrides));

    expect(await screen.findByText(/isn't valid/i)).toBeInTheDocument();
    expect(assign).not.toHaveBeenCalled();
    expect(fetchOAuthClientName).not.toHaveBeenCalled();
    expect(approveOAuth).not.toHaveBeenCalled();
  });


  // Registration is open (RFC 7591), so `client_name` is whatever the app typed
  // when it registered — anyone can call themselves "Claude Desktop". The
  // callback host is the one field a spoofer cannot fake, so the card is only
  // safe if it is on screen.
  it("shows where the code would actually be sent, not just the app's name", async () => {
    renderAt(query());
    const dest = await screen.findByTestId("oauth-destination");
    expect(dest).toHaveTextContent("claude.ai");
  });

  it("warns that a non-loopback destination should be recognised", async () => {
    vi.mocked(fetchOAuthClientName).mockResolvedValue("Claude Desktop");
    renderAt(query({ redirect_uri: "https://totally-not-anthropic.example/cb" }));
    const dest = await screen.findByTestId("oauth-destination");
    // The spoof is only detectable if the real host is shown next to the claim.
    expect(dest).toHaveTextContent("totally-not-anthropic.example");
    expect(dest).toHaveTextContent(/anyone can register an app under any name/i);
  });

  it("resolves the true host when userinfo is used to disguise it", async () => {
    // "https://claude.ai@evil.example/cb" reads as claude.ai but resolves to
    // evil.example. Parsing rather than string-matching is what catches it.
    renderAt(query({ redirect_uri: "https://claude.ai@evil.example/cb" }));
    const dest = await screen.findByTestId("oauth-destination");
    expect(dest).toHaveTextContent("evil.example");
    expect(dest).not.toHaveTextContent(/^claude\.ai$/);
  });

  it("refuses a redirect_uri that is not https or loopback", async () => {
    for (const bad of ["javascript:alert(1)", "http://evil.example/cb", "not-a-url"]) {
      const { unmount } = renderAt(query({ redirect_uri: bad }));
      expect(await screen.findByText(/isn't valid/i)).toBeInTheDocument();
      expect(assign).not.toHaveBeenCalled();
      unmount();
    }
  });

  it("treats a loopback callback as a local app rather than warning", async () => {
    renderAt(query({ redirect_uri: "http://127.0.0.1:33418/oauth/callback" }));
    const dest = await screen.findByTestId("oauth-destination");
    expect(dest).toHaveTextContent("127.0.0.1:33418");
    expect(dest).toHaveTextContent(/app running on this machine/i);
  });

  it("reports an unregistered client without offering to continue", async () => {
    vi.mocked(fetchOAuthClientName).mockRejectedValue({
      error: "invalid_client",
      error_description: "unknown client_id",
    });
    renderAt(query());

    expect(await screen.findByRole("alert")).toHaveTextContent(/unknown client_id/i);
    expect(screen.queryByRole("button", { name: /allow/i })).toBeNull();
  });

  it("keeps the user on the page when approval fails", async () => {
    vi.mocked(approveOAuth).mockRejectedValue({
      error: "invalid_request",
      error_description: "redirect_uri does not match this client's registration",
    });
    renderAt(query());

    fireEvent.click(await screen.findByRole("button", { name: /allow claude/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/does not match/i);
    expect(assign).not.toHaveBeenCalled();
  });
});

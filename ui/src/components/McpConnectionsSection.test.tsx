import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import McpConnectionsSection from "./McpConnectionsSection";
import { createApiToken, listApiTokens, revokeApiToken } from "../api/tokens";

vi.mock("../api/tokens", async () => {
  const actual = await vi.importActual<typeof import("../api/tokens")>("../api/tokens");
  return {
    ...actual,
    listApiTokens: vi.fn(),
    createApiToken: vi.fn(),
    revokeApiToken: vi.fn(),
  };
});

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const TOKENS = [
  {
    id: "t1",
    name: "Claude Desktop",
    scope: "full" as const,
    createdAt: "2026-07-01T00:00:00Z",
    lastUsedAt: "2026-07-20T09:30:00Z",
  },
  {
    id: "t2",
    name: "ChatGPT",
    scope: "read" as const,
    createdAt: "2026-07-02T00:00:00Z",
    lastUsedAt: null,
  },
];

describe("McpConnectionsSection", () => {
  beforeEach(() => {
    vi.mocked(listApiTokens).mockResolvedValue([]);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows the MCP endpoint so the user can paste it into a client", async () => {
    render(<McpConnectionsSection />);
    expect(await screen.findByText(`${window.location.origin}/mcp`)).toBeInTheDocument();
  });

  it("lists existing tokens with their access level and last use", async () => {
    vi.mocked(listApiTokens).mockResolvedValue(TOKENS);
    render(<McpConnectionsSection />);

    expect(await screen.findByText("Claude Desktop")).toBeInTheDocument();
    expect(screen.getByText("Read and write")).toBeInTheDocument();
    expect(screen.getByText("ChatGPT")).toBeInTheDocument();
    expect(screen.getByText("Read only")).toBeInTheDocument();
    // A token that has never been used must say so rather than showing a
    // misleading epoch date.
    expect(screen.getByText("Never")).toBeInTheDocument();
  });

  it("defaults a new token to read-only", async () => {
    render(<McpConnectionsSection />);
    fireEvent.click(await screen.findByRole("button", { name: /create token/i }));

    const readOnly = screen.getByRole("button", { name: "Read only" });
    expect(readOnly).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Read and write" })).toHaveAttribute(
      "aria-pressed",
      "false"
    );
  });

  it("reveals a created token exactly once and never re-fetches it", async () => {
    vi.mocked(createApiToken).mockResolvedValue({
      id: "t9",
      name: "Claude Desktop",
      scope: "full",
      createdAt: "2026-07-25T00:00:00Z",
      lastUsedAt: null,
      token: "finsight_pat_SECRETSECRETSECRETSECRETSECRETSECRETSECRET",
    });

    render(<McpConnectionsSection />);
    fireEvent.click(await screen.findByRole("button", { name: /create token/i }));
    fireEvent.change(screen.getByLabelText(/name/i), { target: { value: "Claude Desktop" } });
    fireEvent.click(screen.getByRole("button", { name: "Read and write" }));
    fireEvent.click(screen.getByRole("button", { name: /^create token$/i }));

    await waitFor(() =>
      expect(createApiToken).toHaveBeenCalledWith("Claude Desktop", "full")
    );

    // Shown twice on purpose: once on its own to copy, once inside the
    // ready-to-run Claude Code command.
    const shown = await screen.findAllByText(/finsight_pat_SECRET/);
    expect(shown).toHaveLength(2);
    expect(screen.getByText(/claude mcp add --transport http finsight/)).toBeInTheDocument();

    // Dismissing removes it from the DOM — the list view never shows secrets,
    // so there is no way back to it.
    fireEvent.click(screen.getByRole("button", { name: /done/i }));
    await waitFor(() => expect(screen.queryAllByText(/finsight_pat_SECRET/)).toHaveLength(0));
  });

  it("refuses to create an unnamed token", async () => {
    render(<McpConnectionsSection />);
    fireEvent.click(await screen.findByRole("button", { name: /create token/i }));
    fireEvent.click(screen.getByRole("button", { name: /^create token$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/give this token a name/i);
    expect(createApiToken).not.toHaveBeenCalled();
  });

  it("revokes a token after confirmation and refreshes the list", async () => {
    vi.mocked(listApiTokens).mockResolvedValue(TOKENS);
    vi.mocked(revokeApiToken).mockResolvedValue();
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<McpConnectionsSection />);
    const [firstRevoke] = await screen.findAllByRole("button", { name: /revoke/i });
    vi.mocked(listApiTokens).mockResolvedValue(TOKENS.slice(1));
    fireEvent.click(firstRevoke!);

    await waitFor(() => expect(revokeApiToken).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(screen.queryByText("Claude Desktop")).toBeNull());
  });

  it("does not revoke when the confirmation is declined", async () => {
    vi.mocked(listApiTokens).mockResolvedValue(TOKENS);
    vi.spyOn(window, "confirm").mockReturnValue(false);

    render(<McpConnectionsSection />);
    const [firstRevoke] = await screen.findAllByRole("button", { name: /revoke/i });
    fireEvent.click(firstRevoke!);

    expect(revokeApiToken).not.toHaveBeenCalled();
  });

  it("surfaces a load failure instead of rendering an empty list", async () => {
    vi.mocked(listApiTokens).mockRejectedValue({
      code: "auth.db",
      message: "database is locked",
    });

    render(<McpConnectionsSection />);
    expect(await screen.findByRole("alert")).toBeInTheDocument();
  });
});

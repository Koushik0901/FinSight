import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import UsersAdmin from "./UsersAdmin";
import { createUser, deleteUser, fetchAuthStatus, listUsers } from "../../api/auth";

vi.mock("../../api/auth", async () => {
  const actual = await vi.importActual<typeof import("../../api/auth")>("../../api/auth");
  return {
    ...actual,
    fetchAuthStatus: vi.fn(),
    listUsers: vi.fn(),
    createUser: vi.fn(),
    deleteUser: vi.fn(),
  };
});

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const RECOVERY_KEY = "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-11111111-22222222";

const USERS = [
  { id: "u1", username: "koushik", isAdmin: true, createdAt: "2026-07-01T00:00:00Z" },
  { id: "u2", username: "sam", isAdmin: false, createdAt: "2026-07-02T00:00:00Z" },
];

function setServerMode(on: boolean) {
  if (on) (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__ = true;
  else delete (window as unknown as Record<string, unknown>).__FINSIGHT_HTTP__;
}

describe("UsersAdmin", () => {
  afterEach(() => {
    vi.clearAllMocks();
    setServerMode(false);
    vi.restoreAllMocks();
  });

  it("renders nothing outside server mode (desktop/Tauri)", () => {
    setServerMode(false);
    const { container } = render(<UsersAdmin />);
    expect(container).toBeEmptyDOMElement();
    expect(fetchAuthStatus).not.toHaveBeenCalled();
  });

  it("shows a not-authorized note for a non-admin session, without attempting to list users", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "sam",
      isAdmin: false,
    });
    // Mirrors the real backend: GET /api/auth/users 403s auth.admin_required
    // for non-admins. The component must gate on isAdmin from the status
    // response BEFORE calling listUsers(), not rely on this rejection.
    vi.mocked(listUsers).mockRejectedValue({
      code: "auth.admin_required",
      message: "Admin access required.",
    });

    render(<UsersAdmin />);

    expect(await screen.findByText(/don't have permission/i)).toBeInTheDocument();
    expect(screen.queryByRole("table")).toBeNull();
    expect(listUsers).not.toHaveBeenCalled();
  });

  it("renders the user list with an admin badge and created date", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    vi.mocked(listUsers).mockResolvedValue(USERS);

    render(<UsersAdmin />);

    expect(await screen.findByText("koushik")).toBeInTheDocument();
    expect(screen.getByText("sam")).toBeInTheDocument();
    expect(screen.getByText("Admin")).toBeInTheDocument();
  });

  it("disables Delete on the current user's own row but not on others", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    vi.mocked(listUsers).mockResolvedValue(USERS);

    render(<UsersAdmin />);
    await screen.findByText("koushik");

    const rows = screen.getAllByRole("row").slice(1); // drop header row
    const koushikRow = rows.find((r) => r.textContent?.includes("koushik"))!;
    const samRow = rows.find((r) => r.textContent?.includes("sam"))!;

    expect(within(koushikRow).getByRole("button", { name: /delete/i })).toBeDisabled();
    expect(within(samRow).getByRole("button", { name: /delete/i })).not.toBeDisabled();
  });

  it("deletes another user after confirmation and refreshes the list", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    vi.mocked(listUsers).mockResolvedValue(USERS);
    vi.mocked(deleteUser).mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<UsersAdmin />);
    await screen.findByText("sam");

    const rows = screen.getAllByRole("row").slice(1);
    const samRow = rows.find((r) => r.textContent?.includes("sam"))!;
    fireEvent.click(within(samRow).getByRole("button", { name: /delete/i }));

    await waitFor(() => expect(deleteUser).toHaveBeenCalledWith("u2"));
    await waitFor(() => expect(listUsers).toHaveBeenCalledTimes(2));
  });

  it("does not delete when the confirm dialog is dismissed", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    vi.mocked(listUsers).mockResolvedValue(USERS);
    vi.spyOn(window, "confirm").mockReturnValue(false);

    render(<UsersAdmin />);
    await screen.findByText("sam");

    const rows = screen.getAllByRole("row").slice(1);
    const samRow = rows.find((r) => r.textContent?.includes("sam"))!;
    fireEvent.click(within(samRow).getByRole("button", { name: /delete/i }));

    expect(deleteUser).not.toHaveBeenCalled();
  });

  it("add-user flow shows the recovery key once via RecoveryKeyReveal, then refreshes on continue", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    vi.mocked(listUsers).mockResolvedValue(USERS);
    vi.mocked(createUser).mockResolvedValue({ recoveryKey: RECOVERY_KEY });

    render(<UsersAdmin />);
    await screen.findByText("koushik");

    fireEvent.change(screen.getByLabelText(/^username$/i), { target: { value: "newperson" } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: "hunter2hunter2" } });
    fireEvent.click(screen.getByRole("button", { name: /^add user$/i }));

    await waitFor(() => expect(createUser).toHaveBeenCalledWith("newperson", "hunter2hunter2"));
    // RecoveryKeyReveal's distinguishing copy — proves the shared component rendered.
    expect(await screen.findByText(/this is shown once/i)).toBeInTheDocument();
    expect(screen.getByText(RECOVERY_KEY)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("checkbox", { name: /saved my recovery key/i }));
    fireEvent.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => expect(listUsers).toHaveBeenCalledTimes(2));
  });

  it("shows a local error and does not call createUser when the form is incomplete", async () => {
    setServerMode(true);
    vi.mocked(fetchAuthStatus).mockResolvedValue({
      needsSetup: false,
      authenticated: true,
      username: "koushik",
      isAdmin: true,
    });
    vi.mocked(listUsers).mockResolvedValue(USERS);

    render(<UsersAdmin />);
    await screen.findByText("koushik");

    fireEvent.click(screen.getByRole("button", { name: /^add user$/i }));

    expect(screen.getByText(/enter a username and password/i)).toBeInTheDocument();
    expect(createUser).not.toHaveBeenCalled();
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import RecoverScreen from "./RecoverScreen";
import { recoverAccount } from "../../api/auth";

vi.mock("../../api/auth", () => ({
  recoverAccount: vi.fn(),
}));

const NEW_KEY = "11111111-22222222-33333333-44444444-55555555-66666666-77777777-88888888";
const OLD_KEY = "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd";

function fillForm({
  username = "koushik",
  key = OLD_KEY,
  password = "correcthorsebattery",
  confirm = "correcthorsebattery",
}: { username?: string; key?: string; password?: string; confirm?: string } = {}) {
  fireEvent.change(screen.getByLabelText(/^username$/i), { target: { value: username } });
  fireEvent.change(screen.getByLabelText(/^recovery key$/i), { target: { value: key } });
  // Not `$`-anchored: the field carries a hint, which joins its accessible
  // name ("New password At least 10 characters."). Still unambiguous —
  // "Confirm new password" doesn't start with "New password".
  fireEvent.change(screen.getByLabelText(/^new password/i), { target: { value: password } });
  fireEvent.change(screen.getByLabelText(/^confirm new password$/i), { target: { value: confirm } });
}

function submit() {
  fireEvent.click(screen.getByRole("button", { name: /^reset password$/i }));
}

describe("RecoverScreen", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("submits username/recoveryKey/newPassword and reveals the NEW recovery key on success", async () => {
    vi.mocked(recoverAccount).mockResolvedValue({ recoveryKey: NEW_KEY });
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm();
    submit();

    await waitFor(() =>
      expect(recoverAccount).toHaveBeenCalledWith("koushik", OLD_KEY, "correcthorsebattery")
    );
    expect(await screen.findByText(NEW_KEY)).toBeInTheDocument();
    // The old key must not linger on screen next to the new one.
    expect(screen.queryByText(OLD_KEY)).toBeNull();
  });

  it("only calls onComplete after the new key is acknowledged via RecoveryKeyReveal", async () => {
    vi.mocked(recoverAccount).mockResolvedValue({ recoveryKey: NEW_KEY });
    const onComplete = vi.fn();
    render(<RecoverScreen onComplete={onComplete} onCancel={vi.fn()} />);

    fillForm();
    submit();
    await screen.findByText(NEW_KEY);

    expect(onComplete).not.toHaveBeenCalled();
    const continueButton = screen.getByRole("button", { name: /continue/i });
    expect(continueButton).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: /saved my recovery key/i }));
    fireEvent.click(continueButton);
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("shows a GENERIC message on auth.bad_recovery_key — never hints whether the user exists", async () => {
    vi.mocked(recoverAccount).mockRejectedValue({ code: "auth.bad_recovery_key", message: "nope" });
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm();
    submit();

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("That username and recovery key don't match.");
    expect(alert.textContent).not.toMatch(/unknown user|no such user|does not exist/i);
  });

  it("shows the minimum-length message on auth.weak_password", async () => {
    vi.mocked(recoverAccount).mockRejectedValue({ code: "auth.weak_password", message: "too short" });
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm({ password: "short", confirm: "short" });
    submit();

    expect(await screen.findByRole("alert")).toHaveTextContent("at least 10 characters");
  });

  it("shows a back-off message on auth.too_many_attempts", async () => {
    vi.mocked(recoverAccount).mockRejectedValue({ code: "auth.too_many_attempts", message: "slow down" });
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm();
    submit();

    expect(await screen.findByRole("alert")).toHaveTextContent(/too many attempts/i);
  });

  it("falls back to the server message for unexpected failures", async () => {
    vi.mocked(recoverAccount).mockRejectedValue({ code: "rpc.transport", message: "HTTP 502 with non-JSON body" });
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm();
    submit();

    expect(await screen.findByRole("alert")).toHaveTextContent("HTTP 502 with non-JSON body");
  });

  it("blocks submission when the new passwords don't match (never calls the API)", () => {
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm({ confirm: "somethingelse" });
    submit();

    expect(screen.getByRole("alert")).toHaveTextContent(/don't match/i);
    expect(recoverAccount).not.toHaveBeenCalled();
  });

  it("blocks submission when a field is empty (never calls the API)", () => {
    render(<RecoverScreen onComplete={vi.fn()} onCancel={vi.fn()} />);

    fillForm({ key: "" });
    submit();

    expect(screen.getByRole("alert")).toHaveTextContent(/every field/i);
    expect(recoverAccount).not.toHaveBeenCalled();
  });

  it("offers a way back to sign in", () => {
    const onCancel = vi.fn();
    render(<RecoverScreen onComplete={vi.fn()} onCancel={onCancel} />);

    fireEvent.click(screen.getByRole("button", { name: /back to sign in/i }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});

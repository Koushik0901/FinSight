import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import SetupScreen from "./SetupScreen";
import { setup } from "../../api/auth";

vi.mock("../../api/auth", () => ({
  setup: vi.fn(),
}));

const RECOVERY_KEY = "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-11111111-22222222";

function fillForm(username: string, password: string, confirm: string) {
  fireEvent.change(screen.getByLabelText(/^username$/i), { target: { value: username } });
  fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: password } });
  fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: confirm } });
}

describe("SetupScreen", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("submits username/password and reveals the recovery key on success", async () => {
    vi.mocked(setup).mockResolvedValue({ recoveryKey: RECOVERY_KEY });
    render(<SetupScreen onComplete={vi.fn()} />);

    fillForm("koushik", "hunter2hunter2", "hunter2hunter2");
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));

    await waitFor(() => expect(setup).toHaveBeenCalledWith("koushik", "hunter2hunter2"));
    expect(await screen.findByText(RECOVERY_KEY)).toBeInTheDocument();
  });

  it("blocks submission with a local error when passwords don't match (never calls setup)", () => {
    render(<SetupScreen onComplete={vi.fn()} />);
    fillForm("koushik", "hunter2hunter2", "somethingelse");
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));

    expect(screen.getByText(/don't match/i)).toBeInTheDocument();
    expect(setup).not.toHaveBeenCalled();
  });

  it("only calls onComplete after the recovery key is confirmed via RecoveryKeyReveal", async () => {
    vi.mocked(setup).mockResolvedValue({ recoveryKey: RECOVERY_KEY });
    const onComplete = vi.fn();
    render(<SetupScreen onComplete={onComplete} />);

    fillForm("koushik", "hunter2hunter2", "hunter2hunter2");
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));
    await screen.findByText(RECOVERY_KEY);

    expect(onComplete).not.toHaveBeenCalled();
    const continueButton = screen.getByRole("button", { name: /continue/i });
    expect(continueButton).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: /saved my recovery key/i }));
    fireEvent.click(continueButton);
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("shows a friendly message on 409 auth.already_setup", async () => {
    vi.mocked(setup).mockRejectedValue({ code: "auth.already_setup", message: "Setup already completed." });
    render(<SetupScreen onComplete={vi.fn()} />);

    fillForm("koushik", "hunter2hunter2", "hunter2hunter2");
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));

    expect(await screen.findByText(/already/i)).toBeInTheDocument();
  });
});

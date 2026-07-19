import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import LoginScreen from "./LoginScreen";
import { login } from "../../api/auth";

// LoginScreen now renders RecoverScreen in place, which pulls `recoverAccount`
// from this same module — it must exist on the mock or the import throws.
vi.mock("../../api/auth", () => ({
  login: vi.fn(),
  recoverAccount: vi.fn(),
}));

describe("LoginScreen", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("submits username/password and calls onComplete on success", async () => {
    vi.mocked(login).mockResolvedValue(undefined);
    const onComplete = vi.fn();
    render(<LoginScreen onComplete={onComplete} />);

    fireEvent.change(screen.getByLabelText(/username/i), { target: { value: "koushik" } });
    fireEvent.change(screen.getByLabelText(/password/i), { target: { value: "hunter2" } });
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() => expect(login).toHaveBeenCalledWith("koushik", "hunter2"));
    await waitFor(() => expect(onComplete).toHaveBeenCalledTimes(1));
  });

  it("shows 'Wrong username or password.' on auth.bad_credentials and does not call onComplete", async () => {
    vi.mocked(login).mockRejectedValue({ code: "auth.bad_credentials", message: "nope" });
    const onComplete = vi.fn();
    render(<LoginScreen onComplete={onComplete} />);

    fireEvent.change(screen.getByLabelText(/username/i), { target: { value: "koushik" } });
    fireEvent.change(screen.getByLabelText(/password/i), { target: { value: "wrong" } });
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    expect(await screen.findByText("Wrong username or password.")).toBeInTheDocument();
    expect(onComplete).not.toHaveBeenCalled();
  });

  it("shows a generic error message for unexpected failures", async () => {
    vi.mocked(login).mockRejectedValue({ code: "rpc.transport", message: "HTTP 502 with non-JSON body" });
    render(<LoginScreen onComplete={vi.fn()} />);

    fireEvent.change(screen.getByLabelText(/username/i), { target: { value: "koushik" } });
    fireEvent.change(screen.getByLabelText(/password/i), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    expect(await screen.findByText("HTTP 502 with non-JSON body")).toBeInTheDocument();
  });

  it("'Forgot your password?' opens the recovery screen, and it can be backed out of", () => {
    render(<LoginScreen onComplete={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /forgot your password/i }));
    expect(screen.getByRole("button", { name: /^reset password$/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/^recovery key$/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /back to sign in/i }));
    expect(screen.getByRole("button", { name: /^sign in$/i })).toBeInTheDocument();
  });
});

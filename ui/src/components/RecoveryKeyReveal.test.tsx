import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { RecoveryKeyReveal } from "./RecoveryKeyReveal";

const KEY = "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-11111111-22222222";

describe("RecoveryKeyReveal", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders the recovery key in full", () => {
    render(<RecoveryKeyReveal recoveryKey={KEY} onContinue={vi.fn()} />);
    expect(screen.getByText(KEY)).toBeInTheDocument();
  });

  it("copies the key to the clipboard when Copy is clicked", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    render(<RecoveryKeyReveal recoveryKey={KEY} onContinue={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /copy/i }));

    await waitFor(() => expect(writeText).toHaveBeenCalledWith(KEY));
  });

  it("disables Continue until the confirm checkbox is checked, then calls onContinue", () => {
    const onContinue = vi.fn();
    render(<RecoveryKeyReveal recoveryKey={KEY} onContinue={onContinue} />);

    const continueButton = screen.getByRole("button", { name: /continue/i });
    expect(continueButton).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: /saved my recovery key/i }));
    expect(continueButton).toBeEnabled();

    fireEvent.click(continueButton);
    expect(onContinue).toHaveBeenCalledTimes(1);
  });

  it("does not call onContinue if clicked while still disabled", () => {
    const onContinue = vi.fn();
    render(<RecoveryKeyReveal recoveryKey={KEY} onContinue={onContinue} />);
    fireEvent.click(screen.getByRole("button", { name: /continue/i }));
    expect(onContinue).not.toHaveBeenCalled();
  });
});

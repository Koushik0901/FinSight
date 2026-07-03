import React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import DeleteAllDataDialog from "./DeleteAllDataDialog";

const deleteMutate = vi.fn().mockResolvedValue(undefined);
const resetMutate = vi.fn().mockResolvedValue(undefined);
const navigate = vi.fn();

vi.mock("react-router-dom", () => ({
  useNavigate: () => navigate,
}));
vi.mock("react-focus-lock", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("../api/hooks/settings", () => ({
  useDeleteAllData: () => ({ mutateAsync: deleteMutate, isPending: false }),
}));
vi.mock("../api/hooks/onboarding", () => ({
  useResetOnboarding: () => ({ mutateAsync: resetMutate }),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

describe("DeleteAllDataDialog", () => {
  beforeEach(() => {
    deleteMutate.mockClear();
    resetMutate.mockClear();
    navigate.mockClear();
  });

  it("keeps the destructive button disabled until DELETE is typed exactly", () => {
    render(<DeleteAllDataDialog open onClose={() => {}} />);

    const confirmBtn = screen.getByRole("button", { name: /delete all data/i });
    expect(confirmBtn).toBeDisabled();

    const input = screen.getByLabelText(/type delete to confirm/i);
    fireEvent.change(input, { target: { value: "delet" } });
    expect(confirmBtn).toBeDisabled();

    fireEvent.change(input, { target: { value: "DELETE" } });
    expect(confirmBtn).not.toBeDisabled();
  });

  it("does not call delete when the confirmation word is wrong", () => {
    render(<DeleteAllDataDialog open onClose={() => {}} />);
    const input = screen.getByLabelText(/type delete to confirm/i);
    fireEvent.change(input, { target: { value: "nope" } });
    fireEvent.click(screen.getByRole("button", { name: /delete all data/i }));
    expect(deleteMutate).not.toHaveBeenCalled();
  });

  it("deletes, resets onboarding, and navigates to onboarding once confirmed", async () => {
    const onClose = vi.fn();
    render(<DeleteAllDataDialog open onClose={onClose} />);

    fireEvent.change(screen.getByLabelText(/type delete to confirm/i), { target: { value: "DELETE" } });
    fireEvent.click(screen.getByRole("button", { name: /delete all data/i }));

    await waitFor(() => expect(deleteMutate).toHaveBeenCalledTimes(1));
    expect(resetMutate).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(navigate).toHaveBeenCalledWith("/onboarding"));
    expect(onClose).toHaveBeenCalled();
  });
});

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import LiabilityDrawer from "./LiabilityDrawer";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));

const createMutate = vi.fn(() => Promise.resolve());

vi.mock("../api/hooks/assets", () => ({
  useCreateLiability: vi.fn(() => ({ mutateAsync: createMutate, isPending: false })),
  useUpdateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("LiabilityDrawer", () => {
  it("sends null for empty optional limit and APR on create", async () => {
    render(<LiabilityDrawer open onClose={() => {}} />, { wrapper: createWrapper() });

    // Fill in name
    fireEvent.change(screen.getByPlaceholderText("e.g. Mortgage"), {
      target: { value: "Car Loan" },
    });

    // Fill in balance (first spinbutton); leave limit and APR empty
    const [balanceInput] = screen.getAllByRole("spinbutton");
    fireEvent.change(balanceInput as HTMLElement, { target: { value: "12000" } });

    // Submit
    fireEvent.click(screen.getByRole("button", { name: /add liability/i }));

    await waitFor(() => expect(createMutate).toHaveBeenCalled());

    const arg = (createMutate.mock.calls as unknown as [[{ balanceCents: number; limitCents: number | null; aprPct: number | null }]])[0][0];
    expect(arg.balanceCents).toBe(1200000);
    expect(arg.limitCents).toBeNull();
    expect(arg.aprPct).toBeNull();
  });
});

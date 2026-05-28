import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import AccountDrawer from "./AccountDrawer";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/accounts", () => ({
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({}) })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({ id: "a1", name: "Renamed" }) })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));

const existingAccount = {
  id: "a1", owner: "Me", bank: "Chase", type: "Checking" as const,
  name: "Old Name", last4: null, currency: "USD", color: "#fff",
  archived_at: null, created_at: "2024-01-01T00:00:00Z",
};

describe("AccountDrawer — create mode", () => {
  it("shows 'Add account' title and submit button", () => {
    render(<AccountDrawer open={true} onClose={() => {}} />, { wrapper: createWrapper() });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Create account")).toBeInTheDocument();
  });
});

describe("AccountDrawer — edit mode", () => {
  it("shows 'Edit Account' title and pre-filled name", () => {
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByDisplayValue("Old Name")).toBeInTheDocument();
    expect(screen.getByText("Save changes")).toBeInTheDocument();
  });

  it("shows archive button", () => {
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByRole("button", { name: /archive/i })).toBeInTheDocument();
  });

  it("two-click confirm on archive: first click shows confirm text", async () => {
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    fireEvent.click(screen.getByRole("button", { name: /archive account/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /confirm archive/i })).toBeInTheDocument()
    );
  });
});

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
  liquidity_type: "liquid", emergency_fund_eligible: true, goal_earmark: null, apy_pct: null,
  simplefin_account_id: null, last_synced_at: null, nickname: null,
  connection_id: null, institution_id: null, external_account_id: null, official_name: null,
  mask: null, subtype: null, account_group: "cash", available_balance_cents: null,
  balance_date: null, extra_json: null, raw_json: null, import_pending: false,
  apr_pct: null, min_payment_cents: null, payoff_date: null, limit_cents: null,
  original_balance_cents: null, started_at: null, promo_apr_expires_on: null, post_promo_apr_pct: null,
};

describe("AccountDrawer — create mode", () => {
  it("shows 'Add account' title and submit button", () => {
    render(<AccountDrawer open={true} onClose={() => {}} />, { wrapper: createWrapper() });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Create account")).toBeInTheDocument();
  });

  it("uses a generic owner example", () => {
    render(<AccountDrawer open={true} onClose={() => {}} />, { wrapper: createWrapper() });
    expect(screen.getByPlaceholderText("Add a person (e.g. Jane Doe)")).toBeInTheDocument();
  });


  it("shows APY field when Savings is selected", () => {
    render(<AccountDrawer open={true} onClose={() => {}} />, { wrapper: createWrapper() });
    expect(screen.queryByLabelText(/apy/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: /savings/i }));
    expect(screen.getByLabelText(/apy/i)).toBeInTheDocument();
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

  it("shows APY field for savings account in edit mode", () => {
    const savingsAccount = { ...existingAccount, type: "Savings" as const, apy_pct: 4.5 };
    render(
      <AccountDrawer open={true} onClose={() => {}} account={savingsAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByLabelText(/apy/i)).toHaveValue(4.5);
  });

  it("shows APY hint for savings accounts without APY", () => {
    const savingsAccount = { ...existingAccount, type: "Savings" as const, apy_pct: null };
    render(
      <AccountDrawer open={true} onClose={() => {}} account={savingsAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByText(/Add an APY so savings projections/i)).toBeInTheDocument();
  });

  it("shows promotional-rate fields only for debt accounts, pre-filled", () => {
    // A promo is meaningless on a chequing account; the fieldset it lives in
    // is debt-only, so the fields must not leak into other account types.
    render(
      <AccountDrawer open={true} onClose={() => {}} account={existingAccount} />,
      { wrapper: createWrapper() },
    );
    expect(screen.queryByLabelText(/promotional rate ends/i)).not.toBeInTheDocument();

    const card = {
      ...existingAccount,
      type: "Credit" as const,
      apr_pct: 0,
      promo_apr_expires_on: "2026-09-01",
      post_promo_apr_pct: 22.99,
    };
    render(
      <AccountDrawer open={true} onClose={() => {}} account={card} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByLabelText(/promotional rate ends/i)).toHaveValue("2026-09-01");
    expect(screen.getByLabelText(/rate after the promo/i)).toHaveValue(22.99);
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

import { beforeEach, describe, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import StepAccounts from "./StepAccounts";
import StepHistory from "./StepHistory";

const useAccounts = vi.hoisted(() => vi.fn());
const useTransactions = vi.hoisted(() => vi.fn());

vi.mock("../../api/hooks/accounts", () => ({ useAccounts }));
vi.mock("../../api/hooks/transactions", () => ({ useTransactions }));
vi.mock("../../components/AccountDrawer", () => ({
  default: ({ open }: { open: boolean }) => open ? <div role="dialog" aria-label="Account editor" /> : null,
}));
vi.mock("./SimpleFinDialog", () => ({
  default: ({ open }: { open: boolean }) => open ? <div role="dialog" aria-label="SimpleFIN setup" /> : null,
}));
vi.mock("../../components/FilePicker", () => ({
  default: ({ label, onPicked }: { label: string; onPicked: (path: string) => void }) => (
    <button type="button" onClick={() => onPicked("C:\\statement.csv")}>{label}</button>
  ),
}));
vi.mock("../../components/ImportMappingDialog", () => ({
  default: ({ path, defaultAccountId }: { path: string; defaultAccountId: string }) => (
    <div role="dialog" aria-label="CSV mapping">{defaultAccountId}:{path}</div>
  ),
}));

const manualAccount = {
  id: "manual-1",
  bank: "Local Credit Union",
  name: "Everyday Checking",
  nickname: null,
  type: "Checking",
  color: "#38BDF8",
  last4: "1234",
  simplefin_account_id: null,
  last_synced_at: null,
};

const simpleFinAccount = {
  id: "simplefin-1",
  bank: "Connected Bank",
  name: "Rainy Day Savings",
  nickname: "Rainy Day",
  type: "Savings",
  color: "#4ADE80",
  last4: "9876",
  simplefin_account_id: "sf-account-1",
  last_synced_at: "2026-07-16T10:00:00Z",
};

describe("Onboarding account-first flow", () => {
  beforeEach(() => {
    useAccounts.mockReturnValue({ data: [], isLoading: false, error: null });
    useTransactions.mockReturnValue({ data: [], isLoading: false, error: null });
  });

  it("keeps account establishment separate from transaction history", () => {
    const onNext = vi.fn();
    render(<StepAccounts onNext={onNext} />);

    expect(screen.getByRole("heading", { name: /start with your accounts/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^\+ add account$/i })).toBeInTheDocument();    expect(screen.getByRole("button", { name: /connect simplefin/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /import csv/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /transaction/i })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^\+ add account$/i }));    expect(screen.getByRole("dialog", { name: /account editor/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /add accounts later/i }));
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("shows both manual and discovered accounts in the roster", () => {
    useAccounts.mockReturnValue({
      data: [manualAccount, simpleFinAccount],
      isLoading: false,
      error: null,
    });
    render(<StepAccounts onNext={() => {}} />);

    expect(screen.getByText("Everyday Checking")).toBeInTheDocument();
    expect(screen.getByText("Rainy Day")).toBeInTheDocument();
    expect(screen.getByText("Manual", { selector: "span" })).toBeInTheDocument();
    expect(screen.getByText("SimpleFIN")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /continue to history/i })).toBeInTheDocument();
  });

  it("offers CSV import only for manual accounts and scopes it to that account", () => {
    useAccounts.mockReturnValue({
      data: [manualAccount, simpleFinAccount],
      isLoading: false,
      error: null,
    });
    useTransactions.mockReturnValue({
      data: [{ id: "txn-1", account_id: "manual-1" }],
      isLoading: false,
      error: null,
    });
    render(<StepHistory onBack={() => {}} onNext={() => {}} />);

    expect(screen.getByText("Connected")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /import another csv/i }));
    expect(screen.getByRole("dialog", { name: /csv mapping/i })).toHaveTextContent(
      "manual-1:C:\\statement.csv",
    );
  });

  it("allows empty history to go back or be deferred", () => {
    const onBack = vi.fn();
    const onNext = vi.fn();
    render(<StepHistory onBack={onBack} onNext={onNext} />);

    fireEvent.click(screen.getByRole("button", { name: /back to accounts/i }));
    expect(onBack).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /do this later/i }));
    expect(onNext).toHaveBeenCalledTimes(1);
  });
});

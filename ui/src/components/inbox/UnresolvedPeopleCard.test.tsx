import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, within, fireEvent } from "@testing-library/react";
import UnresolvedPeopleCard from "./UnresolvedPeopleCard";

const useUnresolvedCounterparties = vi.fn();
const mockMutate = vi.fn();
const useApplyCounterpartyVerdict = vi.fn(() => ({ mutate: mockMutate, isPending: false }));

vi.mock("../../api/hooks/inbox", () => ({
  useUnresolvedCounterparties: () => useUnresolvedCounterparties(),
  useApplyCounterpartyVerdict: () => useApplyCounterpartyVerdict(),
}));

vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

const JOE = { pattern: "%joe%", label: "Joe", txnCount: 12, inflowCents: 300_000, outflowCents: 1_147_500 };
const SWATHI = { pattern: "%swathi%", label: "Swathi", txnCount: 11, inflowCents: 0, outflowCents: 1_936_000 };
const UNNAMED = { pattern: null, label: "Unnamed internal transfers", txnCount: 1, inflowCents: 0, outflowCents: 5_000 };

describe("UnresolvedPeopleCard", () => {
  beforeEach(() => {
    useUnresolvedCounterparties.mockReset();
    useApplyCounterpartyVerdict.mockReset();
    mockMutate.mockReset();
    mockNavigate.mockReset();
    useApplyCounterpartyVerdict.mockReturnValue({ mutate: mockMutate, isPending: false });
    useUnresolvedCounterparties.mockReturnValue({ data: [JOE, SWATHI, UNNAMED], isLoading: false });
  });

  it("shows a header with the group count", () => {
    render(<UnresolvedPeopleCard />);
    expect(screen.getByText("People with unresolved money (3)")).toBeInTheDocument();
  });

  it("renders named groups with net in/out amounts and three verdict buttons each", () => {
    render(<UnresolvedPeopleCard />);

    const joeRow = screen.getByTestId("counterparty-row-%joe%");
    expect(within(joeRow).getByText("Joe")).toBeInTheDocument();
    expect(within(joeRow).getByText(/12 txns/)).toBeInTheDocument();
    const joeOut = within(joeRow).getByText("$11,475");
    const joeIn = within(joeRow).getByText("$3,000");
    expect(joeOut).toHaveClass("money");
    expect(joeIn).toHaveClass("money");
    expect(within(joeRow).getByRole("button", { name: "Transfer" })).toBeInTheDocument();
    expect(within(joeRow).getByRole("button", { name: "Settle-up" })).toBeInTheDocument();
    expect(within(joeRow).getByRole("button", { name: "Real" })).toBeInTheDocument();

    const swathiRow = screen.getByTestId("counterparty-row-%swathi%");
    expect(within(swathiRow).getByText("Swathi")).toBeInTheDocument();
    const swathiOut = within(swathiRow).getByText("$19,360");
    expect(swathiOut).toHaveClass("money");
    // Zero inflow side is not shown for Swathi.
    expect(within(swathiRow).queryByText("$0")).not.toBeInTheDocument();
    expect(within(swathiRow).getByRole("button", { name: "Transfer" })).toBeInTheDocument();
    expect(within(swathiRow).getByRole("button", { name: "Settle-up" })).toBeInTheDocument();
    expect(within(swathiRow).getByRole("button", { name: "Real" })).toBeInTheDocument();
  });

  it("renders the unnamed bucket without verdict buttons but with a Review individually affordance", () => {
    render(<UnresolvedPeopleCard />);

    const unnamedRow = screen.getByTestId("counterparty-row-unnamed");
    expect(within(unnamedRow).getByText("Unnamed internal transfers")).toBeInTheDocument();
    expect(within(unnamedRow).getByText(/1 txn\b/)).toBeInTheDocument();
    expect(within(unnamedRow).getByText("$50")).toHaveClass("money");
    expect(within(unnamedRow).queryByRole("button", { name: "Transfer" })).not.toBeInTheDocument();
    expect(within(unnamedRow).queryByRole("button", { name: "Settle-up" })).not.toBeInTheDocument();
    expect(within(unnamedRow).queryByRole("button", { name: "Real" })).not.toBeInTheDocument();
    expect(within(unnamedRow).getByRole("button", { name: /review individually/i })).toBeInTheDocument();
  });

  it("navigates to the transfer-review ledger when Review individually is clicked", () => {
    render(<UnresolvedPeopleCard />);
    const unnamedRow = screen.getByTestId("counterparty-row-unnamed");
    fireEvent.click(within(unnamedRow).getByRole("button", { name: /review individually/i }));
    expect(mockNavigate).toHaveBeenCalledWith("/transactions?filter=transfer_review");
  });

  it("calls the apply mutation with pattern+verdict when clicking Joe's Settle-up", () => {
    render(<UnresolvedPeopleCard />);
    const joeRow = screen.getByTestId("counterparty-row-%joe%");
    fireEvent.click(within(joeRow).getByRole("button", { name: "Settle-up" }));

    expect(mockMutate).toHaveBeenCalledTimes(1);
    expect(mockMutate.mock.calls[0]?.[0]).toEqual({ pattern: "%joe%", verdict: "settleUp" });
  });

  it("renders nothing when there are no unresolved counterparties", () => {
    useUnresolvedCounterparties.mockReturnValue({ data: [], isLoading: false });
    const { container } = render(<UnresolvedPeopleCard />);
    expect(container).toBeEmptyDOMElement();
  });
});

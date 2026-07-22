import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import MonthClose from "./MonthClose";
import { createWrapperWithEntries } from "../test-utils";
import { useMonthClose } from "../api/hooks/reports";

const mockSave = vi.fn();
vi.mock("../api/hooks/reports", () => ({
  useMonthClose: vi.fn(),
  useSaveMonthClose: vi.fn(() => ({ mutate: mockSave, isPending: false })),
  useMonthCloses: vi.fn(() => ({ data: [] })),
}));

const baseView = {
  year: 2026,
  month: 6,
  monthLabel: "June 2026",
  status: "in_progress",
  notes: null,
  completedAt: null,
  snapshot: {
    incomeCents: 540000,
    expenseCents: 388000,
    savingsCents: 152000,
    savingsRatePct: 28,
    netWorthCents: 7428000,
    debtTotalCents: 124000,
    overBudgetCategories: ["Dining"],
    goalProgress: [],
    subscriptionChangeCount: 2,
  },
  flags: [
    { id: "uncat", category: "review", priority: "high", title: "12 transactions need categorizing", detail: "Uncategorized spending makes the budget review unreliable.", actionRoute: "/transactions", count: 12, acknowledged: false },
  ],
  drift: [],
};

describe("MonthClose — in progress", () => {
  beforeEach(() => {
    mockSave.mockClear();
    vi.mocked(useMonthClose).mockReturnValue({ data: baseView, isLoading: false, error: null } as unknown as ReturnType<typeof useMonthClose>);
  });

  it("renders the review sections and the month's figures", () => {
    render(<MonthClose />, { wrapper: createWrapperWithEntries(["/close?year=2026&month=6"]) });
    expect(screen.getByText(/Close out June 2026/i)).toBeInTheDocument();
    expect(screen.getByText(/Verify the month/i)).toBeInTheDocument();
    expect(screen.getByText("12 transactions need categorizing")).toBeInTheDocument();
    expect(screen.getByText(/The month in numbers/i)).toBeInTheDocument();
    // The subscription-change summary surfaces #58's signal in the close.
    expect(screen.getByText(/2 subscription changes/i)).toBeInTheDocument();
  });

  it("completes the close, recording which flags were acknowledged", () => {
    render(<MonthClose />, { wrapper: createWrapperWithEntries(["/close?year=2026&month=6"]) });
    fireEvent.click(screen.getByRole("checkbox", { name: /Acknowledge: 12 transactions/i }));
    fireEvent.click(screen.getByRole("button", { name: "Complete close" }));
    expect(mockSave).toHaveBeenCalledWith(
      expect.objectContaining({ year: 2026, month: 6, status: "completed", acknowledgedFlagIds: ["uncat"] }),
      expect.anything(),
    );
  });

  it("skips the month via the lifecycle, not by inventing a new path", () => {
    render(<MonthClose />, { wrapper: createWrapperWithEntries(["/close?year=2026&month=6"]) });
    fireEvent.click(screen.getByRole("button", { name: "Skip this month" }));
    expect(mockSave).toHaveBeenCalledWith(
      expect.objectContaining({ status: "skipped" }),
      expect.anything(),
    );
  });
});

describe("MonthClose — completed", () => {
  it("freezes the record: shows drift and offers reopen, not complete", () => {
    vi.mocked(useMonthClose).mockReturnValue({
      data: {
        ...baseView,
        status: "completed",
        completedAt: "2026-07-01T00:00:00Z",
        drift: [{ label: "Spending", recordedCents: 388000, currentCents: 421000, changedMaterially: true }],
      },
      isLoading: false,
      error: null,
    } as unknown as ReturnType<typeof useMonthClose>);

    render(<MonthClose />, { wrapper: createWrapperWithEntries(["/close?year=2026&month=6"]) });
    expect(screen.getByText(/Numbers have drifted/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reopen" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Complete close" })).toBeNull();
  });
});

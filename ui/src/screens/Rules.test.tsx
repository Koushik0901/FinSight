import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Rules from "./Rules";
import { createWrapper } from "../test-utils";

const accept = vi.fn();
const decline = vi.fn();
const createRuleMock = vi.fn().mockResolvedValue({ id: "r99", pattern: "%coffee%", categoryId: "cat-1", source: "user", enabled: true, createdAt: "" });

vi.mock("../api/hooks/transactions", () => ({
  useRulesWithCategories: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useToggleRule: vi.fn(() => ({ mutateAsync: vi.fn() })),
  useCreateRule: vi.fn(() => ({ mutateAsync: createRuleMock, isPending: false })),
  useCategoriesWithSpending: vi.fn(() => ({
    data: [
      { id: "cat-1", label: "Groceries", color: "#4ade80", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 30000, lastMonthCents: 20000, txnCount: 5, yearTotalCents: 300000, budgetCents: 40000 },
    ],
    isLoading: false, error: null,
  })),
}));

vi.mock("../api/hooks/proposals", () => ({
  useRuleProposals: vi.fn(() => ({ data: [
    { id: "p1", whenLabel: "3 corrections for Whole Foods", description: "Always categorize Whole Foods as Groceries", pattern: "%whole foods%", categoryId: "groceries", status: "pending", createdAt: "2026-06-01T00:00:00Z" },
  ] })),
  useAcceptRuleProposal: vi.fn(() => ({ mutateAsync: accept, isPending: false })),
  useDeclineRuleProposal: vi.fn(() => ({ mutateAsync: decline, isPending: false })),
}));

describe("Rules — agent proposals", () => {
  it("renders proposals and accepts one", async () => {
    render(<Rules />, { wrapper: createWrapper() });
    expect(screen.getByText("Agent proposals")).toBeInTheDocument();
    expect(screen.getByText("Always categorize Whole Foods as Groceries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /accept/i }));
    await waitFor(() => expect(accept).toHaveBeenCalledWith("p1"));
  });

  it("declines a proposal", async () => {
    render(<Rules />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /decline/i }));
    await waitFor(() => expect(decline).toHaveBeenCalledWith("p1"));
  });
});

describe("Rules — trust dial copy", () => {
  it("does not overclaim a per-category autonomy control in the trust dial copy", () => {
    render(<Rules />, { wrapper: createWrapper() });
    expect(screen.queryByText(/per category in Settings/i)).not.toBeInTheDocument();
    expect(screen.getByText(/Auto-categorization is controlled in Settings/i)).toBeInTheDocument();
  });
});

describe("Rules — new-rule builder", () => {
  it("opens inline form on New rule button click", () => {
    render(<Rules />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /new rule/i }));
    expect(screen.getByPlaceholderText(/%starbucks%/i)).toBeInTheDocument();
  });

  it("calls createRule when form is submitted", async () => {
    createRuleMock.mockClear();

    render(<Rules />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /new rule/i }));

    const patternInput = screen.getByPlaceholderText(/%starbucks%/i);
    fireEvent.change(patternInput, { target: { value: "coffee" } });

    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "cat-1" } });

    fireEvent.click(screen.getByRole("button", { name: /create rule/i }));
    await waitFor(() => {
      expect(createRuleMock).toHaveBeenCalledWith({
        pattern: "%coffee%",
        categoryId: "cat-1",
      });
    });
  });
});

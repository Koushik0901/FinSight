import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Rules from "./Rules";
import { createWrapper } from "../test-utils";

const accept = vi.fn();

vi.mock("../api/hooks/transactions", () => ({
  useRulesWithCategories: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useToggleRule: vi.fn(() => ({ mutateAsync: vi.fn() })),
}));

vi.mock("../api/hooks/proposals", () => ({
  useRuleProposals: vi.fn(() => ({ data: [
    { id: "p1", whenLabel: "3 corrections for Whole Foods", description: "Always categorize Whole Foods as Groceries", pattern: "%whole foods%", categoryId: "groceries", status: "pending", createdAt: "2026-06-01T00:00:00Z" },
  ] })),
  useAcceptRuleProposal: vi.fn(() => ({ mutateAsync: accept, isPending: false })),
  useDeclineRuleProposal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("Rules — agent proposals", () => {
  it("renders proposals and accepts one", async () => {
    render(<Rules />, { wrapper: createWrapper() });
    expect(screen.getByText("Agent proposals")).toBeInTheDocument();
    expect(screen.getByText("Always categorize Whole Foods as Groceries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /accept/i }));
    await waitFor(() => expect(accept).toHaveBeenCalledWith("p1"));
  });
});

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Insights from "./Insights";
import { createWrapper } from "../test-utils";

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/transactions", () => ({
  useCategoriesWithSpending: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/budget", () => ({
  useBudgetEnvelopes: vi.fn(() => ({ data: [] })),
  useGoals: vi.fn(() => ({ data: [] })),
}));
vi.mock("../api/hooks/agentMemory", () => ({
  useAgentMemory: vi.fn(() => ({ data: [] })),
  useForgetAgentMemory: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));

const triggerMock = vi.fn().mockResolvedValue(undefined);
vi.mock("../api/hooks/agent", () => ({
  useTriggerCategorize: vi.fn(() => ({ mutateAsync: triggerMock, isPending: false })),
  useNeedsReviewCount: vi.fn(() => ({ data: 0 })),
}));

vi.mock("../api/client", () => ({
  commands: {
    getMonthTotals: vi.fn().mockResolvedValue({ status: "ok", data: { incomeCents: 0, expenseCents: 0, netCents: 0, savingsRatePct: 0, txnCount: 0 } }),
    listRecurring: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

describe("Insights — agent operator panel", () => {
  it("renders the agent status bar", () => {
    render(<Insights />, { wrapper: createWrapper() });
    expect(screen.getByText(/Agent · running locally/)).toBeInTheDocument();
  });

  it("Re-run scan button calls triggerCategorize", async () => {
    render(<Insights />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /re-run scan/i }));
    await waitFor(() => {
      expect(triggerMock).toHaveBeenCalled();
    });
  });
});

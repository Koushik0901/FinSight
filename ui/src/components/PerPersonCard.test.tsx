import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import PerPersonCard from "./PerPersonCard";

// Controllable hook mocks so we can exercise the populated (2+ members) path —
// the marquee P0-2 UI, which returns null in every other screen test's DB state.
const useHouseholdMembers = vi.fn();
const useFinancialMetrics = vi.fn();
vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: () => useHouseholdMembers(),
}));
vi.mock("../api/hooks/metrics", () => ({
  useFinancialMetrics: (memberId?: string | null) => useFinancialMetrics(memberId),
}));

const TWO_MEMBERS = {
  data: [
    { id: "m-alice", name: "Alice", color: "#38BDF8", created_at: "2026-01-01" },
    { id: "m-bob", name: "Bob", color: "#F472B6", created_at: "2026-01-02" },
  ],
};

const METRICS = {
  data: {
    thisMonthIncomeCents: 350000,
    thisMonthExpenseCents: 120000,
    thisMonthSavingsRatePct: 65,
    liquidCents: 90000,
  },
};

describe("PerPersonCard", () => {
  beforeEach(() => {
    useHouseholdMembers.mockReset();
    useFinancialMetrics.mockReset();
    useFinancialMetrics.mockReturnValue(METRICS);
  });

  it("renders the switcher and refetches per member (member-keyed)", () => {
    useHouseholdMembers.mockReturnValue(TWO_MEMBERS);
    render(<PerPersonCard currency="CAD" />);

    // Everyone + both members appear as tabs, and the populated cards render.
    expect(screen.getByRole("tab", { name: "Everyone" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Alice/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Bob/ })).toBeInTheDocument();
    expect(screen.getByText("Income (this month)")).toBeInTheDocument();

    // Default selection is the household → metrics fetched with null.
    expect(useFinancialMetrics).toHaveBeenCalledWith(null);

    // Selecting a member re-invokes the metrics hook with that member's id,
    // which is what keys the per-person refetch.
    fireEvent.click(screen.getByRole("tab", { name: /Alice/ }));
    expect(useFinancialMetrics).toHaveBeenCalledWith("m-alice");
  });

  it("renders nothing for a single-person (or empty) household", () => {
    useHouseholdMembers.mockReturnValue({ data: [TWO_MEMBERS.data[0]] });
    const { container } = render(<PerPersonCard currency="CAD" />);
    expect(container).toBeEmptyDOMElement();
  });
});

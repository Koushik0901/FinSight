import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import type { ReactNode } from "react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import Budget from "./Budget";
import * as budgetHooks from "../api/hooks/budget";

/**
 * `?focusCategory=` is the deep link the Copilot uses after applying a budget
 * change, so the user can see the result instead of taking "done" on trust.
 *
 * These tests cover the contract the backend relies on: the link must open the
 * right envelope, tolerate ids from data we have never seen, and never leave a
 * stale parameter behind.
 */

const envelope = (categoryId: string, categoryLabel: string, groupLabel = "Everyday") => ({
  categoryId,
  categoryLabel,
  groupLabel,
  budgetCents: 50000,
  spentCents: 12000,
  carryoverCents: 0,
  rolloverEnabled: false,
  txnCount: 3,
  color: "#27ae60",
});

const mockEnvelopes = vi.fn();

vi.mock("../api/hooks/budget", () => ({
  useBudgetEnvelopes: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useBudgetHistory: vi.fn(() => ({ data: [] })),
  useSetBudget: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  usePlanNextMonthData: vi.fn(() => ({ data: null, isLoading: false })),
  useApplyNextMonthPlan: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useGoals: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useUpdateGoalBalance: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useContributeToGoal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useGoalContributions: vi.fn(() => ({ data: [] })),
  useMemberBudgetEnvelopes: vi.fn(() => ({ data: [] })),
}));

const householdMock = vi.fn(() => ({ data: [] as unknown[] }));
vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: () => householdMock(),
}));

vi.mock("../api/client", () => ({
  commands: {
    getMonthTotals: vi.fn().mockResolvedValue({ status: "error", error: { message: "no data" } }),
    getSpendingBreakdown: vi.fn().mockResolvedValue({
      status: "error",
      error: { message: "no data" },
    }),
  },
}));

/** Surfaces the live URL so we can assert the param gets cleaned up. */
function LocationProbe() {
  const location = useLocation();
  return <span data-testid="url">{`${location.pathname}${location.search}`}</span>;
}

function renderAt(url: string) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <MemoryRouter initialEntries={[url]}>
      <QueryClientProvider client={queryClient}>
        {children}
        <LocationProbe />
      </QueryClientProvider>
    </MemoryRouter>
  );
  return render(<Budget />, { wrapper: Wrapper });
}

beforeEach(() => {
  vi.clearAllMocks();
  mockEnvelopes.mockReturnValue({ data: [], isLoading: false, error: null });
  vi.mocked(budgetHooks.useBudgetEnvelopes).mockImplementation(mockEnvelopes);
  householdMock.mockReturnValue({ data: [] });
});

describe("Budget ?focusCategory deep link", () => {
  it("opens the editor for the linked category", async () => {
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries"), envelope("cat-transit", "Transit")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget?focusCategory=cat-groceries");

    // The editor input only renders for the envelope being edited.
    await waitFor(() => {
      expect(screen.getByRole("spinbutton")).toBeInTheDocument();
    });
  });

  it("strips the parameter once consumed so it cannot re-fire", async () => {
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget?focusCategory=cat-groceries");

    await waitFor(() => {
      expect(screen.getByTestId("url")).toHaveTextContent("/budget");
    });
    expect(screen.getByTestId("url").textContent).not.toContain("focusCategory");
  });

  it("accepts a category label as well as an id", async () => {
    // A link may be built from either; the label match is case-insensitive
    // because category naming is entirely user- and import-defined.
    mockEnvelopes.mockReturnValue({
      data: [envelope("11e4c0de-0000-4000-8000-000000000001", "Groceries")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget?focusCategory=groceries");

    await waitFor(() => {
      expect(screen.getByRole("spinbutton")).toBeInTheDocument();
    });
  });

  it("waits for envelopes to load before deciding the category is missing", async () => {
    // A slow load must not look like "not found" and burn the parameter.
    mockEnvelopes.mockReturnValue({ data: [], isLoading: true, error: null });

    const { rerender } = renderAt("/budget?focusCategory=cat-groceries");

    // Still loading: parameter must survive.
    await waitFor(() => {
      expect(screen.getByTestId("url")).toHaveTextContent("focusCategory=cat-groceries");
    });

    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });
    rerender(<Budget />);

    await waitFor(() => {
      expect(screen.getByRole("spinbutton")).toBeInTheDocument();
    });
  });

  it("degrades quietly when the category no longer exists", async () => {
    // Stale link — e.g. the category was deleted after the Copilot answered.
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-transit", "Transit")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget?focusCategory=cat-deleted");

    // No editor opens, no crash, and the dead parameter is cleared.
    await waitFor(() => {
      expect(screen.getByTestId("url").textContent).not.toContain("focusCategory");
    });
    expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
  });

  it("handles ids that are not URL-safe", async () => {
    // Category ids can originate from imported data, so they are not
    // guaranteed to be UUIDs. This must not throw in CSS.escape or the query
    // selector used for scrolling.
    const awkward = "cat #1 & 2";
    mockEnvelopes.mockReturnValue({
      data: [envelope(awkward, "Odd Category")],
      isLoading: false,
      error: null,
    });

    renderAt(`/budget?focusCategory=${encodeURIComponent(awkward)}`);

    await waitFor(() => {
      expect(screen.getByRole("spinbutton")).toBeInTheDocument();
    });
  });

  it("scrolls the focused envelope into view", async () => {
    const scrollIntoView = vi.fn();
    // jsdom does not implement scrollIntoView, so the component guards it with
    // `?.` — install a spy to prove the call actually happens.
    Element.prototype.scrollIntoView = scrollIntoView;

    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget?focusCategory=cat-groceries");

    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled());
  });

  it("focuses an envelope that has no budget set", async () => {
    // Categories with no budget, no spend and no carryover render in their own
    // "Not yet budgeted" section rather than the main grid. A Copilot action
    // that zeroes a budget lands the category here, so this section has to
    // carry the same focus target as the others — it silently did not, and the
    // editor opened off-screen with no scroll.
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;

    mockEnvelopes.mockReturnValue({
      data: [
        { ...envelope("cat-unused", "Unused"), budgetCents: 0, spentCents: 0, carryoverCents: 0 },
      ],
      isLoading: false,
      error: null,
    });

    renderAt("/budget?focusCategory=cat-unused");

    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled());
  });

  it("gives every envelope a focus target regardless of which section renders it", async () => {
    // The invariant the scroll effect depends on: one, and only one, element
    // per envelope carries `data-envelope-id`. Budget renders envelopes from
    // three separate sections, and adding a fourth without the attribute would
    // silently break deep links into it.
    mockEnvelopes.mockReturnValue({
      data: [
        // Over budget -> "Needs a glance" *and* the main grid.
        { ...envelope("cat-over", "Over"), budgetCents: 10000, spentCents: 99000 },
        // Healthy -> main grid only.
        envelope("cat-ok", "Fine"),
        // Untouched -> "Not yet budgeted" only.
        { ...envelope("cat-new", "New"), budgetCents: 0, spentCents: 0, carryoverCents: 0 },
      ],
      isLoading: false,
      error: null,
    });

    const { container } = renderAt("/budget");

    for (const id of ["cat-over", "cat-ok", "cat-new"]) {
      expect(
        container.querySelector(`[data-envelope-id="${id}"]`),
        `${id} has no element carrying data-envelope-id, so it cannot be scrolled to`,
      ).not.toBeNull();
    }
  });

  it("renders normally with no parameter at all", async () => {
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget");

    await waitFor(() => {
      expect(screen.getByTestId("url")).toHaveTextContent("/budget");
    });
    expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
  });
});

describe("Budget loading state", () => {
  it("shows a skeleton grid rather than collapsing to one line", async () => {
    // A bare "Loading budget…" line makes the page collapse and then snap back
    // into a grid once data lands. The skeleton holds the shape.
    mockEnvelopes.mockReturnValue({ data: [], isLoading: true, error: null });

    const { container } = renderAt("/budget");

    expect(container.querySelectorAll(".skeleton").length).toBeGreaterThan(0);
    expect(container.querySelector(".budget-grid")).not.toBeNull();
  });

  it("announces loading to screen readers, since the skeletons are decoration", async () => {
    mockEnvelopes.mockReturnValue({ data: [], isLoading: true, error: null });

    const { container } = renderAt("/budget");

    // The visual grid is aria-hidden, so the status has to be carried by text
    // that is not on screen.
    expect(screen.getByText("Loading budget…")).toHaveClass("sr-only");
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
    expect(container.querySelector('.budget-grid[aria-hidden="true"]')).not.toBeNull();
  });

  it("still renders the real grid once envelopes arrive", async () => {
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });

    const { container } = renderAt("/budget");

    await waitFor(() => {
      expect(container.querySelectorAll(".skeleton").length).toBe(0);
    });
    expect(container.querySelector("[data-envelope-id]")).not.toBeNull();
  });
});

describe("Budget per-person scope", () => {
  it("shows no member toggle when the household has no members", async () => {
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget");

    await waitFor(() => {
      expect(screen.getByText("Groceries")).toBeInTheDocument();
    });
    // With no household, the budget screen looks exactly as it did before.
    expect(screen.queryByRole("button", { name: "Household" })).not.toBeInTheDocument();
  });

  it("offers a scope toggle per member once a household exists", async () => {
    householdMock.mockReturnValue({
      data: [
        { id: "alice", name: "Alice", color: "#f0f", createdAt: "2026-01-01" },
        { id: "bob", name: "Bob", color: "#0ff", createdAt: "2026-01-01" },
      ],
    });
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-groceries", "Groceries")],
      isLoading: false,
      error: null,
    });

    renderAt("/budget");

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Household" })).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /Alice/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Bob/ })).toBeInTheDocument();
    // Household is the default scope.
    expect(screen.getByRole("button", { name: "Household" })).toHaveAttribute("aria-pressed", "true");
  });

  it("overlays a member's share and keeps the household budget when scoped", async () => {
    householdMock.mockReturnValue({
      data: [{ id: "alice", name: "Alice", color: "#f0f", createdAt: "2026-01-01" }],
    });
    mockEnvelopes.mockReturnValue({
      data: [envelope("cat-dining", "Dining")],
      isLoading: false,
      error: null,
    });
    vi.mocked(budgetHooks.useMemberBudgetEnvelopes).mockReturnValue({
      data: [
        {
          categoryId: "cat-dining",
          categoryLabel: "Dining",
          categoryColor: "#000",
          groupLabel: "Everyday",
          budgetCents: 50000,
          householdSpentCents: 30000,
          memberSpentCents: 15000,
          txnCount: 3,
        },
      ],
    } as unknown as ReturnType<typeof budgetHooks.useMemberBudgetEnvelopes>);

    const { container } = renderAt("/budget");

    fireEvent.click(await screen.findByRole("button", { name: /Alice/ }));

    // The per-envelope overlay line, specifically (the scope note above also
    // says "Alice's share", so match on the overlay's own container).
    await waitFor(() => {
      expect(container.querySelector(".budget-member-share")).not.toBeNull();
    });
    const overlay = container.querySelector(".budget-member-share")!;
    expect(overlay.textContent).toContain("Alice's share");
    // Half of the $300 household spend on the joint account. `money()` renders
    // whole dollars by default.
    expect(overlay.textContent).toContain("$150");
    // The note makes clear the target is still shared.
    expect(screen.getByText(/targets are still the household's/)).toBeInTheDocument();
  });
});

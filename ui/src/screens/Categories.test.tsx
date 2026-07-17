import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Categories from "./Categories";
import { createWrapper } from "../test-utils";
import { useCategoriesWithSpending } from "../api/hooks/transactions";

vi.mock("../api/hooks/transactions", () => ({
  useCategoriesWithSpending: vi.fn(() => ({
    data: [
      { id: "groceries", label: "Groceries", color: "#4ade80", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 30000, lastMonthCents: 50000, txnCount: 5, yearTotalCents: 300000, yearTxnCount: 42, budgetCents: 40000 },
      { id: "c2", label: "Dining Out", color: "#fb923c", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 20000, lastMonthCents: 10000, txnCount: 3, yearTotalCents: 150000, yearTxnCount: 27, budgetCents: 0 },
    ],
    isLoading: false,
    error: null,
  })),
  useSetCategorySpendingType: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useUpdateCategoryColor: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useCreateCategory: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({ id: "new" }), isPending: false })),
  useRenameCategory: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useArchiveCategory: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useSetCategoryGuidance: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
  useCategoryGroups: vi.fn(() => ({
    data: [
      { id: "g1", label: "Food", hint: null, sort_order: 0 },
      { id: "g2", label: "Lifestyle", hint: null, sort_order: 1 },
    ],
    isLoading: false,
    error: null,
  })),
  useCreateCategoryGroup: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue({ id: "g3", label: "New Group", hint: null, sort_order: 2 }), isPending: false })),
  useSetCategoryGroup: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined), isPending: false })),
}));

describe("Categories — empty state", () => {
  it("renders an intentional empty state on a fresh (no-category) database", () => {
    vi.mocked(useCategoriesWithSpending).mockReturnValueOnce({ data: [], isLoading: false, error: null } as unknown as ReturnType<typeof useCategoriesWithSpending>);
    render(<Categories />, { wrapper: createWrapper() });
    expect(screen.getByText("No categories yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Get started/i })).toBeInTheDocument();
  });
});

describe("Categories — AI insight sentence", () => {
  it("shows insight sentence when last-month data exists", () => {
    render(<Categories />, { wrapper: createWrapper() });
    // Groceries: thisMonth=30000, lastMonth=50000 → delta=-20000 (dropped, best gainer)
    // Dining Out: thisMonth=20000, lastMonth=10000 → delta=+10000 (rose, top riser)
    // Both names appear in the table AND the insight, so use getAllByText
    expect(screen.getAllByText(/Groceries/).length).toBeGreaterThan(0);
    expect(screen.getByText(/dropped/)).toBeInTheDocument();
    expect(screen.getAllByText(/Dining Out/).length).toBeGreaterThan(0);
    expect(screen.getByText(/rose/)).toBeInTheDocument();
  });
});

describe("Categories — scope-aware labels", () => {
  it("shows a scope-aware value label and omits the compare column for the year scope", () => {
    render(<Categories />, { wrapper: createWrapper() });

    // Default "month" scope shows a "vs. <prior month>" comparison label (a real
    // calendar month name, not the "vs. average" toolbar button).
    expect(screen.getByText(/^vs\. (January|February|March|April|May|June|July|August|September|October|November|December)/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Year" }));

    // Year scope: value label switches, and there is no honest "compare" dataset,
    // so the vs./compare label and table column should not be rendered.
    expect(screen.getAllByText("Year total").length).toBeGreaterThan(0);
    expect(screen.queryByText(/^vs\. (January|February|March|April|May|June|July|August|September|October|November|December)/)).not.toBeInTheDocument();
    expect(screen.queryByText("vs. This month")).not.toBeInTheDocument();
    expect(screen.queryByRole("columnheader", { name: "This month" })).not.toBeInTheDocument();
  });

  it("labels the average scope honestly as a 2-month average, not a 12-month one", () => {
    render(<Categories />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByRole("button", { name: "vs. average" }));

    expect(screen.getAllByText("2-mo average").length).toBeGreaterThan(0);
    expect(screen.queryByText(/12-mo average/)).not.toBeInTheDocument();
  });

  it("shows the year-scoped transaction count under Year, not the this-month count", () => {
    render(<Categories />, { wrapper: createWrapper() });

    // Default "month" scope shows this-month counts (5, 3).
    expect(screen.getByText("5")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Year" }));

    // Year scope must switch to the real year-scoped counts (42, 27), not stay on month counts.
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText("27")).toBeInTheDocument();
    expect(screen.queryByText("5")).not.toBeInTheDocument();
    expect(screen.queryByText("3")).not.toBeInTheDocument();
  });
});

describe("Categories — management", () => {
  it("opens the new-category input and a per-row Manage panel with guidance", () => {
    render(<Categories />, { wrapper: createWrapper() });

    // New category flow.
    fireEvent.click(screen.getByRole("button", { name: /New category/i }));
    expect(screen.getByPlaceholderText(/Category name/i)).toBeInTheDocument();

    // Per-row Manage reveals rename + guidance + archive controls.
    const manageButtons = screen.getAllByRole("button", { name: /^Manage/i });
    fireEvent.click(manageButtons[0]!);
    expect(screen.getByText(/Categorizer & Copilot guidance/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Save guidance/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Archive category/i })).toBeInTheDocument();
  });

  it("expands the Manage panel directly beneath the clicked row, not at the table bottom", () => {
    render(<Categories />, { wrapper: createWrapper() });

    // Open Manage on the FIRST category (sorted by this-month spend: Groceries
    // 30000 > Dining Out 20000, so Groceries is row one).
    const manageButton = screen.getByRole("button", { name: "Manage Groceries" });
    fireEvent.click(manageButton);

    // The panel row must be the immediate next sibling of Groceries' row.
    const groceriesRow = manageButton.closest("tr")!;
    const panelRow = groceriesRow.nextElementSibling as HTMLElement;
    expect(panelRow).not.toBeNull();
    expect(panelRow.textContent).toContain("Rename");
    // And the OTHER category's row comes after the panel, proving the panel is
    // not appended after all rows.
    const rows = Array.from(groceriesRow.parentElement!.children);
    const panelIndex = rows.indexOf(panelRow);
    const diningRow = screen.getByRole("button", { name: "Manage Dining Out" }).closest("tr")!;
    expect(rows.indexOf(diningRow)).toBeGreaterThan(panelIndex);
  });

  it("lets the user pick a group when creating a category", () => {
    render(<Categories />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /New category/i }));
    expect(screen.getByLabelText("New category's group")).toBeInTheDocument();
  });

  it("shows a Move to group control in the Manage panel, defaulted to the category's current group", () => {
    render(<Categories />, { wrapper: createWrapper() });
    // Manage Groceries — its fixture groupId is "g1" ("Food").
    fireEvent.click(screen.getByRole("button", { name: "Manage Groceries" }));
    const groupSelect = screen.getByLabelText("Group") as HTMLSelectElement;
    expect(groupSelect.value).toBe("g1");
  });
});

describe("Categories — icon tiles", () => {
  it("renders the semantic icon for a known seeded category id", () => {
    render(<Categories />, { wrapper: createWrapper() });
    const groceriesIcon = screen.getByTestId("cat-icon-groceries");
    // Cart icon's distinguishing path data (see ui/src/components/Icons.tsx `Cart`)
    expect(groceriesIcon.innerHTML).toContain("M2.5 3h2l1 8h7");
  });

  it("falls back to the generic tag icon for a category id with no semantic match", () => {
    render(<Categories />, { wrapper: createWrapper() });
    const diningIcon = screen.getByTestId("cat-icon-c2");
    // Tag icon's distinguishing path data (see ui/src/components/Icons.tsx `Tag`)
    expect(diningIcon.innerHTML).toContain("M3 3h5.5L13 7.5 8.5 12 4 7.5z");
  });
});

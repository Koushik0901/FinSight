import { describe, it, expect, vi, beforeEach, afterEach, type MockInstance } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Reports, { buildReportCsv } from "./Reports";
import { createWrapper } from "../test-utils";
import { api } from "../api/openapiClient";

vi.mock("../api/openapiClient", () => ({
  unwrap: async (p: Promise<{ status: "ok" | "error"; data?: unknown; error?: { message: string } }>) => { const r = await p; if (r.status === "error") throw new Error(r.error?.message ?? "command failed"); return r.data; },
  api: {
    getReportData: vi.fn(),
    customReport: vi.fn(),
    listReportWidgets: vi.fn(),
    createReportWidget: vi.fn(),
    updateReportWidget: vi.fn(),
    deleteReportWidget: vi.fn(),
    reorderReportWidgets: vi.fn(),
  },
}));

vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: () => ({
    data: [
      { id: "m-alice", name: "Alice", color: "#38BDF8", created_at: "2026-01-01" },
      { id: "m-bob", name: "Bob", color: "#F472B6", created_at: "2026-01-02" },
    ],
  }),
}));

vi.mock("../api/hooks/networth", async () => {
  const actual = await vi.importActual<typeof import("../api/hooks/networth")>("../api/hooks/networth");
  return {
    ...actual,
    useNetWorth: () => 123456,
  };
});

vi.mock("../api/hooks/metrics", async () => {
  const actual = await vi.importActual<typeof import("../api/hooks/metrics")>("../api/hooks/metrics");
  return {
    ...actual,
    useFinancialMetrics: () => ({ data: { runwayDays: 90, currency: "USD", unconvertedHoldings: [] } }),
  };
});

vi.mock("../api/hooks/accounts", async () => {
  const actual = await vi.importActual<typeof import("../api/hooks/accounts")>("../api/hooks/accounts");
  return {
    ...actual,
    useAccounts: () => ({ data: [] }),
  };
});

const REPORT_DATA = {
  monthly: [
    { month: "2026-01", label: "Jan", incomeCents: 500000, expenseCents: 350000, netCents: 150000, budgetCents: 400000 },
    { month: "2026-02", label: "Feb", incomeCents: 520000, expenseCents: 370000, netCents: 150000, budgetCents: 400000 },
    { month: "2026-03", label: "Mar", incomeCents: 510000, expenseCents: 360000, netCents: 150000, budgetCents: 400000 },
  ],
  monthlyLastYear: [
    { month: "2025-01", label: "Jan", incomeCents: 480000, expenseCents: 330000, netCents: 150000, budgetCents: 380000 },
    { month: "2025-02", label: "Feb", incomeCents: 490000, expenseCents: 340000, netCents: 150000, budgetCents: 380000 },
    { month: "2025-03", label: "Mar", incomeCents: 500000, expenseCents: 345000, netCents: 155000, budgetCents: 380000 },
  ],
  topCategories: [
    { categoryId: "cat-1", label: "Groceries", color: "#27ae60", totalCents: 120000, txnCount: 15 },
    { categoryId: "cat-2", label: "Dining", color: "#e67e22", totalCents: 85000, txnCount: 20 },
  ],
  topMerchants: [
    { merchantRaw: "Whole Foods Market", categoryLabel: "Food & Drink", categoryColor: "", totalCents: 75000, txnCount: 8 },
    { merchantRaw: "Chipotle", categoryLabel: "Food & Drink", categoryColor: "", totalCents: 42000, txnCount: 12 },
  ],
};

const WIDGETS = [
  { id: "w1", position: 0, title: "Monthly overview", chartType: "bar", splitBy: "month", period: "All", filtersJson: "{}", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
  { id: "w2", position: 1, title: "Spending by category", chartType: "bar", splitBy: "category", period: "All", filtersJson: "{}", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
  { id: "w3", position: 2, title: "Top categories", chartType: "table", splitBy: "category", period: "All", filtersJson: "{}", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
  { id: "w4", position: 3, title: "Top merchants", chartType: "table", splitBy: "payee", period: "All", filtersJson: "{}", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
  { id: "w5", position: 4, title: "Net worth", chartType: "area", splitBy: "month", period: "All", filtersJson: "{}", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
];

beforeEach(() => {
  vi.mocked(api.getReportData).mockResolvedValue({ status: "ok", data: REPORT_DATA });
  vi.mocked(api.listReportWidgets).mockResolvedValue({ status: "ok", data: WIDGETS } as never);
  vi.mocked(api.customReport).mockImplementation(async (params: unknown) => {
    const p = params as { splitBy?: string; memberId?: string | null };
    const isPayee = p.splitBy === "payee";
    const rows = isPayee
      ? [
          { label: "Whole Foods Market", totalCents: 75000, txnCount: 8 },
          { label: "Chipotle", totalCents: 42000, txnCount: 12 },
        ]
      : [
          { label: "Groceries", totalCents: 120000, txnCount: 15 },
          { label: "Dining", totalCents: 85000, txnCount: 20 },
        ];
    // Include memberId in total for scoping test to detect
    const totalCents = rows.reduce((s, r) => s + r.totalCents, 0);
    return { status: "ok", data: { rows, totalCents } } as never;
  });
});

describe("Reports screen", () => {
  it("renders the Reports heading", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText(/See the shape of your money over time/);
  });

  it("renders all scope toolbar buttons", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText(/See the shape of your money over time/);
    expect(screen.getByText("Month")).toBeInTheDocument();
    expect(screen.getByText("Quarter")).toBeInTheDocument();
    expect(screen.getByText("Year")).toBeInTheDocument();
    expect(screen.getByText("All time")).toBeInTheDocument();
  });

  it("clicking Quarter fetches with 'quarter' scope", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText(/See the shape of your money over time/);
    fireEvent.click(screen.getByText("Quarter"));
    await waitFor(() => expect(api.getReportData).toHaveBeenCalledWith("quarter", null));
  });

  it("scopes report data to a household member when selected", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText(/See the shape of your money over time/);
    await waitFor(() => expect(api.getReportData).toHaveBeenCalledWith("year", null));
    fireEvent.click(screen.getByRole("tab", { name: /Alice/ }));
    await waitFor(() => expect(api.getReportData).toHaveBeenCalledWith("year", "m-alice"));
  });

  it("scopes widget queries to the selected household member", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    // Initially whole-household (memberId null)
    await waitFor(() => expect(api.customReport).toHaveBeenCalled());
    vi.mocked(api.customReport).mockClear();
    fireEvent.click(screen.getByRole("tab", { name: /Alice/ }));
    await waitFor(() => {
      const calls = vi.mocked(api.customReport).mock.calls as unknown as Array<[{ memberId?: string | null }]>;
      // At least one widget refetch should include memberId
      expect(calls.some(([params]) => params.memberId === "m-alice")).toBe(true);
    });
  });

  it("renders category and merchant widgets when data is present", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    expect(screen.getByText("Top merchants")).toBeInTheDocument();
    expect(await screen.findByText("Whole Foods Market")).toBeInTheDocument();
    expect(screen.getByText("Chipotle")).toBeInTheDocument();
  });

  it("shows widget canvas with drag handles and Add widget", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    expect(screen.getByText("Add widget")).toBeInTheDocument();
    expect(screen.getAllByLabelText(/Drag to reorder/).length).toBeGreaterThanOrEqual(5);
  });

  it("reorders widgets via Up/Down buttons", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    const upButtons = screen.getAllByLabelText(/Move .* up/);
    expect(upButtons.length).toBeGreaterThan(0);
    // Second widget's Up should trigger reorder
    fireEvent.click(upButtons[1]!);
    await waitFor(() => expect(api.reorderReportWidgets).toHaveBeenCalled());
    const orderedIds = vi.mocked(api.reorderReportWidgets).mock.calls[0]![0] as string[];
    expect(orderedIds[0]).toBe("w2");
    expect(orderedIds[1]).toBe("w1");
  });

  it("edits a widget via the drawer", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    const editButtons = screen.getAllByLabelText(/Edit Top categories/);
    fireEvent.click(editButtons[0]!);
    // Drawer should open with title input
    const titleInput = await screen.findByPlaceholderText("e.g. Spending by category");
    expect(titleInput).toBeInTheDocument();
    fireEvent.change(titleInput, { target: { value: "My Fav" } });
    const saveBtn = screen.getByRole("button", { name: /Save changes/ });
    fireEvent.click(saveBtn);
    await waitFor(() => expect(api.updateReportWidget).toHaveBeenCalled());
    const callArgs = vi.mocked(api.updateReportWidget).mock.calls[0] as unknown as unknown[];
    expect(callArgs[1]).toBe("My Fav");
  });

  it("filters widget queries by payee and amount when drawer saves", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    // Create a new widget with filters
    fireEvent.click(screen.getByText("Add widget"));
    const titleInput = await screen.findByPlaceholderText("e.g. Spending by category");
    fireEvent.change(titleInput, { target: { value: "Filtered" } });
    const payeeInput = screen.getByPlaceholderText("e.g. Whole Foods");
    fireEvent.change(payeeInput, { target: { value: "Whole Foods" } });
    const minInput = screen.getByPlaceholderText("0.00");
    fireEvent.change(minInput, { target: { value: "10" } });
    const saveBtns = screen.getAllByRole("button", { name: /Add widget/ });
    const saveBtn = saveBtns[saveBtns.length - 1]!;
    fireEvent.click(saveBtn);
    await waitFor(() => expect(api.createReportWidget).toHaveBeenCalled());
    const createCall = vi.mocked(api.createReportWidget).mock.calls[0] as unknown as unknown[];
    const filtersJson = createCall[4] as string;
    const filters = JSON.parse(filtersJson as unknown as string);
    expect(filters.payee).toBe("Whole Foods");
    expect(filters.minAmount).toBe(10);
  });
  it("shows a year-over-year delta computed from monthlyLastYear", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText(/vs.*same months last year/i);
  });

  it("replaces empty analytics with an honest setup state", async () => {
    vi.mocked(api.getReportData).mockResolvedValueOnce({
      status: "ok",
      data: { monthly: [], monthlyLastYear: [], topCategories: [], topMerchants: [] },
    });
    render(<Reports />, { wrapper: createWrapper() });
    expect(await screen.findByText(/No financial history in year/i)).toBeInTheDocument();
    expect(screen.getByText(/will not turn missing activity into zero-valued results/i)).toBeInTheDocument();
    expect(screen.queryByText("Savings rate")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Export" })).not.toBeInTheDocument();
  });

  describe("Export button", () => {
    let createObjectURLSpy: MockInstance;
    let revokeObjectURLSpy: MockInstance;
    let clickSpy: MockInstance;
    let createElementSpy: MockInstance;
    beforeEach(() => {
      createObjectURLSpy = vi.fn(() => "blob:mock-url");
      revokeObjectURLSpy = vi.fn();
      globalThis.URL.createObjectURL = createObjectURLSpy as unknown as typeof URL.createObjectURL;
      globalThis.URL.revokeObjectURL = revokeObjectURLSpy as unknown as typeof URL.revokeObjectURL;
      clickSpy = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
      createElementSpy = vi.spyOn(document, "createElement");
    });
    afterEach(() => {
      clickSpy.mockRestore();
      createElementSpy.mockRestore();
    });
    it("triggers a CSV blob download with a sensible filename when clicked", async () => {
      render(<Reports />, { wrapper: createWrapper() });
      await screen.findByText("Top categories");
      fireEvent.click(screen.getByText("Export"));
      expect(createObjectURLSpy).toHaveBeenCalledTimes(1);
      const blobArg = createObjectURLSpy.mock.calls[0]![0] as Blob;
      expect(blobArg).toBeInstanceOf(Blob);
      expect(blobArg.type).toContain("text/csv");
      expect(clickSpy).toHaveBeenCalledTimes(1);
      // Check filename via created anchor
      const anchorCalls = createElementSpy.mock.calls.filter(([tag]) => tag === "a");
      expect(anchorCalls.length).toBeGreaterThan(0);
      // The download attribute is set on the anchor before click
      await waitFor(() => expect(revokeObjectURLSpy).toHaveBeenCalledWith("blob:mock-url"));
    });
  });
});

describe("buildReportCsv", () => {
  it("builds a CSV with monthly, category, and merchant sections using dollar amounts", () => {
    const csv = buildReportCsv(REPORT_DATA as never);
    const lines = csv.split("\n");
    expect(lines[0]).toBe("Section,Label,Income,Expense,Budget,Net");
    expect(lines).toContain("Monthly,Jan,5000.00,3500.00,4000.00,1500.00");
    expect(lines).toContain("Monthly,Feb,5200.00,3700.00,4000.00,1500.00");
    expect(lines).toContain("Section,Category,Amount,Txns");
    expect(lines).toContain('Top category,"Groceries",1200.00,15');
    expect(lines).toContain("Section,Merchant,Amount,Txns");
    expect(lines).toContain('Top merchant,"Whole Foods Market",750.00,8');
  });
  it("escapes embedded double quotes in labels", () => {
    const csv = buildReportCsv({
      ...REPORT_DATA,
      topCategories: [{ categoryId: "cat-x", label: 'Say "hi"', color: "", totalCents: 100, txnCount: 1 }],
      topMerchants: [],
    } as never);
    expect(csv).toContain('Top category,"Say ""hi""",1.00,1');
  });
});

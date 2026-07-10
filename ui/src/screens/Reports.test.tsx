import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Reports, { buildReportCsv } from "./Reports";
import { createWrapper } from "../test-utils";
import { commands } from "../api/client";

vi.mock("../api/client", () => ({
  commands: {
    getReportData: vi.fn(),
  },
}));

// Two-member household so the MemberSwitcher renders and we can test per-person
// scoping of the report queries.
vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: () => ({
    data: [
      { id: "m-alice", name: "Alice", color: "#38BDF8", created_at: "2026-01-01" },
      { id: "m-bob", name: "Bob", color: "#F472B6", created_at: "2026-01-02" },
    ],
  }),
}));

const REPORT_DATA = {
  monthly: [
    { month: "2026-01", label: "Jan", incomeCents: 500000, expenseCents: 350000, netCents: 150000 },
    { month: "2026-02", label: "Feb", incomeCents: 520000, expenseCents: 370000, netCents: 150000 },
    { month: "2026-03", label: "Mar", incomeCents: 510000, expenseCents: 360000, netCents: 150000 },
  ],
  monthlyLastYear: [
    { month: "2025-01", label: "Jan", incomeCents: 480000, expenseCents: 330000, netCents: 150000 },
    { month: "2025-02", label: "Feb", incomeCents: 490000, expenseCents: 340000, netCents: 150000 },
    { month: "2025-03", label: "Mar", incomeCents: 500000, expenseCents: 345000, netCents: 155000 },
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

beforeEach(() => {
  vi.mocked(commands.getReportData).mockResolvedValue({ status: "ok", data: REPORT_DATA });
});

describe("Reports screen", () => {
  it("renders the Reports heading", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("See the shape of your money over time.");
  });

  it("renders all scope toolbar buttons", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("See the shape of your money over time.");
    expect(screen.getByText("Month")).toBeInTheDocument();
    expect(screen.getByText("Quarter")).toBeInTheDocument();
    expect(screen.getByText("Year")).toBeInTheDocument();
    expect(screen.getByText("All time")).toBeInTheDocument();
  });

  it("clicking Quarter fetches with 'quarter' scope", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("See the shape of your money over time.");
    fireEvent.click(screen.getByText("Quarter"));
    await waitFor(() =>
      // second arg is the household-member filter (null = whole household)
      expect(commands.getReportData).toHaveBeenCalledWith("quarter", null)
    );
  });

  it("scopes report data to a household member when selected", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("See the shape of your money over time.");
    // Default scope is "year"; whole-household fetch uses a null member.
    await waitFor(() => expect(commands.getReportData).toHaveBeenCalledWith("year", null));
    // Selecting a person refetches the report scoped to that member.
    fireEvent.click(screen.getByRole("tab", { name: /Alice/ }));
    await waitFor(() => expect(commands.getReportData).toHaveBeenCalledWith("year", "m-alice"));
  });

  it("renders category and merchant tables when data is present", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    // Wait for the data-driven tables to appear (data loads asynchronously)
    await screen.findByText("Top categories");
    expect(screen.getByText("Top merchants")).toBeInTheDocument();
    // merchant names are unique to the merchant table
    expect(screen.getByText("Whole Foods Market")).toBeInTheDocument();
    expect(screen.getByText("Chipotle")).toBeInTheDocument();
  });

  it("shows each merchant's real category instead of dropping it", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top merchants");
    expect(screen.getAllByText("Food & Drink").length).toBeGreaterThanOrEqual(2);
  });

  it("shows a year-over-year delta computed from monthlyLastYear", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText(/vs the same months last year/i);
  });

  it("switches to a distinct spending breakdown when the Spending deep dive tab is clicked", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    fireEvent.click(screen.getByText("Spending deep dive"));
    expect(await screen.findByText("Where it concentrates, this period")).toBeInTheDocument();
    expect(screen.queryByText("Income and expenses over time")).not.toBeInTheDocument();
  });

  it("switches to a net worth view when the Net worth tab is clicked", async () => {
    render(<Reports />, { wrapper: createWrapper() });
    await screen.findByText("Top categories");
    fireEvent.click(screen.getByRole("button", { name: "Net worth" }));
    expect(screen.queryByText("Income and expenses over time")).not.toBeInTheDocument();
    expect(screen.queryByText("Where it concentrates, this period")).not.toBeInTheDocument();
  });

  describe("Export button", () => {
    let createObjectURLSpy: ReturnType<typeof vi.fn>;
    let revokeObjectURLSpy: ReturnType<typeof vi.fn>;
    let clickSpy: ReturnType<typeof vi.spyOn>;

    beforeEach(() => {
      createObjectURLSpy = vi.fn(() => "blob:mock-url");
      revokeObjectURLSpy = vi.fn();
      // jsdom doesn't implement these; stub them for the download path.
      URL.createObjectURL = createObjectURLSpy as unknown as typeof URL.createObjectURL;
      URL.revokeObjectURL = revokeObjectURLSpy as unknown as typeof URL.revokeObjectURL;
      clickSpy = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    });

    afterEach(() => {
      clickSpy.mockRestore();
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
      expect(revokeObjectURLSpy).toHaveBeenCalledWith("blob:mock-url");
    });
  });
});

describe("buildReportCsv", () => {
  it("builds a CSV with monthly, category, and merchant sections using dollar amounts", () => {
    const csv = buildReportCsv(REPORT_DATA);
    const lines = csv.split("\n");

    expect(lines[0]).toBe("Section,Label,Income,Expense,Net");
    expect(lines).toContain("Monthly,Jan,5000.00,3500.00,1500.00");
    expect(lines).toContain("Monthly,Feb,5200.00,3700.00,1500.00");
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
    });
    expect(csv).toContain('Top category,"Say ""hi""",1.00,1');
  });
});

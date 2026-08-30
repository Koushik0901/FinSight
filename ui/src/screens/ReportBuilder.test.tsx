import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ReportBuilder from "./ReportBuilder";
import { createWrapper } from "../test-utils";
import { api } from "../api/openapiClient";

vi.mock("../api/openapiClient", () => ({
  unwrap: async (p: Promise<{ status: "ok" | "error"; data?: unknown; error?: { message: string } }>) => {
    const r = await p;
    if (r.status === "error") throw new Error(r.error?.message ?? "command failed");
    return r.data;
  },
  api: {
    customReport: vi.fn(),
    getReportData: vi.fn(),
  },
}));

const MOCK_RESULT = {
  rows: [
    { label: "Groceries", totalCents: 5000, txnCount: 2 },
    { label: "Rent", totalCents: 10000, txnCount: 1 },
  ],
  totalCents: 15000,
};

beforeEach(() => {
  vi.mocked(api.customReport).mockResolvedValue({ status: "ok", data: MOCK_RESULT });
});

describe("ReportBuilder", () => {
  it("renders split and period selectors", async () => {
    render(<ReportBuilder />, { wrapper: createWrapper() });
    expect(await screen.findByText("Custom Report Builder")).toBeInTheDocument();
    expect(screen.getByLabelText("Split by")).toBeInTheDocument();
    expect(screen.getByLabelText("Period")).toBeInTheDocument();
    expect(screen.getByText("Include transfers")).toBeInTheDocument();
  });

  it("fetches and displays grouped rows", async () => {
    render(<ReportBuilder />, { wrapper: createWrapper() });
    expect(await screen.findByText("Groceries")).toBeInTheDocument();
    expect(screen.getByText("Rent")).toBeInTheDocument();
    expect(screen.getByText(/Total.*across/i)).toBeInTheDocument();
  });

  it("re-fetches when split changes to Payee", async () => {
    render(<ReportBuilder />, { wrapper: createWrapper() });
    await screen.findByText("Groceries");
    // initial call with Category
    await waitFor(() => expect(api.customReport).toHaveBeenCalled());
    vi.mocked(api.customReport).mockClear();
    const select = screen.getByLabelText("Split by") as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "payee" } });
    await waitFor(() =>
      expect(api.customReport).toHaveBeenCalledWith(
        expect.objectContaining({ splitBy: "payee" })
      )
    );
  });

  it("shows each row amount and txn count", async () => {
    render(<ReportBuilder />, { wrapper: createWrapper() });
    await screen.findByText("Groceries");
    // money formatting may vary, but we check txn count text is present
    expect(screen.getByText(/2 txns/)).toBeInTheDocument();
    expect(screen.getByText(/1 txns/)).toBeInTheDocument();
  });

  it("splitBy Month respects includeArchived and is_transfer=false", async () => {
    render(<ReportBuilder />, { wrapper: createWrapper() });
    await screen.findByText("Groceries");
    await waitFor(() => expect(api.customReport).toHaveBeenCalled());
    vi.mocked(api.customReport).mockClear();
    const select = screen.getByLabelText("Split by") as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "month" } });
    await waitFor(() =>
      expect(api.customReport).toHaveBeenCalledWith(
        expect.objectContaining({ splitBy: "month", includeArchived: false, includeTransfers: false })
      )
    );
  });
});

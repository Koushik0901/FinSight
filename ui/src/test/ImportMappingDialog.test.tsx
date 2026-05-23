import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import ImportMappingDialog from "../screens/onboarding/ImportMappingDialog";

vi.mock("react-focus-lock", () => ({
  default: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("../api/client", () => ({
  commands: {
    previewCsvColumns: vi.fn().mockResolvedValue({
      status: "ok",
      data: {
        headers: ["Date", "Merchant", "Amount"],
        rows: [["2026-05-19", "Safeway", "-8.42"]],
        detected_delimiter: ",",
        total_rows: 1,
        encoding_note: null,
      },
    }),
    listAccounts: vi.fn().mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "a1",
          bank: "Chase",
          name: "Checking",
          type: "Checking",
          owner: "joint",
          currency: "USD",
          color: "#000",
          balance_cents: 0,
        },
      ],
    }),
    importCsv: vi.fn().mockResolvedValue({
      status: "ok",
      data: { import_id: "imp1", rows_imported: 1, rows_skipped_duplicates: 0, errors: [] },
    }),
  },
}));

function renderDialog() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ImportMappingDialog path="/tmp/x.csv" onClose={() => {}} onImported={() => {}} />
    </QueryClientProvider>
  );
}

describe("ImportMappingDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("Import button starts disabled until required columns and account assigned", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());
    const btn = screen.getByRole("button", { name: /^import$/i });
    expect(btn).toBeDisabled();
  });

  it("resets column mapping when skipHeaderRows changes column count", async () => {
    const { commands } = await import("../api/client");
    // First call returns 3 columns (already set by default mock)
    // Second call returns 2 columns
    (commands.previewCsvColumns as ReturnType<typeof vi.fn>)
      .mockResolvedValueOnce({
        status: "ok",
        data: {
          headers: ["Date", "Merchant", "Amount"],
          rows: [["2026-05-19", "Safeway", "-8.42"]],
          detected_delimiter: ",",
          total_rows: 1,
          encoding_note: null,
        },
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: {
          headers: ["Date", "Amount"],
          rows: [["2026-05-19", "-8.42"]],
          detected_delimiter: ",",
          total_rows: 1,
          encoding_note: null,
        },
      });

    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    // Change skipHeaderRows to trigger a re-fetch
    const skipInput = screen.getByRole("spinbutton");
    fireEvent.change(skipInput, { target: { value: "2" } });

    // Wait for the new preview to render (2 columns, not 3)
    await waitFor(() => {
      const headers = screen.getAllByRole("columnheader");
      expect(headers).toHaveLength(2);
    });

    // The column dropdowns should reset to Skip (no sparse array)
    const headers = screen.getAllByRole("columnheader");
    const dropdowns = headers.map((h) => h.querySelector("select") as HTMLSelectElement);
    expect(dropdowns).toHaveLength(2);
    expect(dropdowns[0]!.value).toBe("Skip");
    expect(dropdowns[1]!.value).toBe("Skip");
  });

  it("becomes enabled once required mapping is complete and submits", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    // Select account
    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), {
      target: { value: "a1" },
    });

    // Map column roles via the table header dropdowns
    const headers = screen.getAllByRole("columnheader");
    const dropdowns = headers.map((h) => h.querySelector("select")!);
    const [dd0, dd1, dd2] = dropdowns;
    fireEvent.change(dd0!, { target: { value: "Date" } });
    fireEvent.change(dd1!, { target: { value: "Merchant" } });
    fireEvent.change(dd2!, { target: { value: "Amount" } });

    const btn = screen.getByRole("button", { name: /^import$/i });
    await waitFor(() => expect(btn).not.toBeDisabled());

    fireEvent.click(btn);

    const { commands } = await import("../api/client");
    await waitFor(() => {
      expect(commands.importCsv).toHaveBeenCalledWith(
        "/tmp/x.csv",
        "a1",
        expect.objectContaining({
          columns: expect.arrayContaining(["Date", "Merchant", "Amount"]),
        })
      );
    });
  });
});

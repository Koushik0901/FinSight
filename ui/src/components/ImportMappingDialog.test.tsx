import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import ImportMappingDialog from "./ImportMappingDialog";

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
        {
          id: "amex1",
          bank: "Amex",
          name: "Amex Card",
          type: "Credit",
          owner: "joint",
          currency: "USD",
          color: "#000",
          balance_cents: 0,
        },
      ],
    }),
    importCsv: vi.fn().mockResolvedValue({
      status: "ok",
      data: { import_id: "imp1", rows_imported: 1, rows_skipped_duplicates: 0, rows_queued_for_review: 0, errors: [] },
    }),
  },
}));

function renderDialog() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={qc}>
        <ImportMappingDialog path="/tmp/x.csv" onClose={() => {}} onImported={() => {}} />
      </QueryClientProvider>
    </MemoryRouter>
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

  it("auto-detects column roles when preview loads", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    const headers = screen.getAllByRole("columnheader");
    const dropdowns = headers.map((h) => h.querySelector("select") as HTMLSelectElement);
    expect(dropdowns).toHaveLength(3);
    expect(dropdowns[0]!.value).toBe("Date");
    expect(dropdowns[1]!.value).toBe("Merchant");
    expect(dropdowns[2]!.value).toBe("Amount");
  });

  it("defaults amount convention to positive-is-outflow for a credit account", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    // A bank/asset account keeps the standard negative-is-outflow default.
    const conv = () =>
      screen.getAllByRole("radio").reduce<Record<string, boolean>>((acc, el) => {
        acc[(el as HTMLInputElement).value] = (el as HTMLInputElement).checked;
        return acc;
      }, {});
    expect(conv()["negative_is_outflow"]).toBe(true);

    // Selecting a Credit account flips the default to positive-is-outflow,
    // since credit-card exports use positive = a charge (outflow).
    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), {
      target: { value: "amex1" },
    });
    await waitFor(() => expect(conv()["positive_is_outflow"]).toBe(true));
    expect(conv()["negative_is_outflow"]).toBe(false);
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

  it("navigates to /import-review when rows are queued for review", async () => {
    const { commands } = await import("../api/client");
    (commands.importCsv as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      status: "ok",
      data: { import_id: "imp1", rows_imported: 0, rows_skipped_duplicates: 0, rows_queued_for_review: 3, errors: [] },
    });

    render(
      <MemoryRouter initialEntries={["/accounts/a1"]}>
        <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
          <Routes>
            <Route path="/accounts/:id" element={<ImportMappingDialog path="/tmp/x.csv" onClose={() => {}} onImported={() => {}} />} />
            <Route path="/import-review" element={<div data-testid="review-screen">Review Screen</div>} />
          </Routes>
        </QueryClientProvider>
      </MemoryRouter>
    );

    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), {
      target: { value: "a1" },
    });

    const headers = screen.getAllByRole("columnheader");
    const dropdowns = headers.map((h) => h.querySelector("select")!);
    const [dd0, dd1, dd2] = dropdowns;
    fireEvent.change(dd0!, { target: { value: "Date" } });
    fireEvent.change(dd1!, { target: { value: "Merchant" } });
    fireEvent.change(dd2!, { target: { value: "Amount" } });

    const btn = screen.getByRole("button", { name: /^import$/i });
    await waitFor(() => expect(btn).not.toBeDisabled());
    fireEvent.click(btn);

    await waitFor(() => expect(screen.getByTestId("review-screen")).toBeInTheDocument());
  });
});

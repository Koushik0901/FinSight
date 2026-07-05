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
    getSavedCsvMapping: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    createAccount: vi.fn().mockResolvedValue({ status: "ok", data: { id: "new1" } }),
    prepareCsvImport: vi.fn().mockResolvedValue({
      status: "ok",
      data: {
        signature: "sig",
        rowsTotal: 0,
        rowsImported: 0,
        rowsSkippedDuplicates: 0,
        rowsQueuedForReview: 0,
        errors: [],
      },
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

  it("defaults to negative-is-outflow and auto-checks Flip for a credit account", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    // A bank/asset account keeps the standard default: Flip is unchecked.
    const flip = () => screen.getByRole("checkbox", { name: /flip amounts/i }) as HTMLInputElement;
    expect(flip().checked).toBe(false);

    // Selecting a Credit account auto-checks Flip (charges are positive there).
    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), {
      target: { value: "amex1" },
    });
    await waitFor(() => expect(flip().checked).toBe(true));
  });

  it("Flip amounts sends positive_is_outflow to the backend", async () => {
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), { target: { value: "a1" } });
    const headers = screen.getAllByRole("columnheader");
    const [dd0, dd1, dd2] = headers.map((h) => h.querySelector("select")!);
    fireEvent.change(dd0!, { target: { value: "Date" } });
    fireEvent.change(dd1!, { target: { value: "Merchant" } });
    fireEvent.change(dd2!, { target: { value: "Amount" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /flip amounts/i }));

    const btn = screen.getByRole("button", { name: /^import$/i });
    await waitFor(() => expect(btn).not.toBeDisabled());
    fireEvent.click(btn);

    const { commands } = await import("../api/client");
    await waitFor(() => {
      expect(commands.importCsv).toHaveBeenCalledWith(
        "/tmp/x.csv",
        "a1",
        expect.objectContaining({ amount_convention: "positive_is_outflow" }),
      );
    });
  });

  it("pre-fills the amount handling from this account's saved mapping", async () => {
    const { commands } = await import("../api/client");
    (commands.getSavedCsvMapping as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      status: "ok",
      data: {
        skip_header_rows: 1,
        columns: ["Date", "Merchant", "Amount"],
        date_format: "%Y-%m-%d",
        amount_convention: "positive_is_outflow",
        decimal_separator: ".",
        delimiter: null,
      },
    });
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    // A bank account would normally default Flip off; the saved mapping wins.
    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), { target: { value: "a1" } });
    const flip = () => screen.getByRole("checkbox", { name: /flip amounts/i }) as HTMLInputElement;
    await waitFor(() => expect(flip().checked).toBe(true));
    expect(screen.getByText(/settings from your last import/i)).toBeInTheDocument();
  });

  it("lets you create an account inline and imports into the new account", async () => {
    // Regression: on first run (no accounts) the picker was a dead-end. It must
    // offer inline account creation and import into the freshly-made account.
    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    // Choose the inline-create option → the Add account drawer opens.
    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), {
      target: { value: "__new__" },
    });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /create account/i })).toBeInTheDocument(),
    );
    fireEvent.change(screen.getByLabelText(/Bank/i), { target: { value: "Amex" } });
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Card" } });
    fireEvent.click(screen.getByRole("button", { name: /create account/i }));

    // Map the required columns, then import — into the new account id.
    const headers = screen.getAllByRole("columnheader");
    const [dd0, dd1, dd2] = headers.map((h) => h.querySelector("select")!);
    fireEvent.change(dd0!, { target: { value: "Date" } });
    fireEvent.change(dd1!, { target: { value: "Merchant" } });
    fireEvent.change(dd2!, { target: { value: "Amount" } });

    const btn = screen.getByRole("button", { name: /^import$/i });
    await waitFor(() => expect(btn).not.toBeDisabled());
    fireEvent.click(btn);

    const { commands } = await import("../api/client");
    await waitFor(() => {
      expect(commands.importCsv).toHaveBeenCalledWith("/tmp/x.csv", "new1", expect.anything());
    });
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

  it("shows the live prepared outcome in the footer once mapping is complete", async () => {
    const { commands } = await import("../api/client");
    (commands.prepareCsvImport as ReturnType<typeof vi.fn>).mockResolvedValue({
      status: "ok",
      data: {
        signature: "sig",
        rowsTotal: 3,
        rowsImported: 2,
        rowsSkippedDuplicates: 1,
        rowsQueuedForReview: 0,
        errors: [],
      },
    });

    renderDialog();
    await waitFor(() => expect(screen.getByText("Safeway")).toBeInTheDocument());

    fireEvent.change(screen.getByRole("combobox", { name: /account/i }), {
      target: { value: "a1" },
    });

    const headers = screen.getAllByRole("columnheader");
    const [dd0, dd1, dd2] = headers.map((h) => h.querySelector("select")!);
    fireEvent.change(dd0!, { target: { value: "Date" } });
    fireEvent.change(dd1!, { target: { value: "Merchant" } });
    fireEvent.change(dd2!, { target: { value: "Amount" } });

    const btn = screen.getByRole("button", { name: /^import$/i });
    await waitFor(() => expect(btn).not.toBeDisabled());

    await waitFor(
      () => expect(screen.getByText(/2 new/)).toBeInTheDocument(),
      { timeout: 2000 },
    );
    expect(screen.getByText(/1 duplicate/)).toBeInTheDocument();
    expect(commands.prepareCsvImport).toHaveBeenCalledWith(
      "/tmp/x.csv",
      "a1",
      expect.objectContaining({ columns: expect.arrayContaining(["Date", "Merchant", "Amount"]) }),
    );
  });
});

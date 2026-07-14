import { describe, it, expect } from "vitest";
import {
  detectColumnRoles,
  detectDateFormat,
  detectAmountConvention,
  detectHeaderRow,
  buildDetectedMapping,
} from "./csvDetection";
import type { CsvPreview } from "../api/client";

function preview(headers: string[], rows: string[][]): CsvPreview {
  return {
    headers,
    rows,
    detected_delimiter: ",",
    total_rows: rows.length,
    encoding_note: null,
  };
}

describe("detectColumnRoles", () => {
  it("maps AMEX headers", () => {
    const roles = detectColumnRoles(["Date", "Date Processed", "Description", "Amount"]);
    expect(roles).toEqual(["Date", "Skip", "Merchant", "Amount"]);
  });

  it("maps Chase-style headers", () => {
    const roles = detectColumnRoles([
      "Transaction Date",
      "Post Date",
      "Description",
      "Category",
      "Type",
      "Amount",
      "Memo",
    ]);
    expect(roles).toEqual(["Date", "Skip", "Merchant", "Category", "Skip", "Amount", "Notes"]);
  });

  it("maps split debit/credit headers", () => {
    const roles = detectColumnRoles(["Date", "Description", "Debit", "Credit"]);
    expect(roles).toEqual(["Date", "Merchant", "Debit", "Credit"]);
  });

  it("maps Payee header as Merchant", () => {
    const roles = detectColumnRoles(["Date", "Payee", "Amount"]);
    expect(roles).toEqual(["Date", "Merchant", "Amount"]);
  });

  it("leaves unknown columns as Skip", () => {
    const roles = detectColumnRoles(["Col A", "Col B", "Col C"]);
    expect(roles).toEqual(["Skip", "Skip", "Skip"]);
  });

  it("prefers exact match over substring", () => {
    const roles = detectColumnRoles(["Date", "Amount", "Description", "OtherDescription"]);
    expect(roles[0]).toBe("Date");
    expect(roles[1]).toBe("Amount");
    expect(roles[2]).toBe("Merchant");
    expect(roles[3]).toBe("Skip");
  });
});

describe("detectColumnRoles — investment (brokerage) exports", () => {
  const WEALTHSIMPLE_HEADERS = [
    "transaction_date",
    "settlement_date",
    "account_id",
    "account_type",
    "activity_type",
    "activity_sub_type",
    "direction",
    "symbol",
    "name",
    "currency",
    "quantity",
    "unit_price",
    "commission",
    "net_cash_amount",
  ];

  it("maps the Wealthsimple TFSA export, moving the name column to SecurityName", () => {
    const roles = detectColumnRoles(WEALTHSIMPLE_HEADERS);
    expect(roles).toEqual([
      "Date",
      "Skip", // settlement_date
      "Skip", // account_id
      "Skip", // account_type
      "ActivityType",
      "ActivitySubType",
      "Skip", // direction
      "Symbol",
      "SecurityName", // `name` is the security name, empty on non-trade rows
      "Skip", // currency
      "Quantity",
      "UnitPrice",
      "Skip", // commission
      "Amount", // net_cash_amount
    ]);
  });

  it("reverts investment roles when there is no symbol column (bank CSV with a Units column)", () => {
    // A lone "Units" or "Type" header must not turn a bank export into a
    // brokerage import — investment roles require BOTH activity and symbol.
    const roles = detectColumnRoles(["Date", "Description", "Units", "Amount"]);
    expect(roles).toEqual(["Date", "Merchant", "Skip", "Amount"]);
  });

  it("keeps detecting existing bank header sets identically (regression)", () => {
    expect(
      detectColumnRoles([
        "Transaction Date",
        "Post Date",
        "Description",
        "Category",
        "Type",
        "Amount",
        "Memo",
      ]),
    ).toEqual(["Date", "Skip", "Merchant", "Category", "Skip", "Amount", "Notes"]);
    expect(detectColumnRoles(["Date", "Description", "Debit", "Credit"])).toEqual([
      "Date",
      "Merchant",
      "Debit",
      "Credit",
    ]);
  });

  it("flags investmentDetected on the full mapping", () => {
    const p = preview(WEALTHSIMPLE_HEADERS, [
      [
        "2025-01-01", "2025-01-01", "WS0000000CAD", "TFSA", "Trade", "BUY", "LONG",
        "ACME", "Acme Corp", "CAD", "8.1234", "15.0876", "0", "-122.6",
      ],
      [
        "2025-01-05", "", "WS0000000CAD", "TFSA", "MoneyMovement", "EFT", "", "", "",
        "CAD", "200", "", "", "200",
      ],
    ]);
    const detected = buildDetectedMapping(p);
    expect(detected.investmentDetected).toBe(true);
    expect(detected.skipHeaderRows).toBe(1);
    expect(detected.dateFormat).toBe("%Y-%m-%d");
    expect(detected.amountConvention).toBe("negative_is_outflow");
  });

  it("does not flag investmentDetected for bank files", () => {
    const p = preview(
      ["Date", "Description", "Amount"],
      [["2026-05-19", "Store", "-8.42"]],
    );
    expect(buildDetectedMapping(p).investmentDetected).toBe(false);
  });
});

describe("detectDateFormat", () => {
  it("detects AMEX format", () => {
    expect(detectDateFormat(["01 Jul 2026", "30 Jun 2026"])).toBe("%d %b %Y");
  });

  it("detects ISO format", () => {
    expect(detectDateFormat(["2026-05-19", "2026-05-20"])).toBe("%Y-%m-%d");
  });

  it("detects US slash format", () => {
    expect(detectDateFormat(["5/19/2026", "5/20/2026"])).toBe("%m/%d/%Y");
  });

  it("detects long month format", () => {
    expect(detectDateFormat(["January 19, 2026", "January 20, 2026"])).toBe("%B %d, %Y");
  });

  it("detects short month with comma", () => {
    expect(detectDateFormat(["Jul 01, 2026", "Jun 30, 2026"])).toBe("%b %d, %Y");
  });

  it("returns null for non-date strings", () => {
    expect(detectDateFormat(["not a date", "also not"])).toBeNull();
  });

  it("returns null when fewer than 50% match", () => {
    expect(detectDateFormat(["2026-05-19", "hello", "world"])).toBeNull();
  });
});

describe("detectAmountConvention", () => {
  it("picks split debit/credit when both columns exist", () => {
    const convention = detectAmountConvention(
      ["Date", "Merchant", "Debit", "Credit"],
      [["2026-01-01", "x", "10.00", ""]],
    );
    expect(convention).toBe("split_debit_credit");
  });

  it("does not infer positive-is-outflow from a mostly-positive sample", () => {
    // Regression test: a savings-account-style file where deposits/interest
    // (positive) vastly outnumber withdrawals (negative) by row count must
    // NOT be treated as evidence that positive means outflow — the proportion
    // of positive vs. negative rows reflects account activity, not sign
    // convention. Mirrors the real tangerine-savings-all-time-statement.csv
    // sample (21 positive deposits/interest rows vs. 6 negative withdrawals).
    const convention = detectAmountConvention(
      ["Date", "Merchant", "Amount"],
      [
        ["2026-01-01", "x", "10.00"],
        ["2026-01-02", "y", "5.00"],
        ["2026-01-03", "z", "20.00"],
        ["2026-01-04", "a", "15.00"],
        ["2026-01-05", "b", "8.00"],
        ["2026-01-06", "c", "3.00"],
        ["2026-01-07", "d", "12.00"],
        ["2026-01-08", "e", "7.00"],
        ["2026-01-09", "f", "-50.00"],
        ["2026-01-10", "g", "-25.00"],
      ],
    );
    expect(convention).toBe("negative_is_outflow");
  });

  it("defaults to negative-is-outflow when mixed", () => {
    const convention = detectAmountConvention(
      ["Date", "Merchant", "Amount"],
      [
        ["2026-01-01", "x", "10.00"],
        ["2026-01-02", "y", "-10.00"],
      ],
    );
    expect(convention).toBe("negative_is_outflow");
  });

  it("defaults to negative-is-outflow when all negative", () => {
    const convention = detectAmountConvention(
      ["Date", "Merchant", "Amount"],
      [
        ["2026-01-01", "x", "-10.00"],
        ["2026-01-02", "y", "-20.00"],
      ],
    );
    expect(convention).toBe("negative_is_outflow");
  });

  it("defaults to negative-is-outflow when no Amount column", () => {
    const convention = detectAmountConvention(
      ["Date", "Merchant", "Notes"],
      [["2026-01-01", "x", "note"]],
    );
    expect(convention).toBe("negative_is_outflow");
  });
});

describe("detectHeaderRow", () => {
  it("returns 1 when first row looks like headers", () => {
    const rows = [
      ["Date", "Description", "Amount"],
      ["2026-05-19", "Store", "-8.42"],
    ];
    expect(detectHeaderRow(rows)).toBe(1);
  });

  it("returns 0 when first row looks like data", () => {
    const rows = [
      ["2026-05-19", "Store", "-8.42"],
      ["2026-05-20", "Store", "-9.00"],
    ];
    expect(detectHeaderRow(rows)).toBe(0);
  });

  it("returns 1 for single row", () => {
    expect(detectHeaderRow([["Date", "Amount"]])).toBe(1);
  });
});

describe("buildDetectedMapping", () => {
  it("detects a real header row even though preview.rows already excludes it", () => {
    // Mirrors what the backend actually returns for the default initial fetch:
    // skip_header_rows=1 is requested, so it captures row 0 into `headers` and
    // `rows` only contains data starting from row 1 (AMEX-style export).
    const amexPreview = preview(
      ["Date", "Date Processed", "Description", "Amount"],
      [
        ["01 Jul 2026", "01 Jul 2026", "PAYMENT RECEIVED - THANK YOU", "-2986.14"],
        ["30 Jun 2026", "30 Jun 2026", "ANOMALY SAN FRANCISCO", "31.35"],
      ],
    );
    const detected = buildDetectedMapping(amexPreview);
    expect(detected.skipHeaderRows).toBe(1);
    expect(detected.columns).toEqual(["Date", "Skip", "Merchant", "Amount"]);
  });

  it("does not double-skip data rows when computing date format / amount convention", () => {
    const preview3Rows = preview(
      ["Date", "Merchant", "Amount"],
      [
        ["2026-05-19", "Store A", "-8.42"],
        ["2026-05-20", "Store B", "-9.00"],
      ],
    );
    const detected = buildDetectedMapping(preview3Rows);
    // Both data rows must still be considered — a double-skip would drop row 0
    // and leave only one data point to infer the date format / amount convention from.
    expect(detected.dateFormat).toBe("%Y-%m-%d");
    expect(detected.amountConvention).toBe("negative_is_outflow");
  });
});

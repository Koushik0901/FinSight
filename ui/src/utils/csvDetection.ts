import type { CsvPreview, ColumnRole, AmountConvention } from "../api/client";

export interface DetectedMapping {
  skipHeaderRows: number;
  columns: ColumnRole[];
  dateFormat: string | null;
  amountConvention: AmountConvention;
  detectedFields: Set<string>;
  /** True when the file looks like a brokerage export (activity + symbol columns). */
  investmentDetected: boolean;
}

const ROLE_KEYWORDS: Record<ColumnRole, string[]> = {
  Date: ["date", "transactiondate", "postdate", "posteddate", "dateposted"],
  Amount: ["amount", "transactionamount", "amountusd"],
  Merchant: ["description", "merchant", "payee", "transactiondescription", "name", "details"],
  Notes: ["notes", "memo", "reference", "tag"],
  Category: ["category"],
  Debit: ["debit", "debitamount", "withdrawal", "withdrawals"],
  Credit: ["credit", "creditamount", "deposit", "deposits"],
  // Investment (brokerage export) roles. Only kept when the file ALSO has a
  // symbol column (see finalizeInvestmentRoles) so a bank CSV with a generic
  // "Transaction Type" column is never misread as a brokerage export.
  ActivityType: ["activitytype", "activity", "transactiontype", "action"],
  ActivitySubType: ["activitysubtype", "subtype"],
  Symbol: ["symbol", "ticker", "tickersymbol"],
  // Never greedy-matched ("name" belongs to Merchant); assigned only by the
  // brokerage post-pass, which moves an empty-prone name column here.
  SecurityName: [],
  Quantity: ["quantity", "shares", "units"],
  UnitPrice: ["unitprice", "price", "unitcost"],
  Skip: [],
};

const INVESTMENT_ROLES: ColumnRole[] = [
  "ActivityType",
  "ActivitySubType",
  "Symbol",
  "Quantity",
  "UnitPrice",
];

const DATE_FORMATS = [
  "%Y-%m-%d",
  "%m/%d/%Y",
  "%d/%m/%Y",
  "%d.%m.%Y",
  "%d-%b-%Y",
  "%d %b %Y",
  "%b %d, %Y",
  "%B %d, %Y",
];

function normalizeHeader(header: string): string {
  return header.toLowerCase().replace(/[^a-z0-9]/g, "");
}

const DATE_REGEXES: Record<string, RegExp> = {
  "%Y-%m-%d": /^\d{4}-\d{2}-\d{2}$/,
  "%m/%d/%Y": /^\d{1,2}\/\d{1,2}\/\d{4}$/,
  "%d/%m/%Y": /^\d{1,2}\/\d{1,2}\/\d{4}$/,
  "%d.%m.%Y": /^\d{1,2}\.\d{1,2}\.\d{4}$/,
  "%d-%b-%Y": /^\d{1,2}-[A-Za-z]{3}-\d{4}$/,
  "%d %b %Y": /^\d{1,2} [A-Za-z]{3} \d{4}$/,
  "%b %d, %Y": /^[A-Za-z]{3} \d{1,2}, \d{4}$/,
  "%B %d, %Y": /^[A-Za-z]{4,} \d{1,2}, \d{4}$/,
};

function matchesDateFormat(value: string, fmt: string): boolean {
  return DATE_REGEXES[fmt]?.test(value.trim()) ?? false;
}

export function detectColumnRoles(headers: string[]): ColumnRole[] {
  const normalized = headers.map(normalizeHeader);
  const assigned = new Set<number>();
  const roles: ColumnRole[] = Array(headers.length).fill("Skip");

  // Investment roles come AFTER every bank role so files without brokerage
  // columns detect byte-identically to before they existed.
  const priorityOrder: ColumnRole[] = [
    "Date",
    "Merchant",
    "Amount",
    "Debit",
    "Credit",
    "Category",
    "Notes",
    "ActivityType",
    "ActivitySubType",
    "Symbol",
    "Quantity",
    "UnitPrice",
  ];

  for (const role of priorityOrder) {
    let bestIdx = -1;
    let bestScore = 0;

    for (let i = 0; i < normalized.length; i++) {
      if (assigned.has(i)) continue;
      const h = normalized[i] ?? "";
      const keywords = ROLE_KEYWORDS[role];

      for (const kw of keywords) {
        if (h === kw) {
          bestIdx = i;
          bestScore = 3;
          break;
        }
        if (h.includes(kw) && kw.length > bestScore) {
          bestIdx = i;
          bestScore = kw.length;
        }
      }
      if (bestIdx >= 0 && bestScore === 3) break;
    }

    if (bestIdx >= 0 && bestScore >= 3) {
      roles[bestIdx] = role;
      assigned.add(bestIdx);
    }
  }

  return finalizeInvestmentRoles(roles, normalized);
}

/// Brokerage post-pass. A real brokerage export has BOTH an activity column
/// and a symbol column; anything less (a bank CSV with a lone "Transaction
/// Type" or "Units" header) reverts its investment roles to Skip. When it IS
/// a brokerage export, a Merchant match on a literal `name` header is the
/// security name (empty on non-trade rows), so it moves to SecurityName and
/// merchants get synthesized from the activity instead.
function finalizeInvestmentRoles(roles: ColumnRole[], normalized: string[]): ColumnRole[] {
  const isBrokerage = roles.includes("ActivityType") && roles.includes("Symbol");
  if (!isBrokerage) {
    return roles.map((r) => (INVESTMENT_ROLES.includes(r) ? "Skip" : r));
  }
  return roles.map((r, i) => (r === "Merchant" && normalized[i] === "name" ? "SecurityName" : r));
}

export function detectDateFormat(values: string[]): string | null {
  const nonEmpty = values.filter((v) => v.trim().length > 0);
  if (nonEmpty.length === 0) return null;

  let bestFormat: string | null = null;
  let bestCount = 0;

  for (const fmt of DATE_FORMATS) {
    let count = 0;
    for (const v of nonEmpty) {
      if (matchesDateFormat(v, fmt)) count++;
    }
    if (count > bestCount) {
      bestCount = count;
      bestFormat = fmt;
    }
  }

  return bestCount / nonEmpty.length >= 0.5 ? bestFormat : null;
}

export function detectAmountConvention(
  roles: ColumnRole[],
  rows: string[][],
): AmountConvention {
  if (roles.includes("Debit") && roles.includes("Credit")) {
    return "split_debit_credit";
  }

  // Negative-is-outflow is the standard convention used by the overwhelming
  // majority of real bank/card CSV exports, and is always assumed here. The
  // proportion of positive vs. negative rows in a preview sample reflects the
  // account's activity mix (e.g. a savings account naturally has far more
  // deposits than withdrawals), not which sign the exporter uses for outflows,
  // so it is not a reliable signal for guessing "positive_is_outflow" and is
  // deliberately not used to auto-select it. Manual override remains available.
  return "negative_is_outflow";
}

export function detectHeaderRow(rows: string[][]): number {
  if (rows.length < 2) return 1;
  const first = rows[0] ?? [];
  const second = rows[1] ?? [];

  const firstLooksLikeHeader = first.some((cell) =>
    Object.values(ROLE_KEYWORDS)
      .flat()
      .some((kw) => normalizeHeader(cell).includes(kw)),
  );

  const secondLooksLikeData = second.some((cell) => {
    const isDate = DATE_FORMATS.some((fmt) => matchesDateFormat(cell, fmt));
    const isNumber = /^-?[\d$€£,.()\s]+$/.test(cell.trim());
    return isDate || isNumber;
  });

  if (firstLooksLikeHeader && secondLooksLikeData) return 1;
  if (secondLooksLikeData) return 0;
  return 1;
}

export function buildDetectedMapping(preview: CsvPreview): DetectedMapping {
  // preview.headers/preview.rows are already relative to whatever skip_header_rows
  // was used for this specific fetch (the backend only populates `headers` when the
  // caller requested skip > 0, and `rows` excludes those skipped rows). Reconstruct
  // the raw, unskipped row order here so header detection sees the real first line
  // of the file instead of data that's already had a header stripped from it.
  const rawRows = preview.headers ? [preview.headers, ...preview.rows] : preview.rows;
  const skipHeaderRows = detectHeaderRow(rawRows);
  const headers = rawRows[0] ?? [];
  const dataRows = skipHeaderRows > 0 ? rawRows.slice(skipHeaderRows) : rawRows;
  const columns = detectColumnRoles(headers);

  const dateIdx = columns.indexOf("Date");
  const dateFormat = dateIdx >= 0 ? detectDateFormat(dataRows.map((r) => r[dateIdx] ?? "")) : null;

  const amountConvention = detectAmountConvention(columns, dataRows);

  const detectedFields = new Set<string>();
  if (columns.some((r) => r !== "Skip")) detectedFields.add("columns");
  if (dateFormat) detectedFields.add("dateFormat");
  detectedFields.add("amountConvention");
  if (skipHeaderRows !== 1) detectedFields.add("skipHeaderRows");

  const investmentDetected = columns.includes("ActivityType") && columns.includes("Symbol");

  return {
    skipHeaderRows,
    columns,
    dateFormat,
    amountConvention,
    detectedFields,
    investmentDetected,
  };
}

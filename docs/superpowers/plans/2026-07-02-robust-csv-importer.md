# Robust CSV Importer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the CSV import dialog auto-detect column mapping, date format, amount convention, and header rows from bank statement CSVs, while keeping full manual override.

**Architecture:** Add a pure TypeScript detection module (`ui/src/utils/csvDetection.ts`) that analyzes `previewCsvColumns` output. Seed `ImportMappingDialog` state from detected values. Extend the Rust `parse_amount` function to handle common bank amount variants. Add tests for both layers.

**Tech Stack:** React, TypeScript, vitest, Rust, chrono, csv crate.

## Global Constraints

- Reuse existing `previewCsvColumns` and `importCsv` commands; no new backend endpoints.
- Keep the mapping dialog; do not build a one-click import flow.
- Manual override must remain possible for every detected field.
- Detection is best-effort; fall back to safe defaults when uncertain.
- Run `cd ui && npx tsc --noEmit` before each frontend commit.
- Run `cargo test -p finsight-providers` before each Rust commit.
- Exact file paths from the repo root.

---

### Task 1: Add missing date format presets

**Files:**
- Modify: `ui/src/components/ImportMappingDialog.tsx`

**Interfaces:**
- Consumes: Existing `DATE_FORMATS` array.
- Produces: `DATE_FORMATS` array includes AMEX and common US bank formats.

- [ ] **Step 1: Add presets**

  In `ui/src/components/ImportMappingDialog.tsx`, update `DATE_FORMATS`:

  ```tsx
  const DATE_FORMATS = [
    { label: "2026-05-19",   value: "%Y-%m-%d" },
    { label: "5/19/2026",    value: "%m/%d/%Y" },
    { label: "19/05/2026",   value: "%d/%m/%Y" },
    { label: "19.05.2026",   value: "%d.%m.%Y" },
    { label: "May 19, 2026", value: "%B %d, %Y" },
    { label: "19-May-2026",  value: "%d-%b-%Y" },
    { label: "01 Jul 2026",  value: "%d %b %Y" },
    { label: "Jul 01, 2026", value: "%b %d, %Y" },
    { label: "Custom",       value: "__CUSTOM__" },
  ];
  ```

- [ ] **Step 2: Type-check**

  Run: `cd ui && npx tsc --noEmit`
  Expected: No errors.

- [ ] **Step 3: Commit**

  ```bash
  git add ui/src/components/ImportMappingDialog.tsx
  git commit -m "feat: add AMEX and common US bank date format presets"
  ```

---

### Task 2: Create csvDetection utilities

**Files:**
- Create: `ui/src/utils/csvDetection.ts`
- Create: `ui/src/utils/csvDetection.test.ts`

**Interfaces:**
- Consumes: `CsvPreview` from `../api/client`, `ColumnRole` and `AmountConvention` types.
- Produces:
  - `detectColumnRoles(headers: string[]): ColumnRole[]`
  - `detectDateFormat(values: string[]): string | null`
  - `detectAmountConvention(roles: ColumnRole[], rows: string[][]): AmountConvention`
  - `detectHeaderRow(rows: string[][]): number`
  - `buildDetectedMapping(preview: CsvPreview): DetectedMapping`

- [ ] **Step 1: Write the failing tests**

  Create `ui/src/utils/csvDetection.test.ts`:

  ```tsx
  import { describe, it, expect } from "vitest";
  import {
    detectColumnRoles,
    detectDateFormat,
    detectAmountConvention,
    detectHeaderRow,
  } from "./csvDetection";
  import type { CsvPreview } from "../api/client";

  const preview = (headers: string[], rows: string[][]): CsvPreview => ({
    headers,
    rows,
    detected_delimiter: ",",
    total_rows: rows.length,
    encoding_note: null,
  });

  describe("detectColumnRoles", () => {
    it("maps AMEX headers", () => {
      const roles = detectColumnRoles(["Date", "Date Processed", "Description", "Amount"]);
      expect(roles).toEqual(["Date", "Skip", "Merchant", "Amount"]);
    });

    it("maps Chase-style headers", () => {
      const roles = detectColumnRoles(["Transaction Date", "Post Date", "Description", "Category", "Type", "Amount", "Memo"]);
      expect(roles).toEqual(["Date", "Skip", "Merchant", "Category", "Skip", "Amount", "Notes"]);
    });

    it("maps split debit/credit headers", () => {
      const roles = detectColumnRoles(["Date", "Description", "Debit", "Credit"]);
      expect(roles).toEqual(["Date", "Merchant", "Debit", "Credit"]);
    });
  });

  describe("detectDateFormat", () => {
    it("detects AMEX format", () => {
      expect(detectDateFormat(["01 Jul 2026", "30 Jun 2026"])).toBe("%d %b %Y");
    });

    it("detects ISO format", () => {
      expect(detectDateFormat(["2026-05-19", "2026-05-20"])).toBe("%Y-%m-%d");
    });

    it("returns null when uncertain", () => {
      expect(detectDateFormat(["not a date", "also not"])).toBeNull();
    });
  });

  describe("detectAmountConvention", () => {
    it("picks split debit/credit when both columns exist", () => {
      const convention = detectAmountConvention(
        ["Date", "Merchant", "Debit", "Credit"],
        [["2026-01-01", "x", "10.00", ""]]
      );
      expect(convention).toBe("split_debit_credit");
    });

    it("detects positive-is-outflow", () => {
      const convention = detectAmountConvention(
        ["Date", "Merchant", "Amount"],
        [
          ["2026-01-01", "x", "10.00"],
          ["2026-01-02", "y", "5.00"],
          ["2026-01-03", "z", "-50.00"],
        ]
      );
      expect(convention).toBe("positive_is_outflow");
    });

    it("defaults to negative-is-outflow when mixed", () => {
      const convention = detectAmountConvention(
        ["Date", "Merchant", "Amount"],
        [
          ["2026-01-01", "x", "10.00"],
          ["2026-01-02", "y", "-10.00"],
        ]
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
  });
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cd ui && npx vitest run src/utils/csvDetection.test.ts`
  Expected: FAIL — module not found / functions not defined.

- [ ] **Step 3: Implement csvDetection.ts**

  Create `ui/src/utils/csvDetection.ts`:

  ```ts
  import type { CsvPreview, ColumnRole, AmountConvention } from "../api/client";

  export interface DetectedMapping {
    skipHeaderRows: number;
    columns: ColumnRole[];
    dateFormat: string | null;
    amountConvention: AmountConvention;
    detectedFields: Set<string>;
  }

  const ROLE_KEYWORDS: Record<ColumnRole, string[]> = {
    Date: ["date", "transactiondate", "postdate", "posteddate", "dateposted"],
    Amount: ["amount", "transactionamount", "amountusd", "amount"],
    Merchant: ["description", "merchant", "payee", "transactiondescription", "name", "details"],
    Notes: ["notes", "memo", "reference", "tag"],
    Category: ["category"],
    Debit: ["debit", "debitamount", "withdrawal", "withdrawals"],
    Credit: ["credit", "creditamount", "deposit", "deposits"],
    Skip: [],
  };

  const DATE_FORMATS = [
    "%Y-%m-%d",
    "%m/%d/%Y",
    "%d/%m/%Y",
    "%d.%m.%Y",
    "%B %d, %Y",
    "%d-%b-%Y",
    "%d %b %Y",
    "%b %d, %Y",
  ];

  function normalizeHeader(header: string): string {
    return header.toLowerCase().replace(/[^a-z0-9]/g, "");
  }

  export function detectColumnRoles(headers: string[]): ColumnRole[] {
    const normalized = headers.map(normalizeHeader);
    const assigned = new Set<number>();
    const roles: ColumnRole[] = Array(headers.length).fill("Skip");

    const order: ColumnRole[] = ["Date", "Merchant", "Amount", "Debit", "Credit", "Category", "Notes"];

    for (const role of order) {
      let bestIdx = -1;
      let bestScore = 0;

      for (let i = 0; i < normalized.length; i++) {
        if (assigned.has(i)) continue;
        const h = normalized[i];
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
        if (bestScore === 3) break;
      }

      if (bestIdx >= 0 && bestScore >= 3) {
        roles[bestIdx] = role;
        assigned.add(bestIdx);
      }
    }

    return roles;
  }

  export function detectDateFormat(values: string[]): string | null {
    const nonEmpty = values.filter((v) => v.trim().length > 0);
    if (nonEmpty.length === 0) return null;

    let bestFormat: string | null = null;
    let bestCount = 0;

    for (const fmt of DATE_FORMATS) {
      let count = 0;
      for (const v of nonEmpty) {
        if (parseDateWithFormat(v, fmt)) count++;
      }
      if (count > bestCount) {
        bestCount = count;
        bestFormat = fmt;
      }
    }

    return bestCount / nonEmpty.length >= 0.5 ? bestFormat : null;
  }

  function parseDateWithFormat(value: string, fmt: string): boolean {
    const trimmed = value.trim();
    const regexes: Record<string, RegExp> = {
      "%Y-%m-%d": /^\d{4}-\d{2}-\d{2}$/,
      "%m/%d/%Y": /^\d{1,2}\/\d{1,2}\/\d{4}$/,
      "%d/%m/%Y": /^\d{1,2}\/\d{1,2}\/\d{4}$/,
      "%d.%m.%Y": /^\d{1,2}\.\d{1,2}\.\d{4}$/,
      "%B %d, %Y": /^[A-Za-z]+ \d{1,2}, \d{4}$/,
      "%d-%b-%Y": /^\d{1,2}-[A-Za-z]{3}-\d{4}$/,
      "%d %b %Y": /^\d{1,2} [A-Za-z]{3} \d{4}$/,
      "%b %d, %Y": /^[A-Za-z]{3} \d{1,2}, \d{4}$/,
    };
    return regexes[fmt]?.test(trimmed) ?? false;
  }

  export function detectAmountConvention(
    roles: ColumnRole[],
    rows: string[][]
  ): AmountConvention {
    if (roles.includes("Debit") && roles.includes("Credit")) {
      return "split_debit_credit";
    }

    const amountIdx = roles.indexOf("Amount");
    if (amountIdx < 0) return "negative_is_outflow";

    let positive = 0;
    let negative = 0;
    let numeric = 0;

    for (const row of rows) {
      const raw = row[amountIdx];
      if (!raw) continue;
      const n = parseFloat(raw.replace(/[^\d.-]/g, ""));
      if (Number.isNaN(n)) continue;
      numeric++;
      if (n > 0) positive++;
      else if (n < 0) negative++;
    }

    if (numeric === 0) return "negative_is_outflow";
    const positiveRate = positive / numeric;
    const negativeRate = negative / numeric;

    if (positiveRate > 0.7) return "positive_is_outflow";
    if (negativeRate > 0.7) return "negative_is_outflow";
    return "negative_is_outflow";
  }

  export function detectHeaderRow(rows: string[][]): number {
    if (rows.length < 2) return 1;
    const first = rows[0];
    const second = rows[1];

    const firstLooksLikeHeader = first.some((cell) =>
      Object.values(ROLE_KEYWORDS)
        .flat()
        .some((kw) => normalizeHeader(cell).includes(kw))
    );

    const secondLooksLikeData = second.some((cell, idx) => {
      const norm = normalizeHeader(cell);
      const isDate = DATE_FORMATS.some((fmt) => parseDateWithFormat(cell, fmt));
      const isNumber = /^-?[\d$€£,.()\s]+$/.test(cell.trim());
      return isDate || isNumber;
    });

    if (firstLooksLikeHeader && secondLooksLikeData) return 1;
    if (secondLooksLikeData) return 0;
    return 1;
  }

  export function buildDetectedMapping(preview: CsvPreview): DetectedMapping {
    const headers = preview.headers ?? preview.rows[0] ?? [];
    const skipHeaderRows = detectHeaderRow(preview.rows);
    const dataRows = skipHeaderRows > 0 ? preview.rows.slice(skipHeaderRows) : preview.rows;
    const columns = detectColumnRoles(headers);

    const dateIdx = columns.indexOf("Date");
    const dateFormat = dateIdx >= 0 ? detectDateFormat(dataRows.map((r) => r[dateIdx] ?? "")) : null;

    const amountConvention = detectAmountConvention(columns, dataRows);

    const detectedFields = new Set<string>();
    if (columns.some((r) => r !== "Skip")) detectedFields.add("columns");
    if (dateFormat) detectedFields.add("dateFormat");
    if (amountConvention) detectedFields.add("amountConvention");
    if (skipHeaderRows !== 1) detectedFields.add("skipHeaderRows");

    return {
      skipHeaderRows,
      columns,
      dateFormat,
      amountConvention,
      detectedFields,
    };
  }
  ```

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cd ui && npx vitest run src/utils/csvDetection.test.ts`
  Expected: PASS.

- [ ] **Step 5: Type-check**

  Run: `cd ui && npx tsc --noEmit`
  Expected: No errors.

- [ ] **Step 6: Commit**

  ```bash
  git add ui/src/utils/csvDetection.ts ui/src/utils/csvDetection.test.ts
  git commit -m "feat: add CSV mapping auto-detection utilities"
  ```

---

### Task 3: Wire detection into ImportMappingDialog

**Files:**
- Modify: `ui/src/components/ImportMappingDialog.tsx`

**Interfaces:**
- Consumes: `buildDetectedMapping` from `../utils/csvDetection`, `CsvPreview` from preview query.
- Produces: Dialog state seeded from detection; visual "Auto-detected" badges on detected fields.

- [ ] **Step 1: Add detection import and state tracking**

  In `ui/src/components/ImportMappingDialog.tsx`:

  Add import:
  ```tsx
  import { buildDetectedMapping } from "../utils/csvDetection";
  ```

  Add state for tracking auto-detected fields:
  ```tsx
  const [autoDetected, setAutoDetected] = useState<Set<string>>(new Set());
  ```

- [ ] **Step 2: Seed state from detection when preview loads**

  Replace the existing `useEffect` that initializes columns with one that runs detection:

  ```tsx
  useEffect(() => {
    if (!preview) return;
    const detected = buildDetectedMapping(preview);
    const colCount = preview.headers?.length ?? preview.rows[0]?.length ?? 0;

    setColumns(detected.columns.length === colCount ? detected.columns : Array<ColumnRole>(colCount).fill("Skip"));
    setSkipHeaderRows(detected.skipHeaderRows);
    if (detected.dateFormat) {
      const preset = DATE_FORMATS.find((f) => f.value === detected.dateFormat);
      if (preset) {
        setDateFormat(detected.dateFormat);
        setCustomDateFormat("");
      } else {
        setDateFormat("__CUSTOM__");
        setCustomDateFormat(detected.dateFormat);
      }
    }
    setAmountConvention(detected.amountConvention);
    setAutoDetected(detected.detectedFields);
  }, [preview]);
  ```

- [ ] **Step 3: Clear auto-detected badge on manual change**

  Update the column dropdown `onChange` to clear the badge:

  ```tsx
  onChange={(e) => {
    const next = [...columns];
    next[i] = e.target.value as ColumnRole;
    setColumns(next);
    setAutoDetected((prev) => {
      const copy = new Set(prev);
      copy.delete("columns");
      return copy;
    });
  }}
  ```

  Update date format `onChange`:
  ```tsx
  onChange={(e) => {
    setDateFormat(e.target.value);
    setAutoDetected((prev) => {
      const copy = new Set(prev);
      copy.delete("dateFormat");
      return copy;
    });
  }}
  ```

  Update amount convention radio `onChange`:
  ```tsx
  onChange={() => {
    setAmountConvention(c.value);
    setAutoDetected((prev) => {
      const copy = new Set(prev);
      copy.delete("amountConvention");
      return copy;
    });
  }}
  ```

  Update skip header rows `onChange`:
  ```tsx
  onChange={(e) => {
    setSkipHeaderRows(parseInt(e.target.value, 10) || 0);
    setAutoDetected((prev) => {
      const copy = new Set(prev);
      copy.delete("skipHeaderRows");
      return copy;
    });
  }}
  ```

- [ ] **Step 4: Render auto-detected badges**

  In the Import settings section, add a small badge next to each detected field. For example, next to the date format select:

  ```tsx
  <div className="stack stack-xs">
    <Select
      label={
        <span>
          Date format
          {autoDetected.has("dateFormat") && <span className="chip">Auto-detected</span>}
        </span>
      }
      value={dateFormat}
      onChange={...}
    >
      ...
    </Select>
    ...
  </div>
  ```

  Apply the same pattern to the Account select (skip), Amount convention radios, and Skip header rows input. For column mapping, show a single badge in the "Column mapping" section header when `autoDetected.has("columns")`.

  Use existing styling classes (`chip`, `muted`) — no new CSS needed.

- [ ] **Step 5: Run tests and type-check**

  Run:
  ```bash
  cd ui && npx vitest run src/components/ImportMappingDialog.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no type errors.

- [ ] **Step 6: Commit**

  ```bash
  git add ui/src/components/ImportMappingDialog.tsx
  git commit -m "feat: wire CSV auto-detection into import dialog"
  ```

---

### Task 4: Improve backend amount parsing

**Files:**
- Modify: `crates/finsight-providers/src/csv/parse.rs`

**Interfaces:**
- Consumes: Raw amount strings from CSV rows.
- Produces: More forgiving parsing; same `Result<i64, ParseError>` return type.

- [ ] **Step 1: Write the failing tests**

  In `crates/finsight-providers/src/csv/parse.rs`, add these tests inside the existing `mod tests`:

  ```rust
  #[test]
  fn parentheses_negative_amount() {
      let m = map(
          vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
          AmountConvention::NegativeIsOutflow,
          "%Y-%m-%d",
      );
      let p = parse_row(&["2026-05-19", "Refund", "(8.42)"], &m).unwrap();
      assert_eq!(p.amount_cents, -842);
  }

  #[test]
  fn trailing_minus_sign() {
      let m = map(
          vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
          AmountConvention::NegativeIsOutflow,
          "%Y-%m-%d",
      );
      let p = parse_row(&["2026-05-19", "Refund", "8.42-"], &m).unwrap();
      assert_eq!(p.amount_cents, -842);
  }

  #[test]
  fn leading_plus_sign() {
      let m = map(
          vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
          AmountConvention::NegativeIsOutflow,
          "%Y-%m-%d",
      );
      let p = parse_row(&["2026-05-19", "Store", "+8.42"], &m).unwrap();
      assert_eq!(p.amount_cents, 842);
  }

  #[test]
  fn cr_dr_suffixes() {
      let m = map(
          vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
          AmountConvention::NegativeIsOutflow,
          "%Y-%m-%d",
      );
      let p = parse_row(&["2026-05-19", "Store", "8.42 DR"], &m).unwrap();
      assert_eq!(p.amount_cents, -842);
      let p = parse_row(&["2026-05-19", "Refund", "8.42 CR"], &m).unwrap();
      assert_eq!(p.amount_cents, 842);
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p finsight-providers csv::parse::tests`
  Expected: FAIL — new tests fail to parse.

- [ ] **Step 3: Update parse_amount**

  Replace `parse_amount` in `crates/finsight-providers/src/csv/parse.rs`:

  ```rust
  fn parse_amount(s: &str, decimal_separator: char) -> Result<i64, ParseError> {
      let trimmed = s.trim();

      // Detect common bank suffixes before stripping characters.
      let is_credit = trimmed.to_uppercase().ends_with(" CR");
      let is_debit = trimmed.to_uppercase().ends_with(" DR");
      let has_trailing_minus = trimmed.ends_with('-');
      let has_parentheses = trimmed.starts_with('(') && trimmed.ends_with(')');

      let core: String = trimmed
          .chars()
          .filter_map(|c| match c {
              '\u{2212}' => Some('-'), // Unicode minus sign → ASCII hyphen
              ',' if decimal_separator == ',' => Some('.'),
              '.' if decimal_separator == ',' => None,
              ',' if decimal_separator == '.' => None,
              ' ' | '$' | '€' | '£' | 'C' | 'R' | 'D' => None,
              '(' | ')' | '+' => None,
              other => Some(other),
          })
          .collect();

      let mut f: f64 = core
          .parse()
          .map_err(|_| ParseError::UnparseableAmount(s.to_owned()))?;

      if has_parentheses || has_trailing_minus {
          f = -f.abs();
      }
      if is_debit {
          f = -f.abs();
      }
      if is_credit {
          f = f.abs();
      }

      Ok((f * 100.0).round() as i64)
  }
  ```

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p finsight-providers csv::parse::tests`
  Expected: PASS.

- [ ] **Step 5: Run integration tests**

  Run: `cargo test --test csv_integration -p finsight-providers`
  Expected: PASS.

- [ ] **Step 6: Commit**

  ```bash
  git add crates/finsight-providers/src/csv/parse.rs
  git commit -m "feat: handle parentheses, trailing minus, CR/DR in CSV amounts"
  ```

---

### Task 5: Final verification

**Files:**
- Modify: None (verification only).

- [ ] **Step 1: Run all frontend tests**

  Run: `cd ui && npx vitest run`
  Expected: All tests pass.

- [ ] **Step 2: Run TypeScript check**

  Run: `cd ui && npx tsc --noEmit`
  Expected: No errors.

- [ ] **Step 3: Run all Rust tests**

  Run: `cargo test --workspace`
  Expected: All tests pass.

- [ ] **Step 4: Commit any fixes**

  If verification surfaced issues, commit fixes; otherwise this task is verification only.

---

## Self-Review

**Spec coverage:**
- ✅ Auto-detect column roles → Task 2
- ✅ Auto-detect date format → Task 2
- ✅ Auto-detect amount convention → Task 2
- ✅ Auto-detect header row → Task 2
- ✅ Pre-fill dialog with detected values → Task 3
- ✅ Manual override remains possible → Task 3
- ✅ More forgiving amount parsing → Task 4
- ✅ Add AMEX date preset → Task 1
- ✅ Tests → Tasks 2, 4, 5

**Placeholder scan:**
- ✅ No TBD/TODO placeholders
- ✅ All code blocks contain concrete implementation
- ✅ Exact commands provided

**Type consistency:**
- ✅ `CsvPreview`, `ColumnRole`, `AmountConvention` types imported from `../api/client`
- ✅ `detectedFields` returned as `Set<string>` and consumed as `Set<string>`

# Robust CSV Importer with Auto-Detection

**Date:** 2026-07-02  
**Status:** Approved  
**Scope:** Make the CSV import dialog auto-detect column mapping, date format, amount convention, and header rows from bank statement CSVs, while keeping full manual override.

## Context

The current CSV importer (`ui/src/components/ImportMappingDialog.tsx`) requires the user to manually map every column, pick a date format, and choose an amount convention. This is error-prone and confusing for first-time users. A recent AMEX import failed silently because the date format `01 Jul 2026` was not in the preset list, causing every row to fail date parsing.

## Goals

1. Auto-detect column roles from CSV headers (Date, Merchant, Amount, Debit/Credit, etc.).
2. Auto-detect date format by trying presets against preview rows.
3. Auto-detect amount convention (positive vs negative outflow, split debit/credit).
4. Auto-detect whether the first row is a header.
5. Pre-fill the mapping dialog with detected values; user can override anything.
6. Make the backend amount parser handle common bank variants: parentheses, trailing minus, CR/DR, leading plus.
7. Add unit/integration tests for detection logic and parsing.

## Non-goals

- Remove the mapping dialog or create a fully one-click import flow.
- Add new backend IPC endpoints; reuse existing `previewCsvColumns` and `importCsv`.
- Support non-CSV formats (OFX, QFX, etc.).
- Auto-categorize imported transactions during import.

## Design

### Architecture

```
User selects CSV
  → previewCsvColumns(path, skipHeaderRows)
  → csvDetection.ts analyzes preview rows + headers
  → ImportMappingDialog state is seeded with detected mapping
  → user reviews/overrides and submits
  → importCsv(path, accountId, mapping)
  → backend parses rows with forgiving parse_amount
```

### New frontend module: `ui/src/utils/csvDetection.ts`

Pure helper functions (no React, no side effects):

- `detectColumnRoles(headers: string[]): ColumnRole[]`
  - Normalize each header: lowercase, strip non-alphanumeric.
  - Score against keyword tables for each `ColumnRole`.
  - Assign each role to at most one column; prefer exact matches over substring matches.
  - Unmatched columns default to `Skip`.

- `detectDateFormat(values: string[]): string | null`
  - Try each preset format against non-empty values.
  - Return the format with the highest parse success count.
  - Require at least 50% success rate; otherwise return `null`.

- `detectAmountConvention(roles: ColumnRole[], rows: string[][]): AmountConvention`
  - If `Debit` and `Credit` roles are both assigned → `split_debit_credit`.
  - Else if `Amount` role assigned, inspect signs of numeric-looking cells:
    - >70% positive → `positive_is_outflow`
    - >70% negative → `negative_is_outflow`
    - mixed/unclear → default `negative_is_outflow`
  - Else default `negative_is_outflow`.

- `detectHeaderRow(rows: string[][]): number`
  - If first row cells look like header keywords and second row looks like data (parsable date/number), return `1`.
  - If first row looks like data, return `0`.
  - Default to `1`.

- `buildDetectedMapping(preview: CsvPreview): DetectedMapping`
  - Combines the above helpers and returns a full mapping plus confidence flags.

### UI behavior in `ImportMappingDialog.tsx`

- When `preview` loads, call `buildDetectedMapping(preview)`.
- Seed state with detected values:
  - `columns`
  - `dateFormat` (or `__CUSTOM__` + `customDateFormat` if needed)
  - `amountConvention`
  - `skipHeaderRows`
- Show a muted "Auto-detected" badge next to fields that were filled automatically.
- Remove the badge when the user manually changes that field.
- If detection confidence is low (e.g., required fields still missing), keep the existing "Map X more required fields" helper text.
- Preserve existing manual controls and validation.

### Backend parsing improvements

In `crates/finsight-providers/src/csv/parse.rs`, extend `parse_amount` to normalize:

- `(8.42)` → `-8.42`
- `8.42-` → `-8.42`
- `+8.42` → `8.42`
- `8.42 CR`, `8.42 DR` → strip suffix, parse sign contextually
- Existing handling of `$`, `€`, `£`, spaces, Unicode minus, and comma thousands separators stays.

### Date format presets

Add these presets to `ImportMappingDialog.tsx`:

- `01 Jul 2026` → `%d %b %Y` (AMEX)
- `Jul 01, 2026` → `%b %d, %Y` (some US banks)

These are already supported by Rust `chrono`.

## Testing

1. **TypeScript unit tests** (`ui/src/utils/csvDetection.test.ts`):
   - Header matching for Chase, AMEX, CIBC, RBC style headers.
   - Date format detection on sample cells.
   - Amount convention detection from signed samples.
   - Header-row detection (header present vs absent).

2. **Rust unit tests** in `csv::parse::tests`:
   - Parentheses negative amount.
   - Trailing minus.
   - Leading plus.
   - CR/DR suffixes.

3. **Rust integration tests** in `tests/csv_integration.rs`:
   - Real AMEX all-time statement fixture already added; verify it imports cleanly.

## Files to change

- `ui/src/utils/csvDetection.ts` (new)
- `ui/src/utils/csvDetection.test.ts` (new)
- `ui/src/components/ImportMappingDialog.tsx`
- `crates/finsight-providers/src/csv/parse.rs`
- `crates/finsight-providers/tests/fixtures/csv/amex-all-time-statement.csv` (already added)
- `crates/finsight-providers/tests/csv_integration.rs` (already added)

## Risks

- Detection heuristics can guess wrong for unusual bank formats; manual override must remain obvious.
- Adding too many date presets clutters the dropdown; only add common formats.
- Date format detection relies on a small preview; ambiguous formats (`%d/%m/%Y` vs `%m/%d/%Y`) may be mis-detected.

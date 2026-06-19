import { useEffect, useState } from "react";
import FocusLock from "react-focus-lock";
import { toast } from "sonner";
import { usePreviewCsvColumns } from "../../api/hooks/csv";
import { useImportCsv } from "../../api/hooks/transactions";
import { useAccounts } from "../../api/hooks/accounts";
import type { CsvImportMapping, ImportSummary, ColumnRole } from "../../api/client";
import Button from "../../components/Button";
import Select from "../../components/Select";
import Input from "../../components/Input";
import Table, { TableHead, TableBody, TableRow, TableHeader, TableCell } from "../../components/Table";

const COLUMN_ROLES: ColumnRole[] = ["Date", "Amount", "Merchant", "Notes", "Category", "Skip", "Debit", "Credit"];

const DATE_FORMATS = [
  { label: "2026-05-19",   value: "%Y-%m-%d" },
  { label: "5/19/2026",    value: "%m/%d/%Y" },
  { label: "19/05/2026",   value: "%d/%m/%Y" },
  { label: "19.05.2026",   value: "%d.%m.%Y" },
  { label: "May 19, 2026", value: "%B %d, %Y" },
  { label: "19-May-2026",  value: "%d-%b-%Y" },
  { label: "Custom",       value: "__CUSTOM__" },
];

const AMOUNT_CONVENTIONS = [
  { label: "Negative = outflow",         value: "negative_is_outflow" as const },
  { label: "Positive = outflow",         value: "positive_is_outflow" as const },
  { label: "Separate debit/credit cols", value: "split_debit_credit" as const },
];

interface Props {
  path: string;
  onClose: () => void;
  onImported: (summary: ImportSummary) => void;
  defaultAccountId?: string;
}

export default function ImportMappingDialog({ path, onClose, onImported, defaultAccountId }: Props) {
  const [skipHeaderRows, setSkipHeaderRows] = useState(1);
  const { data: preview } = usePreviewCsvColumns(path, skipHeaderRows);
  const { data: accounts = [] } = useAccounts();

  const [accountId, setAccountId] = useState(defaultAccountId ?? "");
  const [columns, setColumns] = useState<ColumnRole[]>([]);
  const [dateFormat, setDateFormat] = useState("%Y-%m-%d");
  const [customDateFormat, setCustomDateFormat] = useState("");
  const [amountConvention, setAmountConvention] =
    useState<"negative_is_outflow" | "positive_is_outflow" | "split_debit_credit">("negative_is_outflow");

  useEffect(() => {
    if (!preview) return;
    const colCount = preview.headers?.length ?? preview.rows[0]?.length ?? 0;
    setColumns((prev) =>
      prev.length === colCount ? prev : Array<ColumnRole>(colCount).fill("Skip")
    );
  }, [preview]);

  const importCsv = useImportCsv();

  const finalDateFormat = dateFormat === "__CUSTOM__" ? customDateFormat : dateFormat;
  const canSubmit =
    !!accountId &&
    finalDateFormat.length > 0 &&
    columns.includes("Date") &&
    columns.includes("Merchant") &&
    (amountConvention === "split_debit_credit"
      ? columns.includes("Debit") && columns.includes("Credit")
      : columns.includes("Amount"));

  async function submit() {
    const mapping: CsvImportMapping = {
      skip_header_rows: skipHeaderRows,
      columns: columns as CsvImportMapping["columns"],
      date_format: finalDateFormat,
      amount_convention: amountConvention,
      decimal_separator: ".",
      delimiter: null,
    };
    try {
      const summary = await importCsv.mutateAsync({ path, account_id: accountId, mapping });
      toast.success(
        `Imported ${summary.rows_imported} transaction${summary.rows_imported === 1 ? "" : "s"}, skipped ${summary.rows_skipped_duplicates} duplicate${summary.rows_skipped_duplicates === 1 ? "" : "s"}`
      );
      onImported(summary);
    } catch {
      // importCsv.error is now set; rendered in the footer
    }
  }

  const headers = preview?.headers ?? preview?.rows[0] ?? [];

  return (
    <FocusLock returnFocus>
      <div className="dialog-backdrop" onClick={onClose} aria-hidden="true" />
      <div
        className="dialog-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="map-title"
        onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
      >
        <header>
          <h2 id="map-title">Map CSV columns</h2>
        </header>

        <div className="dialog-grid">
          <Select
            label="Account"
            value={accountId}
            onChange={(e) => setAccountId(e.target.value)}
          >
            <option value="">— Pick —</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.bank} · {a.name}
              </option>
            ))}
          </Select>
          <Input
            label="Skip header rows"
            type="number"
            min={0}
            value={skipHeaderRows}
            onChange={(e) => setSkipHeaderRows(parseInt(e.target.value, 10) || 0)}
          />
          <div className="stack stack-xs">
            <Select
              label="Date format"
              value={dateFormat}
              onChange={(e) => setDateFormat(e.target.value)}
            >
              {DATE_FORMATS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
            </Select>
            {dateFormat === "__CUSTOM__" && (
              <Input
                placeholder="e.g. %Y/%m/%d"
                value={customDateFormat}
                onChange={(e) => setCustomDateFormat(e.target.value)}
              />
            )}
          </div>
          <fieldset>
            <legend>Amount convention</legend>
            {AMOUNT_CONVENTIONS.map((c) => (
              <label key={c.value}>
                <input
                  type="radio"
                  name="conv"
                  value={c.value}
                  checked={amountConvention === c.value}
                  onChange={() => setAmountConvention(c.value)}
                />{" "}
                {c.label}
              </label>
            ))}
          </fieldset>
        </div>

        {preview && (
          <Table>
            <TableHead>
              <TableRow>
                {headers.map((_, i) => (
                  <TableHeader key={i}>
                    <Select
                      value={columns[i] ?? "Skip"}
                      onChange={(e) => {
                        const next = [...columns];
                        next[i] = e.target.value as ColumnRole;
                        setColumns(next);
                      }}
                      aria-label={`Column ${i + 1} role`}
                    >
                      {COLUMN_ROLES.map((r) => (
                        <option key={r} value={r}>
                          {r}
                        </option>
                      ))}
                    </Select>
                  </TableHeader>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {preview.rows.slice(0, 5).map((row, ri) => (
                <TableRow key={ri}>
                  {row.map((cell, ci) => (
                    <TableCell key={ci}>{cell}</TableCell>
                  ))}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}

        <footer>
          {importCsv.error && (
            <p role="alert" className="error-text" style={{ color: "var(--error, red)", margin: "0 0 8px" }}>
              {importCsv.error.message}
            </p>
          )}
          <Button variant="ghost" onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            onClick={submit}
            disabled={!canSubmit || importCsv.isPending}
            loading={importCsv.isPending}
          >
            {importCsv.isPending ? "Importing…" : "Import"}
          </Button>
        </footer>
      </div>
    </FocusLock>
  );
}

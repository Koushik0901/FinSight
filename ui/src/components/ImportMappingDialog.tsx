import { useEffect, useMemo, useState } from "react";
import FocusLock from "react-focus-lock";
import { toast } from "sonner";
import { usePreviewCsvColumns } from "../api/hooks/csv";
import { useImportCsv } from "../api/hooks/transactions";
import { useAccounts } from "../api/hooks/accounts";
import type { CsvImportMapping, ImportSummary, ColumnRole } from "../api/client";
import Button from "./Button";
import Select from "./Select";
import Input from "./Input";
import Table, { TableHead, TableBody, TableRow, TableHeader, TableCell } from "./Table";
import { Grid, Check, ArrowRight } from "./Icons";

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
  const { data: preview, isPending: previewLoading } = usePreviewCsvColumns(path, skipHeaderRows);
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

  const mappedCount = useMemo(() => {
    return {
      date: columns.includes("Date"),
      merchant: columns.includes("Merchant"),
      amount: columns.includes("Amount"),
      debit: columns.includes("Debit"),
      credit: columns.includes("Credit"),
    };
  }, [columns]);

  const requiredMet = useMemo(() => {
    const amountReady = amountConvention === "split_debit_credit"
      ? mappedCount.debit && mappedCount.credit
      : mappedCount.amount;
    return mappedCount.date && mappedCount.merchant && amountReady;
  }, [mappedCount, amountConvention]);

  const canSubmit =
    !!accountId &&
    finalDateFormat.length > 0 &&
    requiredMet;

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
  const accountLabel = useMemo(() => {
    const acc = accounts.find((a) => a.id === accountId);
    return acc ? `${acc.bank} · ${acc.name}` : null;
  }, [accounts, accountId]);

  const requiredItems = [
    { key: "date", label: "Date", ready: mappedCount.date },
    { key: "merchant", label: "Merchant", ready: mappedCount.merchant },
    {
      key: "amount",
      label: amountConvention === "split_debit_credit" ? "Debit + Credit" : "Amount",
      ready: amountConvention === "split_debit_credit"
        ? mappedCount.debit && mappedCount.credit
        : mappedCount.amount,
    },
  ] as const;

  return (
    <FocusLock returnFocus>
      <div className="dialog-backdrop" onClick={onClose} aria-hidden="true" />
      <div
        className="dialog-overlay import-mapping-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="map-title"
        onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
      >
        <header>
          <div className="import-header">
            <div className="import-header-icon">
              <Grid />
            </div>
            <div>
              <span className="eyebrow">CSV import</span>
              <h2 id="map-title">Map your columns</h2>
            </div>
          </div>
          <p className="import-subtitle">
            Tell FinSight which column means what. The preview updates as you adjust headers or format.
          </p>
        </header>

        <section className="import-section">
          <div className="import-section-head">
            <span className="eyebrow">
              <span className="dot" />
              Import settings
            </span>
            {accountLabel && <span className="chip accent">{accountLabel}</span>}
          </div>
          <div className="import-settings card tight">
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
            <fieldset className="import-convention">
              <legend>Amount convention</legend>
              {AMOUNT_CONVENTIONS.map((c) => (
                <label key={c.value} className={amountConvention === c.value ? "active" : undefined}>
                  <input
                    type="radio"
                    name="conv"
                    value={c.value}
                    checked={amountConvention === c.value}
                    onChange={() => setAmountConvention(c.value)}
                  />
                  {c.label}
                </label>
              ))}
            </fieldset>
          </div>
        </section>

        <section className="import-section">
          <div className="import-section-head">
            <span className="eyebrow">
              <span className="dot" />
              Column mapping
            </span>
            <div className="import-required" role="status" aria-live="polite">
              {requiredItems.map((item) => (
                <span
                  key={item.key}
                  className={`chip ${item.ready ? "positive" : ""}`}
                  aria-label={`${item.label} ${item.ready ? "mapped" : "not mapped"}`}
                >
                  {item.ready && <Check />}
                  {item.label}
                </span>
              ))}
            </div>
          </div>

          {previewLoading && (
            <div className="import-preview-skeleton">
              <div className="import-skeleton-row" />
              <div className="import-skeleton-row" />
              <div className="import-skeleton-row" />
            </div>
          )}

          {!previewLoading && preview && (
            <div className="import-preview-wrap">
              <Table wrap={false} className="preview-table">
                <TableHead>
                  <TableRow>
                    {headers.map((header, i) => {
                      const role = columns[i] ?? "Skip";
                      const mapped = role !== "Skip";
                      return (
                        <TableHeader key={i} className={mapped ? "mapped" : undefined}>
                          <div className="import-column-header">
                            <span className="import-column-idx">{i + 1}</span>
                            <Select
                              value={role}
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
                            {mapped && (
                              <span className={`import-role-badge role-${role.toLowerCase()}`}>
                                {role}
                              </span>
                            )}
                          </div>
                          <span className="import-column-name" title={String(header)}>
                            {header}
                          </span>
                        </TableHeader>
                      );
                    })}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {preview.rows.slice(0, 5).map((row, ri) => (
                    <TableRow key={ri}>
                      {row.map((cell, ci) => {
                        const role = columns[ci] ?? "Skip";
                        return (
                          <TableCell
                            key={ci}
                            className={role !== "Skip" ? `mapped-role-${role.toLowerCase()}` : undefined}
                          >
                            {cell}
                          </TableCell>
                        );
                      })}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
              {preview.rows.length > 5 && (
                <div className="import-preview-more">
                  +{preview.rows.length - 5} more rows
                </div>
              )}
            </div>
          )}
        </section>

        <footer>
          <div className="import-footer-status">
            {importCsv.error ? (
              <span className="chip negative" role="alert">
                {importCsv.error.message}
              </span>
            ) : (
              <span className="import-summary">
                {requiredMet
                  ? "Ready to import"
                  : `Map ${requiredItems.filter((i) => !i.ready).length} more required field${requiredItems.filter((i) => !i.ready).length === 1 ? "" : "s"}`}
              </span>
            )}
          </div>
          <div className="import-footer-actions">
            <Button variant="ghost" onClick={onClose}>Cancel</Button>
            <Button
              variant="primary"
              onClick={submit}
              disabled={!canSubmit || importCsv.isPending}
              loading={importCsv.isPending}
            >
              {importCsv.isPending ? "Importing…" : (
                <>
                  Import <ArrowRight />
                </>
              )}
            </Button>
          </div>
        </footer>
      </div>
    </FocusLock>
  );
}

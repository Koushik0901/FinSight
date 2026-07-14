import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import FocusLock from "react-focus-lock";
import { toast } from "sonner";
import { usePreviewCsvColumns, useSavedCsvMapping, usePrepareImport } from "../api/hooks/csv";
import { useImportCsv } from "../api/hooks/transactions";
import { useAccounts } from "../api/hooks/accounts";
import { commands, type CsvImportMapping, type ImportSummary, type ColumnRole } from "../api/client";
import AccountDrawer from "./AccountDrawer";
import Button from "./Button";
import Select from "./Select";
import Input from "./Input";
import Table, { TableHead, TableBody, TableRow, TableHeader, TableCell } from "./Table";
import { Grid, Check, ArrowRight } from "./Icons";
import { buildDetectedMapping } from "../utils/csvDetection";
import { accountTypeColor } from "../utils/accountColor";

const COLUMN_ROLES: ColumnRole[] = [
  "Date",
  "Amount",
  "Merchant",
  "Notes",
  "Category",
  "Skip",
  "Debit",
  "Credit",
  "ActivityType",
  "ActivitySubType",
  "Symbol",
  "SecurityName",
  "Quantity",
  "UnitPrice",
];

const INVESTMENT_ROLES: ColumnRole[] = [
  "ActivityType",
  "ActivitySubType",
  "Symbol",
  "SecurityName",
  "Quantity",
  "UnitPrice",
];

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

type AmountConventionValue = "negative_is_outflow" | "positive_is_outflow" | "split_debit_credit";

interface Props {
  path: string;
  onClose: () => void;
  onImported: (summary: ImportSummary) => void;
  defaultAccountId?: string;
}

export default function ImportMappingDialog({ path, onClose, onImported, defaultAccountId }: Props) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [skipHeaderRows, setSkipHeaderRows] = useState(1);
  const { data: preview, isPending: previewLoading } = usePreviewCsvColumns(path, skipHeaderRows);
  const { data: accounts = [] } = useAccounts();

  const [accountId, setAccountId] = useState(defaultAccountId ?? "");
  const [newAccountOpen, setNewAccountOpen] = useState(false);
  const { data: savedMapping } = useSavedCsvMapping(accountId || null);
  const [columns, setColumns] = useState<ColumnRole[]>([]);
  const [dateFormat, setDateFormat] = useState("%Y-%m-%d");
  const [customDateFormat, setCustomDateFormat] = useState("");
  // Amount handling is modelled as two independent toggles rather than a
  // three-way "positive/negative = outflow" radio: the default is the standard
  // negative-is-outflow, "Flip amounts" swaps it (for credit-card exports where
  // charges are positive), and "Separate debit/credit columns" is a distinct
  // shape. The wire value is derived below.
  const [splitMode, setSplitMode] = useState(false);
  const [flipAmounts, setFlipAmounts] = useState(false);
  const amountConvention: AmountConventionValue = splitMode
    ? "split_debit_credit"
    : flipAmounts
      ? "positive_is_outflow"
      : "negative_is_outflow";
  const applyConvention = (conv: AmountConventionValue) => {
    setSplitMode(conv === "split_debit_credit");
    setFlipAmounts(conv === "positive_is_outflow");
  };
  const [autoDetected, setAutoDetected] = useState<Set<string>>(new Set());
  const [usingSaved, setUsingSaved] = useState(false);
  const detectionAppliedForPath = useRef<string | null>(null);
  const savedAppliedForAccount = useRef<string | null>(null);

  useEffect(() => {
    if (!preview) return;
    const colCount = preview.headers?.length ?? preview.rows[0]?.length ?? 0;

    if (detectionAppliedForPath.current === path) {
      setColumns((current) =>
        current.length === colCount ? current : Array<ColumnRole>(colCount).fill("Skip"),
      );
      return;
    }

    detectionAppliedForPath.current = path;
    const detected = buildDetectedMapping(preview);
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
    applyConvention(detected.amountConvention);
    setAutoDetected(detected.detectedFields);
  }, [path, preview]);

  // Apply the mapping this account was last imported with, so a recurring
  // import from the same bank needs zero fiddling. Runs once per account; a
  // saved mapping wins over both auto-detection and the account-type default.
  useEffect(() => {
    if (!accountId) return;
    if (savedAppliedForAccount.current === accountId) return;
    if (savedMapping === undefined) return; // still loading
    savedAppliedForAccount.current = accountId;
    if (!savedMapping) {
      setUsingSaved(false);
      return;
    }
    const colCount = preview?.headers?.length ?? preview?.rows[0]?.length ?? 0;
    if (colCount > 0 && savedMapping.columns.length === colCount) {
      setColumns(savedMapping.columns as ColumnRole[]);
    }
    setSkipHeaderRows(savedMapping.skip_header_rows);
    const preset = DATE_FORMATS.find((f) => f.value === savedMapping.date_format);
    if (preset) {
      setDateFormat(savedMapping.date_format);
      setCustomDateFormat("");
    } else {
      setDateFormat("__CUSTOM__");
      setCustomDateFormat(savedMapping.date_format);
    }
    applyConvention(savedMapping.amount_convention);
    setUsingSaved(true);
    setAutoDetected(new Set()); // saved settings supersede the auto-detected badges
  }, [accountId, savedMapping, preview]);

  // Fallback default when the account has no saved mapping: credit-card and loan
  // exports use positive = a charge (outflow), bank/asset exports negative =
  // outflow. Picking wrong silently inverts every row, so bias the flip by
  // account type — but only while still auto-detected (a manual choice wins).
  useEffect(() => {
    if (savedMapping) return; // saved mapping already applied
    if (splitMode) return;
    if (!autoDetected.has("amountConvention")) return;
    const selected = accounts.find((a) => a.id === accountId);
    if (!selected) return;
    const isLiabilityAccount = selected.type === "Credit" || selected.type === "Loan";
    setFlipAmounts(isLiabilityAccount);
  }, [accountId, accounts, autoDetected, savedMapping, splitMode]);

  const importCsv = useImportCsv();

  const finalDateFormat = dateFormat === "__CUSTOM__" ? customDateFormat : dateFormat;

  const mappedCount = useMemo(() => {
    return {
      date: columns.includes("Date"),
      // An ActivityType column satisfies the merchant requirement: brokerage
      // exports leave the name column empty on most rows and the importer
      // synthesizes merchants from the activity instead.
      merchant: columns.includes("Merchant") || columns.includes("ActivityType"),
      amount: columns.includes("Amount"),
      debit: columns.includes("Debit"),
      credit: columns.includes("Credit"),
    };
  }, [columns]);

  const hasInvestmentColumns = useMemo(
    () => columns.some((c) => INVESTMENT_ROLES.includes(c)),
    [columns],
  );

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

  const previewMapping = useMemo<CsvImportMapping | null>(() => {
    if (!canSubmit) return null;
    return {
      skip_header_rows: skipHeaderRows,
      columns: columns as CsvImportMapping["columns"],
      date_format: finalDateFormat,
      amount_convention: amountConvention,
      decimal_separator: ".",
      delimiter: null,
    };
  }, [canSubmit, skipHeaderRows, columns, finalDateFormat, amountConvention]);

  // Debounce so rapid column/format edits don't spam the backend with a
  // speculative parse+reconcile on every keystroke.
  const [debouncedMapping, setDebouncedMapping] = useState<CsvImportMapping | null>(null);
  useEffect(() => {
    const t = setTimeout(() => setDebouncedMapping(previewMapping), 300);
    return () => clearTimeout(t);
  }, [previewMapping]);
  const prep = usePrepareImport(path, accountId || null, debouncedMapping);

  async function submit() {
    const mapping: CsvImportMapping = previewMapping ?? {
      skip_header_rows: skipHeaderRows,
      columns: columns as CsvImportMapping["columns"],
      date_format: finalDateFormat,
      amount_convention: amountConvention,
      decimal_separator: ".",
      delimiter: null,
    };
    try {
      const result = await importCsv.mutateAsync({ path, account_id: accountId, mapping });
      const summary = result.summary;
      const { rows_imported, rows_skipped_duplicates, rows_queued_for_review, errors } = summary;
      const importedNoun = rows_imported === 1 ? "transaction" : "transactions";
      const duplicateNoun = rows_skipped_duplicates === 1 ? "duplicate" : "duplicates";
      const queuedNoun = rows_queued_for_review === 1 ? "item" : "items";
      const parts: string[] = [];
      if (rows_imported > 0) {
        parts.push(`Imported ${rows_imported} ${importedNoun}`);
      }
      if (rows_skipped_duplicates > 0) {
        parts.push(`skipped ${rows_skipped_duplicates} ${duplicateNoun}`);
      }
      if (rows_queued_for_review > 0) {
        parts.push(`${rows_queued_for_review} queued for review`);
      }
      const summaryText = parts.length > 0 ? parts.join(", ") : "No rows were imported";
      if (errors.length > 0) {
        const firstReason = errors[0]?.reason ?? "Check the row details.";
        toast.error(`Import finished with ${errors.length} row error${errors.length === 1 ? "" : "s"}`, {
          description: `${summaryText}. ${errors.length === 1 ? firstReason : "Open Import Review to see details."}`,
        });
      } else {
        toast.success(summaryText);
      }

      // Make the cloud LLM categorization a visible, informed choice. If the
      // auto-categorize setting already enqueued it, say so; otherwise offer an
      // explicit action rather than silently sending nothing (or everything).
      if (result.uncategorizedAfter > 0) {
        const n = result.uncategorizedAfter;
        const noun = n === 1 ? "transaction has" : "transactions have";
        if (result.aiCategorizationStarted) {
          toast(`Categorizing ${n} uncategorized ${n === 1 ? "transaction" : "transactions"} with AI…`, {
            description: "Running in the background. You can turn this off in Settings → Agent.",
          });
        } else {
          toast(`${n} ${noun} no category`, {
            description: "Run AI categorization to sort them (sends merchant + amount to your provider).",
            action: {
              label: "Categorize with AI",
              onClick: () => {
                void commands.triggerCategorize().then((r) => {
                  if (r.status === "error") toast.error(r.error.message);
                  else toast.success("AI categorization started");
                });
              },
            },
          });
        }
      }

      onImported(summary);
      if (rows_queued_for_review > 0) {
        navigate("/import-review");
      }
    } catch {
      // importCsv.error is now set; rendered in the footer
    }
  }

  const headers = preview?.headers ?? preview?.rows[0] ?? [];
  const selectedAccount = useMemo(
    () => accounts.find((a) => a.id === accountId) ?? null,
    [accounts, accountId]
  );
  const accountLabel = selectedAccount ? `${selectedAccount.bank} · ${selectedAccount.name}` : null;

  // Closing the dialog should drop any cached speculative preview — it's
  // advisory UI scoped to this session and shouldn't linger stale in cache.
  const handleClose = () => {
    qc.invalidateQueries({ queryKey: ["csv-prepare"] });
    onClose();
  };

  const requiredItems = [
    { key: "date", label: "Date", ready: mappedCount.date },
    {
      key: "merchant",
      label: hasInvestmentColumns ? "Merchant / Activity" : "Merchant",
      ready: mappedCount.merchant,
    },
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
      <div className="dialog-backdrop" onClick={handleClose} aria-hidden="true" />
      <div
        className="dialog-overlay import-mapping-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="map-title"
        onKeyDown={(e) => { if (e.key === "Escape") handleClose(); }}
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
            {accountLabel && selectedAccount && (
              <span className="chip accent">
                <span className="cswatch" style={{ background: accountTypeColor(selectedAccount.type), width: 8, height: 8, marginRight: 6 }} />
                {accountLabel}
              </span>
            )}
          </div>
          <div className="import-settings card tight">
            <Select
              label="Account"
              value={accountId}
              onChange={(e) => {
                if (e.target.value === "__new__") {
                  setNewAccountOpen(true);
                  return;
                }
                setAccountId(e.target.value);
              }}
            >
              <option value="">— Pick —</option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.bank} · {a.name}
                </option>
              ))}
              <option value="__new__">+ Create new account…</option>
            </Select>
            {accounts.length === 0 && (
              <p className="import-saved-hint muted">
                Pick “+ Create new account…” to make one for this statement.
              </p>
            )}
            {usingSaved && (
              <p className="import-saved-hint muted">
                Using the settings from your last import for this account.
              </p>
            )}
            {hasInvestmentColumns && selectedAccount && selectedAccount.type !== "Investment" && (
              <p className="import-saved-hint muted">
                Investment columns detected — this statement is best imported into an
                Investment account.
              </p>
            )}
            <Input
              label="Skip header rows"
              type="number"
              min={0}
              value={skipHeaderRows}
              onChange={(e) => {
                setSkipHeaderRows(parseInt(e.target.value, 10) || 0);
                setAutoDetected((prev) => {
                  const copy = new Set(prev);
                  copy.delete("skipHeaderRows");
                  return copy;
                });
              }}
            />
            <div className="stack stack-xs">
              <Select
                label={
                  <span>
                    Date format
                    {autoDetected.has("dateFormat") && <span className="chip">Auto-detected</span>}
                  </span>
                }
                value={dateFormat}
                onChange={(e) => {
                  setDateFormat(e.target.value);
                  setAutoDetected((prev) => {
                    const copy = new Set(prev);
                    copy.delete("dateFormat");
                    return copy;
                  });
                }}
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
              <legend>
                Amount handling
                {usingSaved ? (
                  <span className="chip">Saved</span>
                ) : (
                  autoDetected.has("amountConvention") && <span className="chip">Auto-detected</span>
                )}
              </legend>
              <label className={splitMode ? "active" : undefined}>
                <input
                  type="checkbox"
                  checked={splitMode}
                  onChange={(e) => {
                    setSplitMode(e.target.checked);
                    setUsingSaved(false);
                    setAutoDetected((prev) => {
                      const copy = new Set(prev);
                      copy.delete("amountConvention");
                      return copy;
                    });
                  }}
                />
                Separate debit / credit columns
              </label>
              {!splitMode && (
                <label className={flipAmounts ? "active" : undefined}>
                  <input
                    type="checkbox"
                    checked={flipAmounts}
                    onChange={(e) => {
                      setFlipAmounts(e.target.checked);
                      setUsingSaved(false);
                      setAutoDetected((prev) => {
                        const copy = new Set(prev);
                        copy.delete("amountConvention");
                        return copy;
                      });
                    }}
                  />
                  Flip amounts — my charges are positive numbers
                </label>
              )}
              <p className="import-convention-hint muted">
                {splitMode
                  ? "One column is money out, the other money in."
                  : flipAmounts
                    ? "Positive rows count as spending — typical for credit-card exports."
                    : "Negative rows count as spending — typical for bank exports."}
              </p>
            </fieldset>
          </div>
        </section>

        <section className="import-section">
          <div className="import-section-head">
            <span className="eyebrow">
              <span className="dot" />
              Column mapping
              {autoDetected.has("columns") && <span className="chip">Auto-detected</span>}
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
                                setAutoDetected((prev) => {
                                  const copy = new Set(prev);
                                  copy.delete("columns");
                                  return copy;
                                });
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
                {!requiredMet
                  ? `Map ${requiredItems.filter((i) => !i.ready).length} more required field${requiredItems.filter((i) => !i.ready).length === 1 ? "" : "s"}`
                  : prep.isFetching && !prep.data
                    ? "Checking…"
                    : prep.data
                      ? `${prep.data.rowsImported} new · ${prep.data.rowsSkippedDuplicates} duplicates · ${prep.data.rowsQueuedForReview} to review${prep.data.errors.length ? ` · ${prep.data.errors.length} errors` : ""}`
                      : "Ready to import"}
              </span>
            )}
          </div>
          <div className="import-footer-actions">
            <Button variant="ghost" onClick={handleClose}>Cancel</Button>
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
      <AccountDrawer
        open={newAccountOpen}
        elevated
        onClose={() => setNewAccountOpen(false)}
        onCreated={(id) => {
          setAccountId(id);
          setNewAccountOpen(false);
        }}
      />
    </FocusLock>
  );
}

import { useState, useEffect, useRef } from "react";
import { useTransactions } from "../api/hooks/transactions";
import TransactionDrawer from "../components/TransactionDrawer";
import FilePicker from "../components/FilePicker";
import ImportMappingDialog from "./onboarding/ImportMappingDialog";
import type { Transaction, TxnFilterInput } from "../api/client";
import { commands } from "../api/client";
import { money } from "../utils/format";
import { toast } from "sonner";
import Button from "../components/Button";
import Input from "../components/Input";
import Table, { TableHead, TableBody, TableHeader, TableCell } from "../components/Table";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

type Preset = "" | "needs_review" | "anomalies" | "no_category";

const TABS: { key: Preset; label: string }[] = [
  { key: "", label: "All" },
  { key: "needs_review", label: "Needs review" },
  { key: "anomalies", label: "Anomalies" },
  { key: "no_category", label: "No category" },
];

export default function Transactions() {
  const [addOpen, setAddOpen] = useState(false);
  const [editTxnId, setEditTxnId] = useState<string | null>(null);
  const [csvPath, setCsvPath] = useState<string | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [preset, setPreset] = useState<Preset>("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => setSearch(searchInput), 300);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [searchInput]);

  const filter: TxnFilterInput = {
    accountId: null,
    limit: null,
    offset: null,
    search: search || null,
    filterPreset: preset || null,
  };

  const { data, isLoading, error } = useTransactions(filter);
  const editTxn = data?.find((t) => t.id === editTxnId) ?? null;

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        <span className="spinner" aria-hidden="true" />
        <span style={{ marginTop: 12 }}>Loading…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="stub" role="alert" aria-live="assertive">
        Error: {(error as Error).message}
      </div>
    );
  }

  return (
    <div className="screen-transactions">
      <header className="screen-header">
        <h1>Transactions</h1>
        <div className="actions row row-sm">
          <FilePicker onPicked={setCsvPath} label="Import CSV" />
          <Button
            variant="ghost"
            size="sm"
            onClick={async () => {
              try {
                const result = await commands.exportTransactionsCsv(filter);
                if (result.status === "ok" && result.data) {
                  toast.success("Exported", { description: result.data });
                }
              } catch {
                toast.error("Export failed");
              }
            }}
          >
            ↓ CSV
          </Button>
          <Button variant="primary" onClick={() => setAddOpen(true)}>+ Add transaction</Button>
        </div>
      </header>

      {/* Search */}
      <Input
        className="screen-search"
        type="search"
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        placeholder="Search transactions…"
        style={{ marginBottom: 12 }}
        aria-label="Search transactions"
      />

      {/* Filter tabs */}
      <div className="toolbar" style={{ marginBottom: 20, display: "inline-flex" }}>
        {TABS.map((tab) => (
          <button
            key={tab.key}
            className={preset === tab.key ? "on" : ""}
            aria-pressed={preset === tab.key}
            onClick={() => setPreset(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {(!data || data.length === 0) ? (
        <EmptyState
          title="No transactions match your filters."
          description="Try adjusting your search or filters."
        />
      ) : (
        <Table wrap={true}>
          <TableHead>
            <tr>
              <TableHeader>Date</TableHeader>
              <TableHeader>Merchant</TableHeader>
              <TableHeader>Category</TableHeader>
              <TableHeader right>Amount</TableHeader>
            </tr>
          </TableHead>
          <TableBody>
            {data.map((t: Transaction) => (
              <tr
                key={t.id}
                style={{ cursor: "pointer" }}
                onClick={() => setEditTxnId(t.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setEditTxnId(t.id);
                  }
                }}
                tabIndex={0}
                role="button"
                aria-label={`Edit transaction ${t.merchant_raw}`}
              >
                <TableCell>{formatDate(t.posted_at)}</TableCell>
                <TableCell>
                  <div className="row row-md">
                    <span
                      aria-label={`${t.merchant_label ?? t.merchant_raw} merchant tile`}
                      style={{
                        width: 28, height: 28, borderRadius: 6,
                        background: t.merchant_color ?? "var(--surface-2)",
                        color: "var(--accent-ink)",
                        fontSize: 11, fontWeight: 600,
                        display: "grid", placeItems: "center",
                      }}
                    >
                      {t.merchant_initials ?? "?"}
                    </span>
                    <span>{t.merchant_label ?? t.merchant_raw}</span>
                    {t.is_reimbursable && <Badge>Reimbursable</Badge>}
                    {t.is_split && <Badge>Split</Badge>}
                  </div>
                </TableCell>
                <TableCell>{t.category_label ?? "Uncategorized"}</TableCell>
                <TableCell right>
                  <span className="money">{money(t.amount_cents, { decimals: 2 })}</span>
                </TableCell>
              </tr>
            ))}
          </TableBody>
        </Table>
      )}

      <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <TransactionDrawer
        open={editTxn !== null}
        onClose={() => setEditTxnId(null)}
        transaction={editTxn ?? undefined}
      />
      {csvPath && (
        <ImportMappingDialog
          path={csvPath}
          onClose={() => setCsvPath(null)}
          onImported={() => setCsvPath(null)}
        />
      )}
    </div>
  );
}

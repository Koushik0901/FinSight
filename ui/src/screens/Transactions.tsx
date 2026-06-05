import { useState, useEffect, useRef } from "react";
import { useTransactions } from "../api/hooks/transactions";
import TransactionDrawer from "../components/TransactionDrawer";
import FilePicker from "../components/FilePicker";
import ImportMappingDialog from "./onboarding/ImportMappingDialog";
import type { Transaction, TxnFilterInput } from "../api/client";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

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
  const [editTxn, setEditTxn] = useState<Transaction | null>(null);
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

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;

  return (
    <div className="screen-transactions">
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <h1 style={{ fontSize: 32, fontWeight: 600, margin: 0 }}>Transactions</h1>
        <div className="actions" style={{ display: "flex", gap: 8 }}>
          <FilePicker onPicked={setCsvPath} label="Import CSV" />
          <button className="primary" onClick={() => setAddOpen(true)}>+ Add transaction</button>
        </div>
      </header>

      {/* Search */}
      <input
        type="search"
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        placeholder="Search transactions…"
        style={{
          width: "100%",
          background: "var(--surface-2)",
          border: "1px solid var(--line)",
          borderRadius: 8,
          padding: "8px 14px",
          fontSize: 14,
          color: "var(--ink)",
          outline: "none",
          marginBottom: 12,
          boxSizing: "border-box",
        }}
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
        <div className="stub">No transactions match your filters.</div>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Date</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Merchant</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Category</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Amount</th>
            </tr>
          </thead>
          <tbody>
            {data.map((t: Transaction) => (
              <tr
                key={t.id}
                style={{ borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                onClick={() => setEditTxn(t)}
                aria-label={`Edit transaction ${t.merchant_raw}`}
              >
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{formatDate(t.posted_at)}</td>
                <td style={{ padding: "12px 0" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
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
                    {t.is_reimbursable && <span className="chip" style={{ marginLeft: 6, fontSize: 10 }}>Reimbursable</span>}
                    {t.is_split && <span className="chip" style={{ marginLeft: 6, fontSize: 10 }}>Split</span>}
                  </div>
                </td>
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>
                  {t.category_label ?? "Uncategorized"}
                </td>
                <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                  <span className="money">{formatMoney(t.amount_cents)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <TransactionDrawer
        open={editTxn !== null}
        onClose={() => setEditTxn(null)}
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

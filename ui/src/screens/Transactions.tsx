import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useTransactions, useCategoriesWithSpending } from "../api/hooks/transactions";
import { useNeedsReviewCount, useAgentStatus } from "../api/hooks/agent";
import type { Transaction } from "../api/client";
import { commands } from "../api/client";
import TransactionDrawer from "../components/TransactionDrawer";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

function avatarColor(name: string) {
  let hash = 0;
  for (let i = 0; i < name.length; i += 1) hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
  const colors = ["var(--c-housing)", "var(--c-groceries)", "var(--c-dining)", "var(--c-transport)", "var(--c-travel)", "var(--c-shopping)"];
  return colors[Math.abs(hash) % colors.length] || "var(--accent)";
}

function avatarText(name: string) {
  return name.replace(/[^A-Za-z0-9]/g, "").slice(0, 2).toUpperCase() || "TX";
}

export default function Transactions() {
  const { data: accounts = [] } = useAccounts();
  const { data: categories = [] } = useCategoriesWithSpending();
  const { data: rows = [], isLoading, error } = useTransactions();
  const { data: needsReviewCount = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const [query, setQuery] = useState("");
  const [preset, setPreset] = useState<"all" | "review" | "anomalies">("all");
  const [addOpen, setAddOpen] = useState(false);
  const [editTxnId, setEditTxnId] = useState<string | null>(null);

  const accountNameById = useMemo(() => Object.fromEntries(accounts.map((account) => [account.id, getAccountDisplayName(account)])), [accounts]);
  const accountColorById = useMemo(() => Object.fromEntries(accounts.map((account) => [account.id, account.color])), [accounts]);
  const categoryById = useMemo(() => Object.fromEntries(categories.map((category) => [category.id, category])), [categories]);

  const filtered = useMemo(() => {
    const lower = query.trim().toLowerCase();
    return rows.filter((transaction) => {
      if (preset === "review" && !(transaction.ai_confidence !== null && transaction.ai_confidence < 0.6)) return false;
      if (preset === "anomalies" && !transaction.is_anomaly) return false;
      if (!lower) return true;
      const haystack = [
        transaction.merchant_raw,
        transaction.merchant_label,
        transaction.notes,
        transaction.category_label,
        accountNameById[transaction.account_id],
        money(transaction.amount_cents, { currency: "USD", decimals: 2 }),
      ].join(" ").toLowerCase();
      return haystack.includes(lower);
    });
  }, [accountNameById, preset, query, rows]);

  const editTxn = filtered.find((transaction) => transaction.id === editTxnId) ?? rows.find((transaction) => transaction.id === editTxnId) ?? null;
  const indexedLabel = new Date().toLocaleDateString("en-US", { month: "long", year: "numeric" }).toUpperCase();

  if (isLoading) return <div className="stub">Loading transactions…</div>;
  if (error) return <div className="stub" role="alert">Error loading transactions.</div>;

  return (
    <div className="screen screen-transactions">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />TRANSACTIONS · {indexedLabel} · {filtered.length.toLocaleString()} INDEXED</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Every line of activity, searchable.</h1>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}>
          <button
            className="btn outline sm"
            type="button"
            onClick={async () => {
              try {
                const result = await commands.exportTransactionsCsv({
                  accountId: null,
                  limit: null,
                  offset: null,
                  search: query || null,
                  filterPreset: preset === "all" ? null : preset === "review" ? "needs_review" : "anomalies",
                  startDate: null,
                  endDate: null,
                });
                if (result.status === "ok" && result.data) toast.success("Exported", { description: result.data });
              } catch {
                toast.error("Export failed");
              }
            }}
          >
            Export
          </button>
          <button className="btn primary sm" type="button" onClick={() => setAddOpen(true)}>Add manual</button>
        </div>
      </div>

      <div style={{ marginTop: 14, display: "flex", gap: 10, alignItems: "center" }}>
        <div style={{ flex: 1, display: "flex", alignItems: "center", gap: 10, padding: "8px 14px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10 }}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--ink-faint)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
          <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search by merchant, note, amount, or category…" aria-label="Search transactions" style={{ flex: 1, background: "transparent", border: 0, outline: 0, fontSize: 13.5, color: "var(--ink)" }} />
        </div>
        <div className="toolbar">
          <button className={preset === "all" ? "on" : ""} type="button" onClick={() => setPreset("all")}>All</button>
          <button className={preset === "review" ? "on" : ""} type="button" onClick={() => setPreset("review")}>Needs review {needsReviewCount > 0 ? needsReviewCount : ""}</button>
          <button className={preset === "anomalies" ? "on" : ""} type="button" onClick={() => setPreset("anomalies")}>Anomalies {agentStatus?.anomalyCount ? agentStatus.anomalyCount : ""}</button>
        </div>
      </div>

      <div className="section">
        <div className="card flush">
          <table className="tbl">
            <thead>
              <tr>
                <th>DATE</th>
                <th>MERCHANT</th>
                <th>CATEGORY</th>
                <th>ACCOUNT</th>
                <th className="right">AMOUNT</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((transaction: Transaction) => {
                const category = transaction.category_id ? categoryById[transaction.category_id] : undefined;
                const merchantName = transaction.merchant_label ?? transaction.merchant_raw;
                const avatarBg = transaction.merchant_color || avatarColor(merchantName);
                return (
                  <tr key={transaction.id} onClick={() => setEditTxnId(transaction.id)} style={{ cursor: "pointer" }}>
                    <td style={{ width: 76 }}><span className="mono faint">{formatDate(transaction.posted_at)}</span></td>
                    <td>
                      <div className="row row-sm" style={{ alignItems: "center" }}>
                        <div aria-hidden="true" style={{ width: 26, height: 26, borderRadius: 7, background: avatarBg, color: "var(--accent-ink)", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 700, flexShrink: 0 }}>{avatarText(merchantName)}</div>
                        <div>
                          <div className="row row-sm wrap" style={{ alignItems: "center" }}>
                            <span>{merchantName}</span>
                            {transaction.ai_confidence !== null && transaction.ai_confidence < 0.6 && <span className="chip warning">Needs review</span>}
                            {transaction.is_split && <span className="chip">Split</span>}
                            {transaction.is_reimbursable && <span className="chip accent">Reimbursable</span>}
                          </div>
                          {transaction.notes && <div className="muted" style={{ fontSize: 12 }}>{transaction.notes}</div>}
                        </div>
                      </div>
                    </td>
                    <td><div className="row row-sm"><span className="cswatch" style={{ background: transaction.category_color || category?.color || "var(--ink-faint)" }} /><span>{transaction.category_label || category?.label || "Uncategorized"}</span></div></td>
                    <td><div className="row row-sm"><span className="cswatch" style={{ background: accountColorById[transaction.account_id] || "var(--ink-faint)" }} /><span className="muted">{accountNameById[transaction.account_id] || "Manual"}</span></div></td>
                    <td className="right"><span className={`figure money ${transaction.amount_cents > 0 ? "pos" : ""}`} style={{ fontSize: 16 }}>{money(transaction.amount_cents, { currency: "USD", decimals: 2 })}</span></td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <TransactionDrawer open={editTxn !== null} onClose={() => setEditTxnId(null)} transaction={editTxn ?? undefined} />
    </div>
  );
}

import { useMemo, useState } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { toast } from "sonner";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useAccounts } from "../api/hooks/accounts";
import { useInfiniteTransactions, useCategoriesWithSpending } from "../api/hooks/transactions";
import { useNeedsReviewCount, useAgentStatus } from "../api/hooks/agent";
import { useSyncSimpleFinAccount } from "../api/hooks/simplefin";
import { commands } from "../api/client";
import type { Transaction, TxnFilterInput } from "../api/client";
import TransactionFilter from "../components/TransactionFilter";
import TransactionDrawer from "../components/TransactionDrawer";
import ImportMappingDialog from "../components/ImportMappingDialog";
import SetBalanceDialog from "../components/SetBalanceDialog";
import { getAccountDisplayName } from "../utils/accounts";
import { money } from "../utils/format";
import { isTauriRuntime, userErrorMessage } from "../utils/runtime";

function formatStamp(value: string | null | undefined) {
  if (!value) return "Never synced";
  return new Date(value).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
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

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export default function AccountTransactions() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { data: accounts = [] } = useAccounts();
  const { data: categories = [] } = useCategoriesWithSpending();
  const { data: needsReviewCount = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const syncAccount = useSyncSimpleFinAccount();

  const [search, setSearch] = useState("");
  const [startDate, setStartDate] = useState<string | null>(null);
  const [endDate, setEndDate] = useState<string | null>(null);
  const [preset, setPreset] = useState<"all" | "needs_review" | "anomalies">("all");
  const [editTxnId, setEditTxnId] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [csvPath, setCsvPath] = useState<string | null>(null);
  const [balanceOpen, setBalanceOpen] = useState(false);

  const account = accounts.find((a) => a.id === id);

  const filterValue: TxnFilterInput = useMemo(
    () => ({
      accountId: id ?? null,
      limit: null,
      offset: null,
      search: search || null,
      filterPreset: preset === "all" ? null : preset,
      startDate,
      endDate,
    }),
    [id, search, preset, startDate, endDate]
  );

  const {
    data: txnPages,
    isLoading,
    error,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useInfiniteTransactions(filterValue);
  const transactions = useMemo(() => txnPages?.pages.flat() ?? [], [txnPages]);

  const categoryById = useMemo(
    () => Object.fromEntries(categories.map((c) => [c.id, c])),
    [categories]
  );

  const handleFilterChange = (next: TxnFilterInput) => {
    setSearch(next.search ?? "");
    setStartDate(next.startDate ?? null);
    setEndDate(next.endDate ?? null);
    setPreset((next.filterPreset as "all" | "needs_review" | "anomalies") ?? "all");
  };

  const handleExport = async () => {
    if (!account) return;
    try {
      const result = await commands.exportTransactionsCsv(filterValue);
      if (result.status === "ok" && result.data) toast.success("Exported", { description: result.data });
    } catch (exportError) {
      toast.error("Export failed", { description: userErrorMessage(exportError, "Try again.") });
    }
  };

  if (isLoading) return <div className="stub">Loading transactions…</div>;
  if (error) return <div className="stub" role="alert">Error loading transactions.</div>;
  if (!account) {
    return (
      <div className="stub" role="alert">
        Account not found.
        <br />
        <Link to="/accounts" className="btn primary sm" style={{ marginTop: 12 }}>Back to accounts</Link>
      </div>
    );
  }

  const editTxn = transactions.find((t) => t.id === editTxnId) ?? null;

  return (
    <div className="screen screen-account-transactions">
      <div className="day-hdr">
        <div>
          <button className="btn ghost sm" type="button" onClick={() => navigate("/accounts")}>← Back to accounts</button>
          <div className="eyebrow" style={{ marginTop: 10 }}><span className="dot" />{account.bank} · {account.type}</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>{getAccountDisplayName(account)}</h1>
        </div>
        <div style={{ textAlign: "right" }}>
          {account.balance_known ? (
            <div className="figure money" style={{ fontSize: 34, color: account.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(account.balance_cents, { currency: account.currency || "USD", decimals: 2 })}</div>
          ) : (
            <div>
              <div className="figure" style={{ fontSize: 22, color: "var(--ink-mute)" }}>Balance not set</div>
              <button className="btn outline sm" type="button" style={{ marginTop: 6 }} onClick={() => setBalanceOpen(true)}>
                Set balance
              </button>
            </div>
          )}
          <div className="row row-sm wrap" style={{ justifyContent: "flex-end", marginTop: 10 }}>
            <span className="chip">Updated {formatStamp(account.last_synced_at)}</span>
            {account.simplefin_account_id && (
              <button className="btn ghost sm" type="button" onClick={async () => {
                try {
                  const result = await syncAccount.mutateAsync(account.id);
                  toast.success(`Synced ${result.added} new transaction${result.added === 1 ? "" : "s"}`);
                } catch (syncError) {
                  toast.error("Sync failed", { description: userErrorMessage(syncError, "Check your bank connection and try again.") });
                }
              }}>Sync now</button>
            )}
            <button className="btn outline sm" type="button" onClick={handleExport}>Export</button>
            <button
              className="btn outline sm"
              type="button"
              onClick={async () => {
                if (!isTauriRuntime()) {
                  toast.error("CSV import requires the desktop app.");
                  return;
                }
                const selected = await openDialog({
                  multiple: false,
                  directory: false,
                  filters: [{ name: "CSV", extensions: ["csv"] }],
                });
                if (typeof selected === "string") setCsvPath(selected);
              }}
            >
              Import
            </button>
            <button className="btn primary sm" type="button" onClick={() => setAddOpen(true)}>Add manual</button>
          </div>
        </div>
      </div>

      <div style={{ marginTop: 14 }}>
        <TransactionFilter
          value={filterValue}
          onChange={handleFilterChange}
          counts={{ review: needsReviewCount, anomalies: agentStatus?.anomalyCount ?? 0 }}
        />
      </div>

      <div className="section">
        <div className="card flush">
          <table className="tbl">
            <thead>
              <tr>
                <th>Date</th>
                <th>Merchant</th>
                <th>Category</th>
                <th className="right">Amount</th>
              </tr>
            </thead>
            <tbody>
              {transactions.length === 0 ? (
                <tr>
                  <td colSpan={4} className="muted" style={{ padding: 24, textAlign: "center" }}>
                    No transactions match your filters.
                  </td>
                </tr>
              ) : (
                transactions.map((transaction) => {
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
                      <td><div className="row row-sm">{transaction.is_transfer ? (
                        <><span className="cswatch" style={{ background: "var(--ink-mute)" }} /><span className="muted">{transaction.transfer_peer_account_name ? `Transfer ${transaction.amount_cents < 0 ? "→" : "←"} ${transaction.transfer_peer_account_name}` : "Transfer"}</span></>
                      ) : (
                        <><span className="cswatch" style={{ background: transaction.category_color || category?.color || "var(--ink-faint)" }} /><span>{transaction.category_label || category?.label || "Uncategorized"}</span></>
                      )}</div></td>
                      <td className="right"><span className={`figure money ${transaction.amount_cents > 0 ? "pos" : ""}`} style={{ fontSize: 16 }}>{money(transaction.amount_cents, { currency: account.currency || "USD", decimals: 2 })}</span></td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
          {(hasNextPage || isFetchingNextPage) && (
            <div style={{ display: "flex", justifyContent: "center", padding: "16px 0" }}>
              <button
                className="btn outline sm"
                type="button"
                disabled={isFetchingNextPage}
                onClick={() => fetchNextPage()}
              >
                {isFetchingNextPage ? "Loading…" : "Load more"}
              </button>
            </div>
          )}
          {!hasNextPage && transactions.length > 0 && (
            <div className="muted" style={{ textAlign: "center", padding: "12px 0", fontSize: 12 }}>
              {transactions.length} transaction{transactions.length === 1 ? "" : "s"} · end of list
            </div>
          )}
        </div>
      </div>

      <TransactionDrawer open={addOpen} onClose={() => setAddOpen(false)} accountId={account.id} />
      <TransactionDrawer open={editTxnId !== null} onClose={() => setEditTxnId(null)} transaction={editTxn ?? undefined} accountId={account.id} />
      {csvPath && account && (
        <ImportMappingDialog
          path={csvPath}
          defaultAccountId={account.id}
          onClose={() => setCsvPath(null)}
          onImported={(summary) => {
            setCsvPath(null);
            // Imported history carries no balance field, so nudge the user to
            // confirm the real balance right away instead of leaving it unset.
            if (summary.rows_imported > 0 && !account.balance_known) {
              setBalanceOpen(true);
            }
          }}
        />
      )}
      <SetBalanceDialog open={balanceOpen} onClose={() => setBalanceOpen(false)} account={account} />
    </div>
  );
}

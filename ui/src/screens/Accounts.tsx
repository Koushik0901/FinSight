import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  useAccounts,
  useAccountBalanceHistory,
  useAccountBalanceSparklines,
} from "../api/hooks/accounts";
import { useTransactions } from "../api/hooks/transactions";
import { useSyncSimpleFinAccount, useSyncAllSimpleFinAccounts } from "../api/hooks/simplefin";
import { useManualAssets, useLiabilities } from "../api/hooks/assets";
import { useNetWorth } from "../api/hooks/networth";
import type { Account, AccountSummary, Liability, ManualAsset } from "../api/client";
import { commands } from "../api/client";
import { money } from "../utils/format";
import { userErrorMessage } from "../utils/runtime";
import { getAccountDisplayName, getAccountTypeColor } from "../utils/accounts";
import AccountDrawer from "../components/AccountDrawer";
import AssetDrawer from "../components/AssetDrawer";
import LiabilityDrawer from "../components/LiabilityDrawer";
import AccountSparkline from "../components/AccountSparkline";
import AccountBalanceChart from "../components/AccountBalanceChart";

function formatStamp(value: string | null | undefined) {
  if (!value) return "Never synced";
  return new Date(value).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

export default function Accounts() {
  const [addOpen, setAddOpen] = useState(false);
  const [editAccount, setEditAccount] = useState<Account | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [assetAddOpen, setAssetAddOpen] = useState(false);
  const [editAsset, setEditAsset] = useState<ManualAsset | null>(null);
  const [liabAddOpen, setLiabAddOpen] = useState(false);
  const [editLiab, setEditLiab] = useState<Liability | null>(null);

  const { data: accounts = [], isLoading, error } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const { data: liabilities = [] } = useLiabilities();
  const syncAccount = useSyncSimpleFinAccount();
  const syncAll = useSyncAllSimpleFinAccounts();
  const netWorth = useNetWorth();
  const { data: sparklines = [] } = useAccountBalanceSparklines(90);
  const sparklineById = useMemo(
    () => Object.fromEntries(sparklines.map((s) => [s.accountId, s.points])),
    [sparklines]
  );

  useEffect(() => {
    if (!selectedId && accounts.length > 0) setSelectedId(accounts[0]!.id);
  }, [accounts, selectedId]);

  const selectedAccount = accounts.find((account) => account.id === selectedId) ?? accounts[0] ?? null;
  const { data: balanceHistory = [] } = useAccountBalanceHistory(
    selectedAccount?.id,
    90
  );
  const txFilter = useMemo(() => ({
    accountId: selectedAccount?.id ?? null,
    limit: null,
    offset: null,
    search: null,
    filterPreset: null,
    startDate: null,
    endDate: null,
  }), [selectedAccount?.id]);
  const { data: recentTransactions = [] } = useTransactions(txFilter);

  const connectedAssets = accounts.filter((account) => account.balance_cents >= 0).reduce((sum, account) => sum + account.balance_cents, 0);
  const connectedLiabilities = accounts.filter((account) => account.balance_cents < 0).reduce((sum, account) => sum + Math.abs(account.balance_cents), 0);
  const manualAssetsTotal = assets.reduce((sum, asset) => sum + asset.valueCents, 0);
  const liabilitiesTotal = liabilities.reduce((sum, liability) => sum + liability.balanceCents, 0);
  const lastSyncLabel = accounts.map((account) => account.last_synced_at).filter(Boolean).sort().slice(-1)[0] ?? null;
  const hasSimpleFin = accounts.some((account) => account.simplefin_account_id);

  if (isLoading) return <div className="stub">Loading accounts…</div>;
  if (error) return <div className="stub" role="alert">{userErrorMessage(error, "Could not load accounts.")}</div>;

  return (
    <div className="screen screen-accounts">
      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />ACCOUNTS · {accounts.length} CONNECTED</div>
          <h1 className="h1" style={{ marginTop: 6 }}>Everything in one place.</h1>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}>
          <button
            className="btn outline sm"
            type="button"
            onClick={async () => {
              if (hasSimpleFin) {
                try {
                  await syncAll.mutateAsync();
                  toast.success("Synced all SimpleFin accounts");
                } catch (syncError) {
                  toast.error("Sync failed", { description: userErrorMessage(syncError, "Check your bank connection and try again.") });
                }
              } else {
                setAddOpen(true);
              }
            }}
          >
            Connect bank
          </button>
          <button className="btn primary sm" type="button" onClick={() => setAssetAddOpen(true)}>Add manual asset</button>
        </div>
      </header>

      <div className="stat-row">
        <div className="stat"><div className="label">Assets · connected</div><div className="value money">{money(connectedAssets, { currency: "USD" })}</div><div className="sub">{accounts.filter((account) => account.balance_cents >= 0).length} connected</div></div>
        <div className="stat"><div className="label">Assets · manual</div><div className="value money">{money(manualAssetsTotal, { currency: "USD" })}</div><div className="sub">{assets.length} tracked manually</div></div>
        <div className="stat"><div className="label">Liability total</div><div className="value money">{money(liabilitiesTotal || connectedLiabilities, { currency: "USD" })}</div><div className="sub">Debt and payoff accounts</div></div>
        <div className="stat accent"><div className="label">Net worth total</div><div className="value money">{money(netWorth, { currency: "USD" })}</div><div className="sub">Across every balance</div></div>
      </div>

      <div className="section" style={{ display: "grid", gridTemplateColumns: "1.1fr 1.4fr", gap: 18 }}>
        <div className="stack stack-lg">
          <div>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
              <div className="eyebrow"><span className="dot" />CONNECTED</div>
              <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>Synced {formatStamp(lastSyncLabel)}</span>
            </div>
            <div className="card flush">
              {accounts.map((account) => (
                <button
                  key={account.id}
                  type="button"
                  onClick={() => setSelectedId(account.id)}
                  style={{
                    width: "100%",
                    textAlign: "left",
                    display: "grid",
                    gridTemplateColumns: "12px 1fr 72px 120px",
                    gap: 14,
                    alignItems: "center",
                    padding: "14px 16px",
                    borderBottom: "1px solid var(--hairline)",
                    background: selectedAccount?.id === account.id ? "var(--surface-2)" : "transparent",
                  }}
                >
                  <span className="cswatch" style={{ background: account.balance_cents >= 0 ? "var(--positive)" : "var(--negative)" }} />
                  <div>
                    <div>{getAccountDisplayName(account)}</div>
                    <div className="muted" style={{ fontSize: 12 }}>{account.bank} · {account.type}</div>
                  </div>
                  <AccountSparkline
                    points={sparklineById[account.id] ?? []}
                    color={getAccountTypeColor(account.type)}
                  />
                  <div className="figure money" style={{ fontSize: 16, textAlign: "right", color: account.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(account.balance_cents, { currency: account.currency || "USD", decimals: 2 })}</div>
                </button>
              ))}
            </div>
          </div>

          <div>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
              <h2 className="h3">Manual assets</h2>
              <button className="btn ghost sm" type="button" onClick={() => setAssetAddOpen(true)}>+ Add</button>
            </div>
            <div className="card flush">
              {assets.length === 0 ? <div className="muted" style={{ padding: 18 }}>No manual assets yet.</div> : assets.map((asset) => (
                <button key={asset.id} type="button" onClick={() => setEditAsset(asset)} style={{ width: "100%", textAlign: "left", display: "grid", gridTemplateColumns: "1fr auto", gap: 12, padding: "14px 16px", borderBottom: "1px solid var(--hairline)" }}>
                  <div><div>{asset.name}</div><div className="muted" style={{ fontSize: 12 }}>{asset.assetType}</div></div>
                  <span className="money">{money(asset.valueCents, { currency: asset.currency || "USD", decimals: 2 })}</span>
                </button>
              ))}
            </div>
          </div>

          <div>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
              <h2 className="h3">Liabilities</h2>
              <button className="btn ghost sm" type="button" onClick={() => setLiabAddOpen(true)}>+ Add</button>
            </div>
            <div className="card flush">
              {liabilities.length === 0 ? <div className="muted" style={{ padding: 18 }}>No liabilities yet.</div> : liabilities.map((liability) => (
                <button key={liability.id} type="button" onClick={() => setEditLiab(liability)} style={{ width: "100%", textAlign: "left", display: "grid", gridTemplateColumns: "1fr auto", gap: 12, padding: "14px 16px", borderBottom: "1px solid var(--hairline)" }}>
                  <div><div>{liability.name}</div><div className="muted" style={{ fontSize: 12 }}>{liability.liabilityType}{liability.aprPct != null ? ` · ${liability.aprPct}% APR` : ""}</div></div>
                  <span className="money">{money(liability.balanceCents, { currency: liability.currency || "USD" })}</span>
                </button>
              ))}
            </div>
          </div>
        </div>

        <div>
          {selectedAccount && (
            <div className="card" style={{ padding: 0, overflow: "hidden", position: "sticky", top: 16 }}>
              <div style={{ padding: "22px 26px 18px", borderBottom: "1px solid var(--hairline)" }}>
                <div className="mono muted" style={{ fontSize: 12 }}>{selectedAccount.bank.toUpperCase()} · {selectedAccount.type.toUpperCase()} · {(selectedAccount.mask || "••••")}</div>
                <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 12, marginTop: 10 }}>
                  <h2 className="h1" style={{ fontSize: 26 }}>{getAccountDisplayName(selectedAccount)}</h2>
                  <div className="figure money" style={{ fontSize: 34, color: selectedAccount.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(selectedAccount.balance_cents, { currency: selectedAccount.currency || "USD", decimals: 2 })}</div>
                </div>
                <div className="row row-sm wrap" style={{ marginTop: 12 }}>
                  <span className="chip accent">Auto-synced</span>
                  <span className="chip">Updated {formatStamp(selectedAccount.last_synced_at)}</span>
                  {selectedAccount.simplefin_account_id && (
                    <button className="btn ghost sm" type="button" onClick={async () => {
                      try {
                        const result = await syncAccount.mutateAsync(selectedAccount.id);
                        toast.success(`Synced ${result.added} new transaction${result.added === 1 ? "" : "s"}`);
                      } catch (syncError) {
                        toast.error("Sync failed", { description: userErrorMessage(syncError, "Check your bank connection and try again.") });
                      }
                    }}>Sync now</button>
                  )}
                </div>
                <AccountBalanceChart
                  points={balanceHistory}
                  color={getAccountTypeColor(selectedAccount.type)}
                />
              </div>

              <div className="row" style={{ justifyContent: "space-between", alignItems: "center", padding: "14px 22px" }}>
                <div className="eyebrow"><span className="dot" />RECENT ACTIVITY</div>
                <div className="row row-sm">
                  <button className="btn ghost sm" type="button">Filter</button>
                  <button className="btn ghost sm" type="button" onClick={async () => {
                    try {
                      const result = await commands.exportAccountCsv(selectedAccount.id);
                      if (result.status === "ok" && result.data) toast.success("Exported", { description: result.data });
                    } catch (exportError) {
                      toast.error("Export failed", { description: userErrorMessage(exportError, "Try again from the desktop app.") });
                    }
                  }}>Export</button>
                </div>
              </div>

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
                  {recentTransactions.slice(0, 8).map((transaction) => (
                    <tr key={transaction.id}>
                      <td><span className="mono faint">{new Date(transaction.posted_at).toLocaleDateString("en-US", { month: "short", day: "numeric" })}</span></td>
                      <td>{transaction.merchant_label || transaction.merchant_raw}</td>
                      <td><div className="row row-sm"><span className="cswatch" style={{ background: transaction.category_color || "var(--ink-faint)" }} /><span>{transaction.category_label || "Uncategorized"}</span></div></td>
                      <td className="right"><span className={`money ${transaction.amount_cents > 0 ? "pos" : ""}`}>{money(transaction.amount_cents, { currency: selectedAccount.currency || "USD", decimals: 2 })}</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      <AccountDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <AccountDrawer open={editAccount !== null} onClose={() => setEditAccount(null)} account={editAccount ?? undefined} />
      <AssetDrawer open={assetAddOpen} onClose={() => setAssetAddOpen(false)} />
      <AssetDrawer open={editAsset !== null} onClose={() => setEditAsset(null)} asset={editAsset ?? undefined} />
      <LiabilityDrawer open={liabAddOpen} onClose={() => setLiabAddOpen(false)} />
      <LiabilityDrawer open={editLiab !== null} onClose={() => setEditLiab(null)} liability={editLiab ?? undefined} />
    </div>
  );
}

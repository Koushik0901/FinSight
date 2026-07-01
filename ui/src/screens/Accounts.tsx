import { useId, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts, useAccountBalanceHistory } from "../api/hooks/accounts";
import { useSyncAllSimpleFinAccounts } from "../api/hooks/simplefin";
import { useManualAssets, useLiabilities } from "../api/hooks/assets";
import { useNetWorth } from "../api/hooks/networth";
import { useTransactions } from "../api/hooks/transactions";
import type { Account, AccountBalancePoint, Liability, ManualAsset } from "../api/client";
import { money } from "../utils/format";
import { userErrorMessage } from "../utils/runtime";
import { getAccountDisplayName } from "../utils/accounts";
import AccountDrawer from "../components/AccountDrawer";
import AssetDrawer from "../components/AssetDrawer";
import LiabilityDrawer from "../components/LiabilityDrawer";

function formatStamp(value: string | null | undefined) {
  if (!value) return "Never synced";
  return new Date(value).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

function AccountSparkline({ points, color }: { points: AccountBalancePoint[]; color: string }) {
  if (points.length < 2) return null;
  const values = points.map((p) => p.balanceCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = 100 / (points.length - 1);
  const yOf = (v: number) => 20 - ((v - min) / range) * 16;
  const linePts = values.map((v, i) => ({ x: i * stepX, y: yOf(v) }));
  const lineD = linePts.map((pt, i) => `${i === 0 ? "M" : "L"}${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(" ");
  return (
    <svg viewBox="0 0 100 22" preserveAspectRatio="none" style={{ width: "100%", height: 22, display: "block" }}>
      <path d={lineD} fill="none" stroke={color} strokeWidth="1.5" opacity="0.7" />
    </svg>
  );
}

function AccountDetailChart({ points, color }: { points: AccountBalancePoint[]; color: string }) {
  const gradId = useId();
  if (points.length < 2) {
    return <div className="stub">Balance history is still building for this account.</div>;
  }
  const values = points.map((p) => p.balanceCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = 100 / (points.length - 1);
  const yOf = (v: number) => 34 - ((v - min) / range) * 30;
  const linePts = values.map((v, i) => ({ x: i * stepX, y: yOf(v) }));
  const lineD = linePts.map((pt, i) => `${i === 0 ? "M" : "L"}${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(" ");
  const areaD = `${lineD} L100,40 L0,40 Z`;
  return (
    <svg viewBox="0 0 100 40" preserveAspectRatio="none" style={{ width: "100%", height: 80, display: "block" }}>
      <defs>
        <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.34" />
          <stop offset="60%" stopColor={color} stopOpacity="0.06" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={areaD} fill={`url(#${gradId})`} stroke="none" />
      <path d={lineD} fill="none" stroke={color} strokeWidth="1.2" />
    </svg>
  );
}

export default function Accounts() {
  const navigate = useNavigate();
  const [addOpen, setAddOpen] = useState(false);
  const [editAccount, setEditAccount] = useState<Account | null>(null);
  const [assetAddOpen, setAssetAddOpen] = useState(false);
  const [editAsset, setEditAsset] = useState<ManualAsset | null>(null);
  const [liabAddOpen, setLiabAddOpen] = useState(false);
  const [editLiab, setEditLiab] = useState<Liability | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const { data: accounts = [], isLoading, error } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const { data: liabilities = [] } = useLiabilities();
  const syncAll = useSyncAllSimpleFinAccounts();
  const netWorth = useNetWorth();

  const selected = useMemo(
    () => accounts.find((account) => account.id === selectedId) ?? accounts[0] ?? null,
    [accounts, selectedId]
  );

  const { data: history = [] } = useAccountBalanceHistory(selected?.id, 30);
  const { data: recentTxns = [] } = useTransactions({
    accountId: selected?.id ?? null,
    limit: 8,
    offset: null,
    search: null,
    filterPreset: null,
    startDate: null,
    endDate: null,
  });

  const connectedAssets = accounts.filter((account) => account.balance_cents >= 0).reduce((sum, account) => sum + account.balance_cents, 0);
  const connectedLiabilities = accounts.filter((account) => account.balance_cents < 0).reduce((sum, account) => sum + Math.abs(account.balance_cents), 0);
  const manualAssetsTotal = assets.reduce((sum, asset) => sum + asset.valueCents, 0);
  const liabilitiesTotal = liabilities.reduce((sum, liability) => sum + liability.balanceCents, 0);
  const lastSyncLabel = accounts.map((account) => account.last_synced_at).filter(Boolean).sort().slice(-1)[0] ?? null;
  const hasSimpleFin = accounts.some((account) => account.simplefin_account_id);

  const delta30 = history.length >= 2 ? history[history.length - 1]!.balanceCents - history[0]!.balanceCents : null;

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

      <div style={{ display: "grid", gridTemplateColumns: "1.1fr 1.4fr", gap: 18, marginTop: 26 }}>
        {/* LEFT: account lists */}
        <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
          <div>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
              <div className="eyebrow"><span className="dot" />CONNECTED</div>
              <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>Synced {formatStamp(lastSyncLabel)}</span>
            </div>
            <div className="card flush">
              {accounts.length === 0 ? (
                <div className="muted" style={{ padding: 18 }}>No connected accounts yet.</div>
              ) : (
                accounts.map((account, i) => (
                  <button
                    key={account.id}
                    type="button"
                    onClick={() => setSelectedId(account.id)}
                    style={{
                      width: "100%",
                      textAlign: "left",
                      display: "grid",
                      gridTemplateColumns: "12px 1fr 60px 100px",
                      gap: 14,
                      alignItems: "center",
                      padding: "14px 16px",
                      borderBottom: i === accounts.length - 1 ? "none" : "1px solid var(--hairline)",
                      background: selected?.id === account.id ? "var(--surface-2)" : "transparent",
                      cursor: "pointer",
                    }}
                  >
                    <span className="cswatch" style={{ background: account.balance_cents >= 0 ? "var(--positive)" : "var(--negative)" }} />
                    <div>
                      <div>{getAccountDisplayName(account)}</div>
                      <div className="muted" style={{ fontSize: 12 }}>{account.bank} · {account.type}</div>
                    </div>
                    <div style={{ height: 22, opacity: 0.6 }}>
                      {selected?.id === account.id && <AccountSparkline points={history} color={account.color} />}
                    </div>
                    <div className="figure money" style={{ fontSize: 16, textAlign: "right", color: account.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(account.balance_cents, { currency: account.currency || "USD", decimals: 2 })}</div>
                  </button>
                ))
              )}
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
              {liabilities.length === 0 ? <div className="muted" style={{ padding: 18 }}>No liabilities yet.</div> : liabilities.map((liability, i) => {
                const progress = liability.originalBalanceCents
                  ? Math.max(0, Math.min(100, (1 - liability.balanceCents / liability.originalBalanceCents) * 100))
                  : null;
                return (
                  <button key={liability.id} type="button" onClick={() => setEditLiab(liability)} style={{ width: "100%", textAlign: "left", padding: "14px 16px", borderBottom: i === liabilities.length - 1 ? "none" : "1px solid var(--hairline)" }}>
                    <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 12 }}>
                      <div><div>{liability.name}</div><div className="muted" style={{ fontSize: 12 }}>{liability.liabilityType}{liability.aprPct != null ? ` · ${liability.aprPct}% APR` : ""}</div></div>
                      <span className="money">{money(liability.balanceCents, { currency: liability.currency || "USD" })}</span>
                    </div>
                    {progress !== null && (
                      <div style={{ marginTop: 10, height: 4, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                        <div style={{ width: `${progress}%`, height: "100%", background: "var(--accent)", opacity: 0.6 }} />
                      </div>
                    )}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        {/* RIGHT: detail */}
        <div>
          {!selected ? (
            <div className="card" style={{ padding: 24 }}>
              <div className="stub">Select an account to see its details.</div>
            </div>
          ) : (
            <div className="card" style={{ padding: 0, overflow: "hidden" }}>
              <div style={{ padding: "22px 26px 16px", borderBottom: "1px solid var(--hairline)" }}>
                <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 18 }}>
                  <div>
                    <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>{selected.bank.toUpperCase()} · {selected.type.toUpperCase()}</div>
                    <div className="h1" style={{ fontSize: 24, marginTop: 6 }}>{getAccountDisplayName(selected)}</div>
                  </div>
                  <div className="figure money" style={{ fontSize: 34, lineHeight: 1, color: selected.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>
                    {money(selected.balance_cents, { currency: selected.currency || "USD", decimals: 2 })}
                  </div>
                </div>
                <div style={{ marginTop: 12, display: "flex", gap: 8, flexWrap: "wrap" }}>
                  {selected.simplefin_account_id && <span className="chip"><span className="dot" />Auto-synced</span>}
                  {delta30 !== null && (
                    <span className={`chip ${delta30 >= 0 ? "positive" : "negative"}`}>
                      <span className="dot" />{delta30 >= 0 ? "+" : "−"}{money(Math.abs(delta30), { currency: selected.currency || "USD" })} · 30d
                    </span>
                  )}
                </div>
                <div style={{ marginTop: 18 }}>
                  <AccountDetailChart points={history} color={selected.color} />
                </div>
              </div>

              <div style={{ padding: "12px 22px", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div className="eyebrow"><span className="dot" />Recent activity</div>
                <button className="btn ghost sm" type="button" onClick={() => navigate(`/accounts/${selected.id}/transactions`)}>
                  Open full register →
                </button>
              </div>

              <table className="tbl">
                <tbody>
                  {recentTxns.length === 0 && (
                    <tr><td colSpan={3} style={{ textAlign: "center", color: "var(--ink-faint)", padding: 28 }}>No recent activity on this account.</td></tr>
                  )}
                  {recentTxns.map((txn) => (
                    <tr key={txn.id}>
                      <td style={{ width: 90, color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 12.5 }}>
                        {new Date(txn.posted_at).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                      </td>
                      <td>
                        <div style={{ fontSize: 14 }}>{txn.merchant_label || txn.merchant_raw}</div>
                        {txn.category_label && (
                          <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>
                            <span className="cswatch" style={{ background: txn.category_color || "var(--ink-faint)", marginRight: 6 }} />
                            {txn.category_label}
                          </div>
                        )}
                      </td>
                      <td className="right num tabular money" style={{ color: txn.amount_cents > 0 ? "var(--positive)" : "var(--ink)" }}>
                        {money(txn.amount_cents, { currency: selected.currency || "USD", decimals: 2 })}
                      </td>
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

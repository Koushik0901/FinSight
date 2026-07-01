import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useSyncAllSimpleFinAccounts } from "../api/hooks/simplefin";
import { useManualAssets, useLiabilities } from "../api/hooks/assets";
import { useNetWorth } from "../api/hooks/networth";
import type { Account, Liability, ManualAsset } from "../api/client";
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

export default function Accounts() {
  const navigate = useNavigate();
  const [addOpen, setAddOpen] = useState(false);
  const [editAccount, setEditAccount] = useState<Account | null>(null);
  const [assetAddOpen, setAssetAddOpen] = useState(false);
  const [editAsset, setEditAsset] = useState<ManualAsset | null>(null);
  const [liabAddOpen, setLiabAddOpen] = useState(false);
  const [editLiab, setEditLiab] = useState<Liability | null>(null);

  const { data: accounts = [], isLoading, error } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const { data: liabilities = [] } = useLiabilities();
  const syncAll = useSyncAllSimpleFinAccounts();
  const netWorth = useNetWorth();

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

      <div className="section">
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
                onClick={() => navigate(`/accounts/${account.id}/transactions`)}
                style={{
                  width: "100%",
                  textAlign: "left",
                  display: "grid",
                  gridTemplateColumns: "12px 1fr 120px",
                  gap: 14,
                  alignItems: "center",
                  padding: "14px 16px",
                  borderBottom: "1px solid var(--hairline)",
                  background: "transparent",
                  cursor: "pointer",
                }}
              >
                <span className="cswatch" style={{ background: account.balance_cents >= 0 ? "var(--positive)" : "var(--negative)" }} />
                <div>
                  <div>{getAccountDisplayName(account)}</div>
                  <div className="muted" style={{ fontSize: 12 }}>{account.bank} · {account.type}</div>
                </div>
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

      <AccountDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <AccountDrawer open={editAccount !== null} onClose={() => setEditAccount(null)} account={editAccount ?? undefined} />
      <AssetDrawer open={assetAddOpen} onClose={() => setAssetAddOpen(false)} />
      <AssetDrawer open={editAsset !== null} onClose={() => setEditAsset(null)} asset={editAsset ?? undefined} />
      <LiabilityDrawer open={liabAddOpen} onClose={() => setLiabAddOpen(false)} />
      <LiabilityDrawer open={editLiab !== null} onClose={() => setEditLiab(null)} liability={editLiab ?? undefined} />
    </div>
  );
}

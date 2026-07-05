import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useSyncAllSimpleFinAccounts } from "../api/hooks/simplefin";
import { useManualAssets } from "../api/hooks/assets";
import { useAccountOwners, useHouseholdMembers } from "../api/hooks/household";
import { useNetWorth } from "../api/hooks/networth";
import type { AccountSummary, ManualAsset } from "../api/client";
import { money } from "../utils/format";
import { userErrorMessage } from "../utils/runtime";
import { getAccountDisplayName } from "../utils/accounts";
import { accountTypeColor } from "../utils/accountColor";
import AccountDrawer from "../components/AccountDrawer";
import AssetDrawer from "../components/AssetDrawer";

function formatStamp(value: string | null | undefined) {
  if (!value) return "Never synced";
  return new Date(value).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

export default function Accounts() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const [chooserOpen, setChooserOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [editAccount, setEditAccount] = useState<AccountSummary | null>(null);
  const [assetAddOpen, setAssetAddOpen] = useState(false);
  const [editAsset, setEditAsset] = useState<ManualAsset | null>(null);

  const { data: accounts = [], isLoading, error } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const { data: members = [] } = useHouseholdMembers();
  const { data: accountOwners = [] } = useAccountOwners();

  // Owner names per account, and each member's attributed net worth: sole
  // accounts count fully, joint accounts split equally among their owners.
  const ownersByAccount = useMemo(() => {
    const map = new Map<string, typeof members>();
    for (const pair of accountOwners) {
      const member = members.find((m) => m.id === pair.memberId);
      if (!member) continue;
      map.set(pair.accountId, [...(map.get(pair.accountId) ?? []), member]);
    }
    return map;
  }, [accountOwners, members]);

  const attribution = useMemo(() => {
    if (members.length === 0) return [];
    const totals = new Map<string, number>(members.map((m) => [m.id, 0]));
    let household = 0;
    for (const account of accounts) {
      if (!account.balance_known) continue;
      const owners = ownersByAccount.get(account.id) ?? [];
      if (owners.length === 0) {
        household += account.balance_cents;
      } else {
        for (const owner of owners) {
          totals.set(owner.id, (totals.get(owner.id) ?? 0) + account.balance_cents / owners.length);
        }
      }
    }
    const rows = members.map((m) => ({ id: m.id, name: m.name, color: m.color, cents: Math.round(totals.get(m.id) ?? 0) }));
    if (household !== 0) rows.push({ id: "__household__", name: "Household (shared)", color: null, cents: household });
    return rows;
  }, [accounts, members, ownersByAccount]);
  const syncAll = useSyncAllSimpleFinAccounts();
  const netWorth = useNetWorth();

  const knownAccounts = accounts.filter((account) => account.balance_known);
  const unknownBalanceCount = accounts.length - knownAccounts.length;
  // Debt (Credit/Loan accounts) is just an Account with a negative balance —
  // no separate liabilities table anymore.
  const connectedAssets = knownAccounts.filter((account) => account.balance_cents >= 0).reduce((sum, account) => sum + account.balance_cents, 0);
  const connectedLiabilities = knownAccounts.filter((account) => account.balance_cents < 0).reduce((sum, account) => sum + Math.abs(account.balance_cents), 0);
  const manualAssetsTotal = assets.reduce((sum, asset) => sum + asset.valueCents, 0);
  const lastSyncLabel = accounts.map((account) => account.last_synced_at).filter(Boolean).sort().slice(-1)[0] ?? null;
  const hasSimpleFin = accounts.some((account) => account.simplefin_account_id);

  // Deep-link support: ?focusAccount=<id-or-name> opens that account's editor
  // directly (e.g. from a Copilot recommendation about a specific debt).
  const focusedAccount = useMemo(() => {
    const focus = searchParams.get("focusAccount");
    if (!focus) return null;
    return accounts.find((account) =>
      account.id === focus || account.name.toLowerCase() === focus.toLowerCase()
    ) ?? null;
  }, [accounts, searchParams]);
  const activeEditAccount = editAccount ?? focusedAccount;

  useEffect(() => {
    if (!focusedAccount || editAccount) return;
    setEditAccount(focusedAccount);
    const next = new URLSearchParams(searchParams);
    next.delete("focusAccount");
    setSearchParams(next, { replace: true });
  }, [editAccount, focusedAccount, searchParams, setSearchParams]);

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
          <button className="btn primary sm" type="button" aria-label="Add account, asset, or liability" onClick={() => setChooserOpen(true)}>+ Add</button>
          {hasSimpleFin && (
            <button
              className="btn outline sm"
              type="button"
              onClick={async () => {
                try {
                  await syncAll.mutateAsync();
                  toast.success("Synced all SimpleFin accounts");
                } catch (syncError) {
                  toast.error("Sync failed", { description: userErrorMessage(syncError, "Check your bank connection and try again.") });
                }
              }}
            >
              Sync banks
            </button>
          )}
        </div>
      </header>

      <div className="stat-row">
        <div className="stat"><div className="label">Assets · connected</div><div className="value money">{money(connectedAssets, { currency: "USD" })}</div><div className="sub">{knownAccounts.filter((account) => account.balance_cents >= 0).length} connected</div></div>
        <div className="stat"><div className="label">Assets · manual</div><div className="value money">{money(manualAssetsTotal, { currency: "USD" })}</div><div className="sub">{assets.length} tracked manually</div></div>
        <div className="stat"><div className="label">Liability total</div><div className="value money">{money(connectedLiabilities, { currency: "USD" })}</div><div className="sub">Debt and payoff accounts</div></div>
        <div className="stat accent"><div className="label">Net worth total</div><div className="value money">{money(netWorth, { currency: "USD" })}</div><div className="sub">Across every balance</div></div>
      </div>
      {unknownBalanceCount > 0 && (
        <div className="muted" style={{ fontSize: 12.5, marginTop: -8, marginBottom: 16 }} role="status">
          {unknownBalanceCount} account{unknownBalanceCount === 1 ? "" : "s"} {unknownBalanceCount === 1 ? "has" : "have"} no balance set yet — totals above exclude {unknownBalanceCount === 1 ? "it" : "them"}.
        </div>
      )}

      {attribution.length > 0 && (
        <div className="card tight" style={{ marginBottom: 16, padding: 16 }}>
          <div className="eyebrow" style={{ marginBottom: 10 }}><span className="dot" />By owner · joint accounts split equally</div>
          <div className="row wrap" style={{ gap: 20 }}>
            {attribution.map((row) => (
              <div key={row.id} className="row row-sm" style={{ alignItems: "center" }}>
                <span className="cswatch" style={{ background: row.color || "var(--ink-faint)" }} />
                <span style={{ fontSize: 13 }}>{row.name}</span>
                <span className="money strong" style={{ fontSize: 13 }}>{money(row.cents, { currency: "USD" })}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="section">
        <div>
          <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
            <div className="eyebrow"><span className="dot" />CONNECTED</div>
            <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>Synced {formatStamp(lastSyncLabel)}</span>
          </div>
          <div className="card flush">
            {accounts.length === 0 ? (
              <div className="muted" style={{ padding: 18 }}>No connected accounts yet.</div>
            ) : (
              accounts.map((account) => (
                <div
                  key={account.id}
                  style={{
                    display: "grid",
                    gridTemplateColumns: "1fr auto auto",
                    gap: 8,
                    alignItems: "center",
                    borderBottom: "1px solid var(--hairline)",
                    paddingRight: 12,
                  }}
                >
                  <button
                    type="button"
                    onClick={() => navigate(`/accounts/${account.id}/transactions`)}
                    style={{
                      width: "100%",
                      textAlign: "left",
                      display: "grid",
                      gridTemplateColumns: "12px 1fr",
                      gap: 14,
                      alignItems: "center",
                      padding: "14px 16px",
                      background: "transparent",
                      cursor: "pointer",
                      border: "none",
                    }}
                  >
                    <span className="cswatch" style={{ background: accountTypeColor(account.type) }} />
                    <div>
                      <div className="row row-sm" style={{ alignItems: "center" }}>
                        <span>{getAccountDisplayName(account)}</span>
                        {(ownersByAccount.get(account.id)?.length ?? 0) >= 2 && <span className="chip accent" style={{ fontSize: 11 }}>Joint</span>}
                      </div>
                      <div className="muted" style={{ fontSize: 12 }}>
                        {account.bank} · <span style={{ color: accountTypeColor(account.type) }}>{account.type}</span>
                        {(ownersByAccount.get(account.id)?.length ?? 0) > 0 && (
                          <> · {ownersByAccount.get(account.id)!.map((m) => m.name).join(" & ")}</>
                        )}
                      </div>
                    </div>
                  </button>
                  {account.balance_known ? (
                    <div className="figure money" style={{ fontSize: 16, textAlign: "right", color: account.balance_cents < 0 ? "var(--negative)" : "var(--ink)" }}>{money(account.balance_cents, { currency: account.currency || "USD", decimals: 2 })}</div>
                  ) : (
                    <div className="muted" style={{ fontSize: 13, textAlign: "right" }}>Balance not set</div>
                  )}
                  <button
                    type="button"
                    className="btn ghost sm"
                    aria-label={`Edit ${getAccountDisplayName(account)}`}
                    onClick={() => setEditAccount(account)}
                  >
                    Edit
                  </button>
                </div>
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

      </div>

      {chooserOpen && (
        <AddChooserDialog
          onClose={() => setChooserOpen(false)}
          onPick={(kind) => {
            setChooserOpen(false);
            if (kind === "account") setAddOpen(true);
            else setAssetAddOpen(true);
          }}
        />
      )}
      <AccountDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <AccountDrawer open={activeEditAccount !== null} onClose={() => setEditAccount(null)} account={activeEditAccount ?? undefined} />
      <AssetDrawer open={assetAddOpen} onClose={() => setAssetAddOpen(false)} />
      <AssetDrawer open={editAsset !== null} onClose={() => setEditAsset(null)} asset={editAsset ?? undefined} />
    </div>
  );
}

function AddChooserDialog({
  onClose,
  onPick,
}: {
  onClose: () => void;
  onPick: (kind: "account" | "asset") => void;
}) {
  const options = [
    {
      kind: "account" as const,
      title: "Bank account",
      description: "Chequing, savings, credit card, loan, or investment — tracks a balance and optional transactions.",
    },
    {
      kind: "asset" as const,
      title: "Manual asset",
      description: "Something you own — a car, home, or valuables. Tracks a value, no transactions.",
    },
  ];
  return (
    <>
      <div className="dialog-backdrop" onClick={onClose} aria-hidden="true" />
      <div
        className="dialog-overlay compact"
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-chooser-title"
        onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
      >
        <header style={{ marginBottom: 14 }}>
          <span className="eyebrow">Add to net worth</span>
          <h2 id="add-chooser-title" style={{ marginTop: 6 }}>What do you want to add?</h2>
        </header>
        <div className="stack stack-sm">
          {options.map((option) => (
            <button
              key={option.kind}
              type="button"
              className="card tight"
              onClick={() => onPick(option.kind)}
              style={{ textAlign: "left", padding: 16, cursor: "pointer", width: "100%" }}
            >
              <div className="strong" style={{ marginBottom: 4 }}>{option.title}</div>
              <div className="muted" style={{ fontSize: 12.5 }}>{option.description}</div>
            </button>
          ))}
        </div>
        <div className="row" style={{ justifyContent: "flex-end", marginTop: 14 }}>
          <button className="btn ghost sm" type="button" onClick={onClose}>Cancel</button>
        </div>
      </div>
    </>
  );
}

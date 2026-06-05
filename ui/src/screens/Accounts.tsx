import { useState } from "react";
import { useAccounts } from "../api/hooks/accounts";
import AccountDrawer from "../components/AccountDrawer";
import type { Account, AccountSummary } from "../api/client";
import { useManualAssets, useLiabilities } from "../api/hooks/assets";
import AssetDrawer from "../components/AssetDrawer";
import LiabilityDrawer from "../components/LiabilityDrawer";
import type { ManualAsset, Liability } from "../api/client";
import { useNetWorth } from "../api/hooks/networth";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

export default function Accounts() {
  const [addOpen, setAddOpen] = useState(false);
  const [editAccount, setEditAccount] = useState<Account | null>(null);
  const { data, isLoading, error } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const [assetAddOpen, setAssetAddOpen] = useState(false);
  const [editAsset, setEditAsset] = useState<ManualAsset | null>(null);
  const { data: liabilities = [] } = useLiabilities();
  const [liabAddOpen, setLiabAddOpen] = useState(false);
  const [editLiab, setEditLiab] = useState<Liability | null>(null);
  const netWorth = useNetWorth();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;

  return (
    <div className="screen-accounts">
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 24 }}>
        <div>
          <div className="eyebrow" style={{ marginBottom: 6 }}>Net worth</div>
          <div className="figure money" style={{ fontSize: 40, lineHeight: 1, color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}>
            {formatMoney(netWorth)}
          </div>
          <h1 style={{ fontSize: 20, fontWeight: 600, margin: "12px 0 0" }}>Accounts</h1>
        </div>
        <button className="primary" onClick={() => setAddOpen(true)}>+ Add account</button>
      </header>

      {(!data || data.length === 0) ? (
        <div className="stub">No accounts yet.</div>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Bank</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Name</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Type</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Balance</th>
            </tr>
          </thead>
          <tbody>
            {data.map((a: AccountSummary) => (
              <tr
                key={a.id}
                style={{ borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                onClick={() => setEditAccount(a as unknown as Account)}
                aria-label={`Edit ${a.name}`}
              >
                <td style={{ padding: "12px 0" }}>{a.bank}</td>
                <td style={{ padding: "12px 0" }}>{a.name}</td>
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{a.type}</td>
                <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                  <span className="money">{formatMoney(a.balance_cents)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <section style={{ marginTop: 40 }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Manual assets</h2>
          <button onClick={() => setAssetAddOpen(true)}>+ Add manual asset</button>
        </header>
        {assets.length === 0 ? (
          <div className="stub">No manual assets yet.</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column" }}>
            {assets.map((a) => (
              <div
                key={a.id}
                role="button"
                tabIndex={0}
                onClick={() => setEditAsset(a)}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setEditAsset(a); } }}
                aria-label={`Edit ${a.name}`}
                style={{ display: "flex", alignItems: "center", gap: 12, padding: "12px 0", borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
              >
                <span style={{ width: 28, height: 28, borderRadius: 7, background: "var(--surface-2)", display: "flex", alignItems: "center", justifyContent: "center", fontSize: 13, textTransform: "uppercase", flexShrink: 0 }}>
                  {a.assetType.charAt(0)}
                </span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 14 }}>{a.name}</div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {a.assetType} · updated {new Date(a.updatedAt).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                  </div>
                </div>
                <span className="money" style={{ fontFamily: "var(--mono)", fontSize: 14 }}>{formatMoney(a.valueCents)}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      <section style={{ marginTop: 40 }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Liabilities</h2>
          <button onClick={() => setLiabAddOpen(true)}>+ Add liability</button>
        </header>
        {liabilities.length === 0 ? (
          <div className="stub">No liabilities yet.</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column" }}>
            {liabilities.map((l) => {
              const pct = l.limitCents && l.limitCents > 0
                ? Math.min(100, (l.balanceCents / l.limitCents) * 100) : null;
              return (
                <div
                  key={l.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => setEditLiab(l)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setEditLiab(l); } }}
                  aria-label={`Edit ${l.name}`}
                  style={{ padding: "12px 0", borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 14 }}>{l.name}</div>
                      <div className="muted" style={{ fontSize: 12 }}>
                        <span className="chip" style={{ marginRight: 8 }}>{l.liabilityType}</span>
                        {l.aprPct != null && <>{l.aprPct}% APR</>}
                        {l.payoffDate && <> · payoff {new Date(l.payoffDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}</>}
                      </div>
                    </div>
                    <span className="money" style={{ fontFamily: "var(--mono)", fontSize: 14, color: "var(--negative)" }}>
                      {formatMoney(l.balanceCents)}
                    </span>
                  </div>
                  {pct != null && (
                    <div style={{ height: 4, background: "var(--surface-2)", borderRadius: 999, marginTop: 8 }}>
                      <div style={{ width: `${pct}%`, height: "100%", background: "var(--negative)", borderRadius: 999 }} />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </section>

      <LiabilityDrawer open={liabAddOpen} onClose={() => setLiabAddOpen(false)} />
      <LiabilityDrawer open={editLiab !== null} onClose={() => setEditLiab(null)} liability={editLiab ?? undefined} />

      <AssetDrawer open={assetAddOpen} onClose={() => setAssetAddOpen(false)} />
      <AssetDrawer open={editAsset !== null} onClose={() => setEditAsset(null)} asset={editAsset ?? undefined} />

      <AccountDrawer open={addOpen} onClose={() => setAddOpen(false)} />
      <AccountDrawer
        open={editAccount !== null}
        onClose={() => setEditAccount(null)}
        account={editAccount ?? undefined}
      />
    </div>
  );
}

import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useAccounts } from "../../api/hooks/accounts";
import { useManualAssets } from "../../api/hooks/assets";
import { useCurrencyScope } from "../../api/hooks/currencyScope";
import { useNetWorth } from "../../api/hooks/networth";
import { money } from "../../utils/format";
import { getAccountDisplayName } from "../../utils/accounts";
import { accountTypeColor } from "../../utils/accountColor";
import type { AccountSummary } from "../../api/openapiClient";
import * as I from "../../components/Icons";
import { MobileStat, MobileStatRow } from "../../components/mobile/MobileStat";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import AccountDrawer from "../../components/AccountDrawer";
import AssetDrawer from "../../components/AssetDrawer";
import { UnconvertedCurrencies } from "../../components/UnconvertedCurrencies";

export default function MobileAccounts() {
  const navigate = useNavigate();
  const { data: accounts = [], isLoading } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const netWorth = useNetWorth();
  const { currency: scopeCurrency, unconverted, inScope } = useCurrencyScope();
  const [detailAccount, setDetailAccount] = useState<AccountSummary | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [assetEditOpen, setAssetEditOpen] = useState(false);

  const knownAccounts = accounts.filter((a) => a.balance_known && inScope(a.currency));
  const primaryCurrency = scopeCurrency ?? accounts[0]?.currency ?? "USD";

  const grouped = useMemo(() => {
    const cash = knownAccounts.filter((a) => ["checking", "savings", "cash"].includes(a.type.toLowerCase()));
    const credit = knownAccounts.filter((a) => a.balance_cents < 0);
    const invest = knownAccounts.filter((a) => ["investment", "brokerage", "retirement"].includes(a.type.toLowerCase()));
    const other = knownAccounts.filter((a) => !cash.includes(a) && !credit.includes(a) && !invest.includes(a));
    return { cash, credit, invest, other };
  }, [knownAccounts]);

  const totals = useMemo(() => {
    const assetsTotal = knownAccounts.filter((a) => a.balance_cents >= 0).reduce((s, a) => s + a.balance_cents, 0);
    const liabilitiesTotal = knownAccounts.filter((a) => a.balance_cents < 0).reduce((s, a) => s + Math.abs(a.balance_cents), 0);
    const manualTotal = assets.filter((ass) => inScope(ass.currency)).reduce((s, ass) => s + ass.valueCents, 0);
    return { assetsTotal, liabilitiesTotal, manualTotal };
  }, [knownAccounts, assets, inScope]);

  if (isLoading) return <div className="stub"><span className="spinner" aria-hidden="true" /> Loading…</div>;

  if (accounts.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <MobileEmptyState
          icon={<I.Wallet width={28} height={28} />}
          title="No accounts yet"
          description="Add your checking, savings, credit, and investment accounts to see your total position."
          primaryAction={<button className="btn primary" type="button" onClick={() => setEditOpen(true)}>Add account</button>}
        />
        <AccountDrawer open={editOpen} onClose={() => setEditOpen(false)} />
      </div>
    );
  }

  const renderGroup = (title: string, items: AccountSummary[]) => {
    if (items.length === 0) return null;
    return (
      <MobileSection title={title}>
        <MobileList ariaLabel={title}>
          {items.map((a) => (
            <MobileListItem
              key={a.id}
              icon={<span style={{ width: 12, height: 12, borderRadius: "50%", background: accountTypeColor(a.type), display: "inline-block" }} />}
              title={getAccountDisplayName(a)}
              subtitle={`${a.type} · ${a.currency} · ${a.last_synced_at ? new Date(a.last_synced_at).toLocaleDateString("en-US", { month: "short", day: "numeric" }) : "No sync"}`}
              value={a.balance_known ? money(a.balance_cents, { currency: a.currency }) : "—"}
              valueTone={a.balance_cents < 0 ? "negative" : "default"}
              onPress={() => setDetailAccount(a)}
            />
          ))}
        </MobileList>
      </MobileSection>
    );
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      {/* Total position */}
      <div className="mobile-stat hero" style={{ padding: 16 }}>
        <span className="mobile-stat-label">Total net worth</span>
        <span className="mobile-stat-value lg money" style={{ color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}>
          {money(netWorth, { currency: primaryCurrency })}
        </span>
        <span className="mobile-stat-sub">
          {knownAccounts.length} accounts with known balance · {assets.length} manual assets
        </span>
        {unconverted.length > 0 ? <UnconvertedCurrencies holdings={unconverted} primary={scopeCurrency} /> : null}
      </div>

      <MobileStatRow>
        <MobileStat label="Assets" value={money(totals.assetsTotal + totals.manualTotal, { currency: primaryCurrency })} sub="Cash + investments + manual" />
        <MobileStat label="Liabilities" value={money(totals.liabilitiesTotal, { currency: primaryCurrency })} sub="Credit & loans" />
      </MobileStatRow>

      {renderGroup("Cash & checking", grouped.cash)}
      {renderGroup("Credit & liabilities", grouped.credit)}
      {renderGroup("Investments", grouped.invest)}
      {renderGroup("Other", grouped.other)}

      {assets.length > 0 ? (
        <MobileSection title="Manual assets" description="Homes, vehicles, other holdings" actionLabel="Add" onAction={() => setAssetEditOpen(true)}>
          <MobileList ariaLabel="Manual assets">
            {assets.map((ass) => (
              <MobileListItem
                key={ass.id}
                icon={<I.House width={14} height={14} />}
                title={ass.name}
                subtitle={`${ass.currency} · ${money(ass.valueCents, { currency: ass.currency })}`}
                value={money(ass.valueCents, { currency: ass.currency })}
                onPress={() => setAssetEditOpen(true)}
              />
            ))}
          </MobileList>
        </MobileSection>
      ) : (
        <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface)", display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
          <div>
            <strong style={{ display: "block", fontSize: 14 }}>Manual assets</strong>
            <span style={{ fontSize: 12, color: "var(--ink-mute)" }}>Track homes, cars, and other holdings</span>
          </div>
          <button className="btn sm" type="button" onClick={() => setAssetEditOpen(true)}>Add</button>
        </div>
      )}

      <button type="button" className="btn" style={{ width: "100%", minHeight: 44 }} onClick={() => setEditOpen(true)}>
        <I.Plus width={14} height={14} /> Add account
      </button>

      <BottomSheet open={!!detailAccount} onClose={() => setDetailAccount(null)} title={detailAccount ? getAccountDisplayName(detailAccount) : "Account"}>
        {detailAccount ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span style={{ width: 44, height: 44, borderRadius: 12, background: "var(--surface-2)", border: "1px solid var(--hairline)", display: "grid", placeItems: "center" }}>
                <span style={{ width: 12, height: 12, borderRadius: "50%", background: accountTypeColor(detailAccount.type), display: "inline-block" }} />
              </span>
              <div>
                <div style={{ fontWeight: 650, fontSize: 16, lineHeight: 1.2 }}>{getAccountDisplayName(detailAccount)}</div>
                <div style={{ fontSize: 12, color: "var(--ink-mute)" }}>{detailAccount.type} · {detailAccount.currency}</div>
              </div>
            </div>

            <div style={{ padding: 16, border: "1px solid var(--line)", borderRadius: 16, background: "var(--surface)" }}>
              <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Balance</div>
              <div className="money" style={{ fontSize: 28, fontWeight: 750, letterSpacing: "-0.03em", color: detailAccount.balance_cents < 0 ? "var(--negative)" : "var(--ink)", marginTop: 4 }}>
                {detailAccount.balance_known ? money(detailAccount.balance_cents, { currency: detailAccount.currency }) : "Balance not set"}
              </div>
              <div style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 6 }}>
                {detailAccount.last_synced_at ? `Synced ${new Date(detailAccount.last_synced_at).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}` : "Never synced"}
              </div>
            </div>

            <div style={{ display: "flex", gap: 10 }}>
              <button type="button" className="btn primary" style={{ flex: 1, minHeight: 44 }} onClick={() => navigate(`/accounts/${detailAccount.id}/transactions`)}>
                View transactions
              </button>
              <button type="button" className="btn" style={{ flex: 1, minHeight: 44 }} onClick={() => { setEditOpen(true); }}>
                Edit
              </button>
            </div>

            <button type="button" className="btn ghost" style={{ width: "100%", minHeight: 44 }} onClick={() => setDetailAccount(null)}>
              Close
            </button>
          </div>
        ) : null}
      </BottomSheet>

      <AccountDrawer open={editOpen} onClose={() => setEditOpen(false)} />
      <AssetDrawer open={assetEditOpen} onClose={() => setAssetEditOpen(false)} />
    </div>
  );
}

import { useState } from "react";
import { useAccounts } from "../api/hooks/accounts";
import AccountDrawer from "../components/AccountDrawer";
import type { Account, AccountSummary } from "../api/client";
import { commands } from "../api/client";
import { useSyncSimpleFinAccount } from "../api/hooks/simplefin";
import { useManualAssets, useLiabilities } from "../api/hooks/assets";
import AssetDrawer from "../components/AssetDrawer";
import LiabilityDrawer from "../components/LiabilityDrawer";
import type { ManualAsset, Liability } from "../api/client";
import { useNetWorth } from "../api/hooks/networth";
import { money } from "../utils/format";
import { toast } from "sonner";
import { userErrorMessage } from "../utils/runtime";
import Button from "../components/Button";
import Card from "../components/Card";
import EmptyState from "../components/EmptyState";
import ProgressBar from "../components/ProgressBar";
import Badge from "../components/Badge";
import Table from "../components/Table";
import { TableHead, TableBody, TableRow, TableHeader, TableCell } from "../components/Table";

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
  const syncAccount = useSyncSimpleFinAccount();

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        Loading…
      </div>
    );
  }

  if (error) {
    return (
      <div className="empty-state" role="alert" aria-live="assertive">
        <section className="empty-panel">
          <div className="eyebrow">Accounts unavailable</div>
          <h2>We could not load your accounts.</h2>
          <p>{userErrorMessage(error, "Open the desktop app runtime and try again.")}</p>
        </section>
      </div>
    );
  }

  return (
    <div className="screen-accounts">
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="eyebrow">Net worth</div>
          <div
            className="figure money"
            style={{ fontSize: 40, lineHeight: 1, color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}
          >
            {money(netWorth)}
          </div>
          <h1 style={{ fontSize: 20, fontWeight: 600, margin: "12px 0 0" }}>Accounts</h1>
        </div>
        <Button variant="primary" onClick={() => setAddOpen(true)}>
          + Add account
        </Button>
      </header>

      {(!data || data.length === 0) ? (
        <EmptyState
          title="Add an account to start tracking net worth."
          description="Start with a checking, savings, credit, investment, or cash account. You can import transactions after the account exists."
          actions={
            <>
              <Button variant="primary" onClick={() => setAddOpen(true)}>
                Add account
              </Button>
              <Button onClick={() => setAssetAddOpen(true)}>Add asset</Button>
              <Button variant="ghost" onClick={() => setLiabAddOpen(true)}>
                Add liability
              </Button>
            </>
          }
        />
      ) : (
        <Table>
          <TableHead>
            <tr>
              <TableHeader>Bank</TableHeader>
              <TableHeader>Name</TableHeader>
              <TableHeader>Type</TableHeader>
              <TableHeader right>Balance</TableHeader>
              <TableHeader>{""}</TableHeader>
            </tr>
          </TableHead>
          <TableBody>
            {data.map((a: AccountSummary) => (
              <TableRow
                key={a.id}
                onClick={() => setEditAccount(a as unknown as Account)}
                aria-label={`Edit ${a.name}`}
              >
                <TableCell>{a.bank}</TableCell>
                <TableCell>{a.name}</TableCell>
                <TableCell>
                  <span className="muted">{a.type}</span>
                  {a.type === "Savings" && a.apy_pct != null && (
                    <span style={{ marginLeft: 8 }}>
                      <Badge className="chip">{a.apy_pct}% APY</Badge>
                    </span>
                  )}
                </TableCell>
                <TableCell right>
                  <span className="num tabular money">{money(a.balance_cents, { decimals: 2 })}</span>
                </TableCell>
                <TableCell right>
                  <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
                    {a.simplefin_account_id && (
                      <Button
                        variant="ghost"
                        size="sm"
                        title="Sync from SimpleFin"
                        loading={syncAccount.isPending}
                        onClick={async (e) => {
                          e.stopPropagation();
                          try {
                            const result = await syncAccount.mutateAsync(a.id);
                            toast.success(
                              `Synced: ${result.added} added, ${result.skipped} skipped`,
                            );
                          } catch (err) {
                            toast.error("Sync failed", {
                              description: userErrorMessage(err, "Check your SimpleFin connection and try again."),
                            });
                          }
                        }}
                      >
                        ↻ Sync
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      title="Export transactions as CSV"
                      onClick={async (e) => {
                        e.stopPropagation();
                        try {
                          const result = await commands.exportAccountCsv(a.id);
                          if (result.status === "ok" && result.data) {
                            toast.success("Exported", { description: result.data });
                          }
                        } catch (err) {
                          toast.error("Export failed", {
                            description: userErrorMessage(err, "Try exporting again from the desktop app."),
                          });
                        }
                      }}
                    >
                      CSV ↓
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      <section className="section">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Manual assets</h2>
          <Button onClick={() => setAssetAddOpen(true)}>+ Add manual asset</Button>
        </div>
        {assets.length === 0 ? (
          <div className="card muted tight">No manual assets yet.</div>
        ) : (
          <Card flush>
            {assets.map((a) => (
              <div
                key={a.id}
                role="button"
                tabIndex={0}
                onClick={() => setEditAsset(a)}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setEditAsset(a); } }}
                aria-label={`Edit ${a.name}`}
                className="row row-md"
                style={{ padding: "12px 20px", borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
              >
                <span className="ic" style={{ fontSize: 13, textTransform: "uppercase" }}>
                  {a.assetType.charAt(0)}
                </span>
                <div className="grow">
                  <div>{a.name}</div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {a.assetType} · updated {new Date(a.updatedAt).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                  </div>
                </div>
                <span className="num money">{money(a.valueCents, { decimals: 2 })}</span>
              </div>
            ))}
          </Card>
        )}
      </section>

      <section className="section">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Liabilities</h2>
          <Button onClick={() => setLiabAddOpen(true)}>+ Add liability</Button>
        </div>
        {liabilities.length === 0 ? (
          <div className="card muted tight">No liabilities yet.</div>
        ) : (
          <Card flush>
            {liabilities.map((l) => {
              const hasLimit = l.limitCents && l.limitCents > 0;
              const hasOriginal = l.originalBalanceCents && l.originalBalanceCents > 0;
              const paidDownCents = hasOriginal && l.originalBalanceCents != null
                ? Math.max(0, l.originalBalanceCents - l.balanceCents)
                : 0;
              const paidDownPct = hasOriginal && l.originalBalanceCents != null && l.originalBalanceCents > 0
                ? Math.round((paidDownCents / l.originalBalanceCents) * 100)
                : 0;
              return (
                <div
                  key={l.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => setEditLiab(l)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setEditLiab(l); } }}
                  aria-label={`Edit ${l.name}`}
                  className="stack stack-sm"
                  style={{ padding: "12px 20px", borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                >
                  <div className="row row-md">
                    <div className="grow">
                      <div>{l.name}</div>
                      <div className="muted" style={{ fontSize: 12 }}>
                        <Badge className="chip">{l.liabilityType}</Badge>
                        {l.aprPct != null && <>{l.aprPct}% APR</>}
                        {l.minPaymentCents != null && <> · min {money(l.minPaymentCents, { decimals: 0 })}/mo</>}
                        {l.payoffDate && <> · payoff {new Date(l.payoffDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}</>}
                        {l.startedAt && <> · started {new Date(`${l.startedAt}-01`).toLocaleDateString("en-US", { month: "short", year: "numeric" })}</>}
                        {hasOriginal && <> · {paidDownPct}% paid down</>}
                      </div>
                    </div>
                    <span className="num money neg">{money(l.balanceCents, { decimals: 2 })}</span>
                  </div>
                  {(hasLimit || hasOriginal) && (
                    <ProgressBar
                      value={hasOriginal ? paidDownCents : l.balanceCents}
                      max={hasOriginal ? l.originalBalanceCents ?? undefined : l.limitCents ?? undefined}
                      size="sm"
                      tone={hasOriginal ? "default" : "negative"}
                      aria-label={hasOriginal ? `${l.name} payoff progress` : `${l.name} utilization`}
                    />
                  )}
                </div>
              );
            })}
          </Card>
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

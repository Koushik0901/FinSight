import { useMemo, useState } from "react";
import { useBudgetEnvelopes, useSetBudget } from "../../api/hooks/budget";
import { useTransactions } from "../../api/hooks/transactions";
import type { BudgetEnvelope } from "../../api/openapiClient";
import { money } from "../../utils/format";
import { toast } from "sonner";
import * as I from "../../components/Icons";
import { MobileStat, MobileStatRow } from "../../components/mobile/MobileStat";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";

function envelopeStatus(env: BudgetEnvelope) {
  const transfer = (env as unknown as { transferCents?: number }).transferCents ?? 0;
  const available = env.budgetCents + env.carryoverCents + transfer;
  if (available <= 0 && env.budgetCents <= 0) return { label: "No budget set", tone: "warning" as const };
  const pct = available > 0 ? (env.spentCents / available) * 100 : 100;
  if (env.spentCents > available) {
    const remaining = available - env.spentCents;
    return { label: `Over by ${money(remaining, { decimals: 2 })}`, tone: "negative" as const };
  }
  if (pct > 90) return { label: "Almost used", tone: "warning" as const };
  if (pct > 60) return { label: "Watch", tone: "accent" as const };
  return { label: "Available", tone: "positive" as const };
}

type Filter = "all" | "over" | "watch" | "ok";

export default function MobileBudget() {
  const { data: envelopes = [], isLoading } = useBudgetEnvelopes();
  const [filter, setFilter] = useState<Filter>("all");
  const [detail, setDetail] = useState<BudgetEnvelope | null>(null);
  const [adjust, setAdjust] = useState(false);
  const [adjustValue, setAdjustValue] = useState("");
  const setBudget = useSetBudget();

  const primaryCurrency = "USD";

  const totals = useMemo(() => {
    const totalBudget = envelopes.reduce((s, e) => s + e.budgetCents + e.carryoverCents, 0);
    const totalSpent = envelopes.reduce((s, e) => s + e.spentCents, 0);
    const remaining = totalBudget - totalSpent;
    const pct = totalBudget > 0 ? Math.round((totalSpent / totalBudget) * 100) : 0;
    return { totalBudget, totalSpent, remaining, pct };
  }, [envelopes]);

  const conscious = useMemo(() => {
    // Budget envelopes don't carry spendingType directly; use categories mapping
    // For mobile, show a simple allocation bar from available vs spent
    return null;
  }, []);

  const filtered = useMemo(() => {
    if (filter === "all") return envelopes;
    return envelopes.filter((e) => {
      const s = envelopeStatus(e);
      if (filter === "over") return s.tone === "negative";
      if (filter === "watch") return s.tone === "warning" || s.tone === "accent";
      if (filter === "ok") return s.tone === "positive";
      return true;
    });
  }, [envelopes, filter]);

  // Transactions for detail sheet
  const { data: detailTxns = [] } = useTransactions(
    detail ? { accountId: null, limit: 20, offset: 0, search: null, filterPreset: null, startDate: null, endDate: null } : { accountId: null, limit: 1, offset: 0, search: null, filterPreset: null, startDate: null, endDate: null }
  );
  const detailFilteredTxns = useMemo(() => {
    if (!detail) return [];
    return (detailTxns as unknown as Array<{ category_id?: string | null }>).filter((t) => t.category_id === detail.categoryId).slice(0, 8) as unknown as typeof detailTxns;
  }, [detail, detailTxns]);

  if (isLoading) return <div className="stub"><span className="spinner" aria-hidden="true" /> Loading…</div>;

  if (envelopes.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <MobileEmptyState
          icon={<I.Lego width={28} height={28} />}
          title="No budgets yet"
          description="Set monthly budgets for each category to see how much remains and where you are over."
          primaryAction={<button className="btn primary" onClick={() => toast("Create categories first")}>Set up categories</button>}
        />
      </div>
    );
  }

  const handleAdjust = async () => {
    if (!detail) return;
    const amountCents = Math.round(Number(adjustValue || 0) * 100);
    try {
      await setBudget.mutateAsync({ categoryId: detail.categoryId, amountCents });
      toast.success("Budget saved", { description: `${detail.categoryLabel} · ${money(amountCents)}` });
      setAdjust(false);
      setAdjustValue("");
    } catch {
      toast.error("Failed to save budget");
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      {/* Hero: remaining + % */}
      <div className="mobile-stat hero" style={{ padding: 16 }}>
        <span className="mobile-stat-label">Budget health</span>
        <span className={`mobile-stat-value lg money`} style={{ color: totals.remaining < 0 ? "var(--negative)" : "var(--ink)" }}>
          {money(totals.remaining, { currency: primaryCurrency })}
        </span>
        <span className="mobile-stat-sub">{totals.remaining < 0 ? "over budget" : "left to spend"} · {totals.pct}% used</span>
        <div style={{ height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 8 }}>
          <div style={{ width: `${Math.min(100, totals.pct)}%`, height: "100%", background: totals.pct > 100 ? "var(--negative)" : totals.pct > 90 ? "var(--warning)" : "var(--accent)", borderRadius: 999 }} />
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--ink-mute)", marginTop: 6 }}>
          <span className="money">{money(totals.totalSpent)} spent</span>
          <span className="money">of {money(totals.totalBudget)}</span>
        </div>
      </div>

      <MobileStatRow>
        <MobileStat label="Envelopes" value={String(envelopes.length)} sub={`${filtered.length} shown`} />
        <MobileStat label="Over budget" value={String(envelopes.filter((e) => envelopeStatus(e).tone === "negative").length)} sub="Need attention" />
      </MobileStatRow>

      {/* Filter */}
      <SegmentedControl
        options={[
          { value: "all", label: "All" },
          { value: "over", label: "Over" },
          { value: "watch", label: "Watch" },
          { value: "ok", label: "OK" },
        ]}
        value={filter}
        onChange={(v) => setFilter(v as Filter)}
        ariaLabel="Budget filter"
      />

      {/* List — touch-friendly cards */}
      <MobileSection title="Categories" description="Tap for detail, transactions, and adjustments">
        <MobileList ariaLabel="Budget categories">
          {filtered.map((env) => {
            const s = envelopeStatus(env);
            const available = env.budgetCents + env.carryoverCents;
            const remaining = available - env.spentCents;
            const pct = available > 0 ? Math.min(100, (env.spentCents / available) * 100) : 0;
            const tone = s.tone === "negative" ? "var(--negative)" : s.tone === "warning" ? "var(--warning)" : s.tone === "positive" ? "var(--positive)" : "var(--accent)";
            return (
              <MobileListItem
                key={env.categoryId}
                icon={<span style={{ width: 12, height: 12, borderRadius: "50%", background: env.categoryColor ?? "var(--accent)", display: "inline-block" }} />}
                title={env.categoryLabel}
                subtitle={`${env.txnCount} transactions · ${s.label}`}
                value={money(remaining, { currency: primaryCurrency })}
                meta={`${Math.round(pct)}% used`}
                onPress={() => {
                  setDetail(env);
                  setAdjust(false);
                  setAdjustValue(env.budgetCents > 0 ? String(Math.round(env.budgetCents / 100)) : "");
                }}
              />
            );
          })}
        </MobileList>
      </MobileSection>

      {/* Detail sheet */}
      <BottomSheet open={!!detail} onClose={() => setDetail(null)} title={detail?.categoryLabel ?? "Category"}>
        {detail ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {(() => {
              const s = envelopeStatus(detail);
              const available = detail.budgetCents + detail.carryoverCents;
              const remaining = available - detail.spentCents;
              const pct = available > 0 ? Math.min(100, (detail.spentCents / available) * 100) : 0;
              return (
                <>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
                    <div>
                      <div style={{ display: "inline-flex", alignItems: "center", gap: 6, marginBottom: 6 }}>
                        <span style={{ width: 10, height: 10, borderRadius: "50%", background: detail.categoryColor ?? "var(--accent)", display: "inline-block" }} />
                        <span style={{ fontSize: 12, color: "var(--ink-mute)" }}>{detail.txnCount} transactions</span>
                      </div>
                      <div className="money" style={{ fontSize: 32, fontWeight: 750, letterSpacing: "-0.03em", color: remaining < 0 ? "var(--negative)" : "var(--ink)", lineHeight: 1 }}>
                        {money(remaining, { currency: primaryCurrency })}
                      </div>
                      <div style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 4 }}>{remaining < 0 ? "over budget" : "left to spend"}</div>
                    </div>
                    <span className={`chip ${s.tone === "negative" ? "negative" : s.tone === "warning" ? "warning" : s.tone === "positive" ? "positive" : "accent"}`}>{s.label}</span>
                  </div>

                  <div style={{ height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                    <div style={{ width: `${pct}%`, height: "100%", background: s.tone === "negative" ? "var(--negative)" : s.tone === "warning" ? "var(--warning)" : detail.categoryColor ?? "var(--accent)", borderRadius: 999 }} />
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--ink-mute)" }}>
                    <span className="money">{money(detail.spentCents)} spent</span>
                    <span className="money">of {money(available)}</span>
                  </div>

                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
                    <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Budgeted</div>
                      <div className="money" style={{ fontWeight: 650, marginTop: 4 }}>{money(detail.budgetCents)}</div>
                    </div>
                    <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Spent</div>
                      <div className="money" style={{ fontWeight: 650, marginTop: 4 }}>{money(detail.spentCents)}</div>
                    </div>
                  </div>

                  {!adjust ? (
                    <button type="button" className="btn" style={{ width: "100%", minHeight: 44 }} onClick={() => setAdjust(true)}>
                      Adjust budget
                    </button>
                  ) : (
                    <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface-2)", display: "flex", flexDirection: "column", gap: 10 }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Monthly budget for {detail.categoryLabel}</div>
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <input
                          className="control"
                          type="number"
                          min="0"
                          step="10"
                          value={adjustValue}
                          onChange={(e) => setAdjustValue(e.target.value)}
                          placeholder="0"
                          style={{ flex: 1, minHeight: 44 }}
                          autoFocus
                        />
                        <button type="button" className="btn primary" style={{ minHeight: 44 }} onClick={() => void handleAdjust()} disabled={setBudget.isPending}>
                          Save
                        </button>
                        <button type="button" className="btn" style={{ minHeight: 44 }} onClick={() => setAdjust(false)}>
                          Cancel
                        </button>
                      </div>
                    </div>
                  )}

                  <MobileSection title="Recent transactions" description="Filtered to this category">
                    {detailFilteredTxns.length === 0 ? (
                      <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface)", color: "var(--ink-mute)", fontSize: 13 }}>
                        No transactions in this category for the current filter.
                      </div>
                    ) : (
                      <MobileList ariaLabel="Category transactions">
                        {(detailFilteredTxns as unknown as Array<{ id: string; merchant_raw: string; posted_at: string; amount_cents: number }>).map((t) => (
                          <MobileListItem
                            key={t.id}
                            title={t.merchant_raw}
                            subtitle={new Date(t.posted_at).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                            value={money(t.amount_cents)}
                            chevron={false}
                          />
                        ))}
                      </MobileList>
                    )}
                  </MobileSection>
                </>
              );
            })()}
          </div>
        ) : null}
      </BottomSheet>
    </div>
  );
}

import { useState, useMemo } from "react";
import { useNavigate, Link } from "react-router-dom";
import { useAccounts } from "../../api/hooks/accounts";
import { useAgentStatus, useNeedsReviewCount } from "../../api/hooks/agent";
import { useHealthScore } from "../../api/hooks/insights";
import { useCategoriesWithSpending } from "../../api/hooks/transactions";
import { useMonthTotals, useSavingsRateHistory } from "../../api/hooks";
import { useFinancialMetrics } from "../../api/hooks/metrics";
import { useNetWorth, useNetWorthHistory } from "../../api/hooks/networth";
import { useBudgetEnvelopes } from "../../api/hooks/budget";
import { useTransactions } from "../../api/hooks/transactions";
import { money } from "../../utils/format";
import { accountTypeColor } from "../../utils/accountColor";
import * as I from "../../components/Icons";
import { CopilotNudge } from "../../components/CopilotNudge";
import { MobileStat, MobileStatRow } from "../../components/mobile/MobileStat";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import NetWorthChart from "../../components/NetWorthChart";
import { UnconvertedCurrencies } from "../../components/UnconvertedCurrencies";
import ExplainInspector from "../../components/ExplainInspector";

function minutesAgoLabel(iso: string | null | undefined): string {
  if (!iso) return "Not yet scanned";
  const mins = Math.round((Date.now() - new Date(iso).getTime()) / 60_000);
  if (mins < 1) return "Just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.round(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.round(hrs / 24)}d ago`;
}

export default function MobileToday() {
  const navigate = useNavigate();
  const { data: accounts = [], isLoading: accLoading } = useAccounts();
  const { data: totals } = useMonthTotals();
  const { data: metrics } = useFinancialMetrics();
  const { data: healthScore } = useHealthScore();
  const { data: savingsRateHistory = [] } = useSavingsRateHistory();
  const { data: cats = [] } = useCategoriesWithSpending();
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const netWorth = useNetWorth();
  const { data: envelopes = [] } = useBudgetEnvelopes();
  const { data: recentTxns = [] } = useTransactions({ accountId: null, limit: 5, offset: 0, search: null, filterPreset: null, startDate: null, endDate: null });

  const [range] = useState<"6M">("6M");
  const [explainKey, setExplainKey] = useState<string | null>(null);

  const days = 180;
  const { data: nwHistory = [] } = useNetWorthHistory(days);

  const isLoading = accLoading;

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        <span className="spinner" aria-hidden="true" />
        <span style={{ marginTop: 12 }}>Loading…</span>
      </div>
    );
  }

  if (accounts.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <MobileEmptyState
          icon={<I.Wallet width={28} height={28} />}
          title="No accounts yet"
          description="Add your first account to unlock Today — import a statement, connect a bank, or add it by hand."
          primaryAction={
            <button className="btn primary" type="button" onClick={() => navigate("/onboarding")}>
              Start setup
            </button>
          }
          secondaryAction={
            <button className="btn" type="button" onClick={() => navigate("/accounts")}>
              Add manually
            </button>
          }
        />
      </div>
    );
  }

  const primaryCurrency = metrics?.currency ?? accounts[0]?.currency ?? "USD";
  const activeCats = cats.filter((c) => c.thisMonthCents > 0);
  const totalSpendRaw = activeCats.reduce((s, c) => s + c.thisMonthCents, 0);
  const liquidCents = metrics?.liquidCents ?? 0;
  const runwayDays = metrics?.runwayDays ?? null;

  // Remaining budget: sum of envelope remaining
  const budgetRemaining = useMemo(() => {
    return envelopes.reduce((sum, e) => {
      const available = (e.budgetCents ?? 0) + (e.carryoverCents ?? 0) + ((e as unknown as { transferCents?: number }).transferCents ?? 0);
      return sum + (available - e.spentCents);
    }, 0);
  }, [envelopes]);

  const budgetPercentUsed = useMemo(() => {
    const totalBudget = envelopes.reduce((s, e) => s + (e.budgetCents ?? 0) + (e.carryoverCents ?? 0), 0);
    const totalSpent = envelopes.reduce((s, e) => s + e.spentCents, 0);
    if (totalBudget <= 0) return null;
    return Math.round((totalSpent / totalBudget) * 100);
  }, [envelopes]);

  const savingsRate = savingsRateHistory.length > 0 ? savingsRateHistory[savingsRateHistory.length - 1]?.savingsRatePct ?? null : null;
  const health = healthScore?.total ?? null;
  const now = new Date();
  const weekday = now.toLocaleDateString("en-US", { weekday: "long" });
  const dateLong = now.toLocaleDateString("en-US", { month: "long", day: "numeric", year: "numeric" });

  const anomalyCount = agentStatus?.anomalyCount ?? 0;
  const dayOfMonth = now.getDate();
  const prevMonthDate = new Date(now.getFullYear(), now.getMonth() - 1, 1);
  const prevMonthLabel = prevMonthDate.toLocaleDateString("en-US", { month: "long", year: "numeric" });
  const shouldShowMonthlyReview = dayOfMonth <= 7;

  const primaryAction = needsReview > 0
    ? { heading: `${needsReview} transaction${needsReview === 1 ? "" : "s"} need review`, label: "Review now", route: "/inbox" }
    : anomalyCount > 0
      ? { heading: `${anomalyCount} unusual charge${anomalyCount === 1 ? "" : "s"} flagged`, label: "Check charges", route: "/inbox" }
      : shouldShowMonthlyReview
        ? { heading: `Close out ${prevMonthLabel}`, label: "Start close", route: "/close" }
        : { heading: "You're all caught up", label: "View categories", route: "/categories" };

  // Conscious spending allocation (Need/Want/Saving)
  const consciousTotals = useMemo(() => {
    const byType: Record<string, number> = { Need: 0, Want: 0, Saving: 0, Investment: 0 };
    for (const c of cats) {
      const t = (c as unknown as { spendingType?: string }).spendingType ?? "Want";
      byType[t] = (byType[t] ?? 0) + (c.thisMonthCents ?? 0);
    }
    const total = Object.values(byType).reduce((s, v) => s + v, 0) || 1;
    return { byType, total };
  }, [cats]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      {/* Hero: financial position */}
      <section className="mobile-stat hero" style={{ padding: 16, gap: 8 }}>
        <span className="mobile-stat-label" style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
          <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--accent)", display: "inline-block" }} />
          {weekday} · {dateLong}
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <span style={{ fontFamily: "var(--mono)", fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-mute)" }}>
            Net worth
          </span>
          <button
            type="button"
            aria-label="Explain net worth"
            onClick={() => setExplainKey("net_worth")}
            style={{ width: 28, height: 28, display: "grid", placeItems: "center", borderRadius: 8, border: "1px solid var(--line)", background: "var(--surface)", color: "var(--ink-mute)" }}
          >
            ⓘ
          </button>
        </div>
        <span className="mobile-stat-value lg money" style={{ color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}>
          {money(netWorth, { currency: primaryCurrency })}
        </span>
        <span className="mobile-stat-sub">
          {totalSpendRaw > 0 ? `${money(totalSpendRaw)} spent this month` : "Fresh month, fresh baseline."} · Agent {minutesAgoLabel(agentStatus?.lastScanAt)}
        </span>
        {nwHistory.length > 1 ? (
          <div style={{ margin: "4px -8px 0" }}>
            <NetWorthChart points={nwHistory.slice(-30)} rangeLabel="6M" embed />
          </div>
        ) : null}
        {metrics?.unconvertedHoldings ? <UnconvertedCurrencies holdings={metrics.unconvertedHoldings} primary={metrics.currency} /> : null}
      </section>

      {/* Three answers: spent / remaining / saving */}
      <MobileStatRow>
        <MobileStat label="Spent this month" value={money(totalSpendRaw ?? 0, { currency: primaryCurrency })} sub={`${activeCats.length} categories`} />
        <MobileStat
          label="Remaining budget"
          value={envelopes.length > 0 ? money(budgetRemaining ?? 0, { currency: primaryCurrency }) : "—"}
          sub={budgetPercentUsed !== null ? `${budgetPercentUsed}% used` : "Set budgets to track"}
        />
      </MobileStatRow>

      <MobileStatRow>
        <MobileStat
          label="Savings rate"
          value={savingsRate !== null ? `${Math.round(savingsRate)}%` : "—"}
          sub={health !== null ? `Health ${health}/100` : "No score yet"}
        />
        <MobileStat
          label="Runway"
          value={runwayDays !== null ? `${runwayDays}d` : "—"}
          sub={runwayDays !== null ? `Liquid ${money(liquidCents, { currency: primaryCurrency })}` : "Needs history"}
        />
      </MobileStatRow>

      {/* Conscious spending allocation - horizontal bar */}
      {activeCats.length > 0 ? (
        <MobileSection title="Conscious spending" description="How this month's spending splits">
          <div style={{ display: "flex", height: 12, borderRadius: 999, overflow: "hidden", background: "var(--surface-2)", gap: 2 }}>
            <span style={{ flex: (consciousTotals.byType.Need ?? 0) / consciousTotals.total, background: "#60A5FA", borderRadius: 999 }} title={`Need ${money(consciousTotals.byType.Need ?? 0)}`} />
            <span style={{ flex: (consciousTotals.byType.Want ?? 0) / consciousTotals.total, background: "#FB923C", borderRadius: 999 }} title={`Want ${money(consciousTotals.byType.Want ?? 0)}`} />
            <span style={{ flex: (consciousTotals.byType.Saving ?? 0) / consciousTotals.total, background: "#34D399", borderRadius: 999 }} title={`Saving ${money(consciousTotals.byType.Saving ?? 0)}`} />
            <span style={{ flex: (consciousTotals.byType.Investment ?? 0) / consciousTotals.total, background: "#A78BFA", borderRadius: 999 }} title={`Investment ${money(consciousTotals.byType.Investment ?? 0)}`} />
          </div>
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap", fontSize: 12, color: "var(--ink-mute)" }}>
            <span><span style={{ width: 8, height: 8, borderRadius: "50%", background: "#60A5FA", display: "inline-block", marginRight: 6 }} />Need {Math.round(((consciousTotals.byType.Need ?? 0) / consciousTotals.total) * 100)}%</span>
            <span><span style={{ width: 8, height: 8, borderRadius: "50%", background: "#FB923C", display: "inline-block", marginRight: 6 }} />Want {Math.round(((consciousTotals.byType.Want ?? 0) / consciousTotals.total) * 100)}%</span>
            <span><span style={{ width: 8, height: 8, borderRadius: "50%", background: "#34D399", display: "inline-block", marginRight: 6 }} />Saving {Math.round(((consciousTotals.byType.Saving ?? 0) / consciousTotals.total) * 100)}%</span>
          </div>
        </MobileSection>
      ) : null}

      {/* Single most useful nudge */}
      <div className="card" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ fontFamily: "var(--mono)", fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--accent)", display: "inline-block" }} />
          Next action
        </div>
        <h2 style={{ margin: 0, fontSize: 15, fontWeight: 650, lineHeight: 1.35, color: "var(--ink)" }}>{primaryAction.heading}</h2>
        <p style={{ margin: 0, color: "var(--ink-mute)", fontSize: 13, lineHeight: 1.5 }}>
          {totals ? `Net ${money(Math.max(totals.netCents, 0))} left from ${now.toLocaleString("default", { month: "long" }).toLowerCase()} cash flow.` : "Your latest snapshot is ready."}
        </p>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 4 }}>
          <button className="btn primary sm" type="button" onClick={() => navigate(primaryAction.route)}>
            {primaryAction.label}
          </button>
          <CopilotNudge prompt="Give me the short version of what changed financially this week and what I should do next." label="Ask Copilot" />
        </div>
      </div>

      {/* Recent transactions — summary → detail */}
      <MobileSection
        title="Recent transactions"
        actionLabel="View all"
        onAction={() => navigate("/transactions")}
      >
        {recentTxns.length === 0 ? (
          <div style={{ padding: 16, border: "1px solid var(--line)", borderRadius: 16, background: "var(--surface)", color: "var(--ink-mute)", fontSize: 13 }}>
            No transactions yet. Import a statement to see them here.
          </div>
        ) : (
          <MobileList ariaLabel="Recent transactions">
            {recentTxns.slice(0, 5).map((t) => {
              const cat = (t as unknown as { category_label?: string }).category_label ?? (t.category_label as string | null) ?? "Uncategorized";
              const spendingType = (t as unknown as { spending_type?: string }).spending_type ?? null;
              const date = new Date(t.posted_at).toLocaleDateString("en-US", { month: "short", day: "numeric" });
              const amt = money(t.amount_cents, { currency: primaryCurrency });
              const isExpense = t.amount_cents < 0;
              return (
                <MobileListItem
                  key={t.id}
                  icon={
                    <span style={{ width: 10, height: 10, borderRadius: "50%", background: t.category_color ?? "var(--accent)", display: "inline-block" }} />
                  }
                  title={t.merchant_raw}
                  subtitle={`${cat}${spendingType ? ` · ${spendingType}` : ""} · ${date}`}
                  value={amt}
                  valueTone={isExpense ? "default" : "positive"}
                  onPress={() => navigate(`/transactions?focus=${t.id}`)}
                />
              );
            })}
          </MobileList>
        )}
      </MobileSection>

      {/* Progressive disclosure */}
      <details style={{ borderTop: "1px solid var(--hairline)", paddingTop: 12 }}>
        <summary style={{ fontSize: 13, fontWeight: 600, color: "var(--ink-2)", cursor: "pointer", listStyle: "none", display: "flex", alignItems: "center", justifyContent: "space-between", minHeight: 44 }}>
          <span>More household detail</span>
          <I.Down width={14} height={14} aria-hidden="true" />
        </summary>
        <div style={{ display: "flex", flexDirection: "column", gap: 12, paddingTop: 8 }}>
          <Link to="/budget" style={{ padding: 14, border: "1px solid var(--line)", borderRadius: 14, background: "var(--surface)", textDecoration: "none", color: "var(--ink)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span>
              <strong style={{ display: "block", fontSize: 14 }}>Budget overview</strong>
              <span style={{ fontSize: 12, color: "var(--ink-mute)" }}>{envelopes.length} envelopes · {budgetPercentUsed ?? 0}% used</span>
            </span>
            <I.ArrowRight width={16} height={16} />
          </Link>
          <Link to="/accounts" style={{ padding: 14, border: "1px solid var(--line)", borderRadius: 14, background: "var(--surface)", textDecoration: "none", color: "var(--ink)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span>
              <strong style={{ display: "block", fontSize: 14 }}>Accounts</strong>
              <span style={{ fontSize: 12, color: "var(--ink-mute)" }}>{accounts.length} accounts</span>
            </span>
            <I.ArrowRight width={16} height={16} />
          </Link>
        </div>
      </details>

      <BottomSheet open={explainKey !== null} onClose={() => setExplainKey(null)} title="About this number">
        {explainKey ? <ExplainInspector metricKey={explainKey} currency={primaryCurrency} onClose={() => setExplainKey(null)} /> : null}
      </BottomSheet>
    </div>
  );
}

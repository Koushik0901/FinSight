import { useState } from "react";
import { useNavigate, Link } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useAgentStatus, useNeedsReviewCount } from "../api/hooks/agent";
import { useHealthScore } from "../api/hooks/insights";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { useGoals, useUpdateGoalBalance } from "../api/hooks/budget";
import { useRecurring } from "../api/hooks/recurring";
import { useCreateMonthlyReview, useMonthTotals, useSavingsRateHistory } from "../api/hooks";
import AgentActivityFeed from "../components/AgentActivityFeed";
import { useUncelebratedMilestones } from "../api/hooks/assets";
import { useNetWorth, useNetWorthHistory } from "../api/hooks/networth";
import NetWorthChart from "../components/NetWorthChart";
import { CopilotNudge } from "../components/CopilotNudge";
import { CopilotQuickAsk } from "../components/CopilotQuickAsk";
import { money } from "../utils/format";
import * as I from "../components/Icons";

const RANGES = [
  { key: "1M", days: 30, label: "month" },
  { key: "3M", days: 90, label: "3 months" },
  { key: "6M", days: 180, label: "6 months" },
  { key: "1Y", days: 365, label: "year" },
  { key: "All", days: 36500, label: "all time" },
] as const;

function minutesAgoLabel(iso: string | null | undefined) {
  if (!iso) return "standing by";
  const mins = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60_000));
  if (mins < 1) return "ran just now";
  if (mins < 60) return `ran ${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `ran ${hours}h ago`;
  return `ran ${Math.round(hours / 24)}d ago`;
}

function daysUntilLabel(dateStr: string): string | null {
  const diff = Math.round((new Date(dateStr).getTime() - Date.now()) / 86400000);
  if (diff < 0 || diff > 14) return null;
  if (diff === 0) return "today";
  if (diff === 1) return "tomorrow";
  return `in ${diff} days`;
}

function milestoneLabel(thresholdCents: number, currency: string) {
  return money(thresholdCents, { currency, decimals: 0 });
}

function SavingsRateSparkline({ points }: { points: Array<{ month: string; savingsRatePct: number }> }) {
  if (points.length < 2) return null;
  const width = 140;
  const height = 40;
  const max = Math.max(...points.map((point) => point.savingsRatePct), 10);
  const min = Math.min(...points.map((point) => point.savingsRatePct), 0);
  const range = Math.max(max - min, 1);
  const path = points.map((point, index) => {
    const x = (index / Math.max(points.length - 1, 1)) * width;
    const y = height - ((point.savingsRatePct - min) / range) * height;
    return `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-label="Savings rate history">
      <path d={path} fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

function SmartSweepCard({ netCents, onDismiss }: { netCents: number; onDismiss: () => void }) {
  const navigate = useNavigate();
  const { data: goals = [] } = useGoals();
  const updateBalance = useUpdateGoalBalance();
  const firstGoal = goals[0] ?? null;

  const handlePark = async () => {
    if (!firstGoal) return;
    try {
      await updateBalance.mutateAsync({ id: firstGoal.id, currentCents: firstGoal.currentCents + netCents });
      toast.success(`Parked ${money(netCents)} in ${firstGoal.name}`);
      onDismiss();
    } catch {
      toast.error("Could not park funds");
    }
  };

  return (
    <div className="card accent" style={{ height: "100%" }}>
      <div className="eyebrow" style={{ color: "var(--accent)", marginBottom: 8 }}><span className="dot" />Smart sweep</div>
      <div className="h3" style={{ marginBottom: 10 }}>You have {money(netCents)} unallocated this month.</div>
      <p className="muted" style={{ marginTop: 0, lineHeight: 1.6 }}>
        Put surplus cash to work before it disappears into drift. FinSight can park it in your next goal or let you choose where it goes.
      </p>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 14 }}>
        {firstGoal && <button className="btn primary sm" type="button" disabled={updateBalance.isPending} onClick={() => void handlePark()}>{updateBalance.isPending ? "Parking…" : `Park in ${firstGoal.name}`}</button>}
        <button className="btn sm" type="button" onClick={() => navigate("/goals")}>Assign to a goal…</button>
        <button className="btn ghost sm" type="button" onClick={onDismiss}>Dismiss</button>
      </div>
    </div>
  );
}

function HealthScoreCard({ score, savingsPoints }: { score: ReturnType<typeof useHealthScore>["data"]; savingsPoints: Array<{ month: string; savingsRatePct: number }> }) {
  if (!score || !("breakdown" in score) || !score.breakdown) return null;
  return (
    <div className="card" style={{ height: "100%" }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 20, flexWrap: "wrap" }}>
        <div className="row row-md" style={{ alignItems: "center" }}>
          <div style={{ background: `conic-gradient(var(--accent) ${score.total * 3.6}deg, var(--surface-2) 0deg)`, borderRadius: "50%", width: 80, height: 80, display: "flex", alignItems: "center", justifyContent: "center" }}>
            <div style={{ background: "var(--elevated)", borderRadius: "50%", width: 64, height: 64, display: "flex", alignItems: "center", justifyContent: "center", flexDirection: "column" }}>
              <span style={{ fontSize: 22, fontWeight: 700 }}>{score.grade}</span>
              <span className="muted" style={{ fontSize: 11 }}>{score.total}/100</span>
            </div>
          </div>
          <div className="stack stack-xs">
            <div className="eyebrow">Financial health</div>
            <div className="h3">Your scorecard this month</div>
            <div className="muted" style={{ fontSize: 12.5 }}>Savings {score.breakdown.savingsRatePct}% · Emergency fund {score.breakdown.emergencyFundMonths.toFixed(1)} months</div>
          </div>
        </div>
        <div className="stack stack-xs" style={{ minWidth: 180 }}>
          <div className="eyebrow">Savings trend</div>
          <SavingsRateSparkline points={savingsPoints} />
          <div className="muted" style={{ fontSize: 12 }}>{savingsPoints[savingsPoints.length - 1]?.month ?? ""} · latest {savingsPoints[savingsPoints.length - 1]?.savingsRatePct ?? 0}%</div>
        </div>
      </div>
      <ul style={{ margin: "14px 0 0", paddingLeft: 18 }}>
        {score.tips.map((tip) => <li key={tip} style={{ marginBottom: 6, color: "var(--ink-mute)", fontSize: 13 }}>{tip}</li>)}
      </ul>
    </div>
  );
}

export default function Today() {
  const navigate = useNavigate();
  const { data: accounts = [], isLoading: accLoading } = useAccounts();
  const { data: totals, isLoading: totLoading } = useMonthTotals();
  const { data: healthScore } = useHealthScore();
  const { data: savingsRateHistory = [] } = useSavingsRateHistory();
  const { data: cats = [] } = useCategoriesWithSpending();
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const { data: milestones = [] } = useUncelebratedMilestones();
  const createMonthlyReview = useCreateMonthlyReview();
  const netWorth = useNetWorth();
  const [range, setRange] = useState<typeof RANGES[number]["key"]>("6M");
  const [sweepDismissed, setSweepDismissed] = useState(false);
  const [dismissedMilestones, setDismissedMilestones] = useState<number[]>([]);
  const days = RANGES.find((r) => r.key === range)!.days;
  const { data: nwHistory = [] } = useNetWorthHistory(days);
  const { data: recurring = [] } = useRecurring();
  const now = new Date();
  const weekday = now.toLocaleDateString("en-US", { weekday: "long" });
  const dateLong = now.toLocaleDateString("en-US", { month: "long", day: "numeric", year: "numeric" });
  const monthLabel = now.toLocaleString("default", { month: "long" });
  const primaryCurrency = accounts[0]?.currency ?? "USD";
  const isLoading = accLoading || totLoading;

  if (isLoading) return <div className="stub" aria-live="polite" aria-busy="true"><span className="spinner" aria-hidden="true" /><span style={{ marginTop: 12 }}>Loading…</span></div>;

  if (accounts.length === 0) {
    return (
      <div className="empty-state">
        <section className="empty-panel" aria-labelledby="today-empty-title">
          <div className="eyebrow">First step</div>
          <h2 id="today-empty-title">No accounts yet. Add your first account to unlock Today.</h2>
          <p>Import a CSV statement, connect SimpleFIN, or add accounts by hand to start using your own financial data.</p>
          <div className="empty-actions">
            <button className="btn primary" type="button" onClick={() => navigate("/onboarding")}>Start setup</button>
            <button className="btn" type="button" onClick={() => navigate("/accounts")}>Add manually</button>
            <button className="btn ghost" type="button" onClick={() => navigate("/settings")}>Connect SimpleFIN</button>
          </div>
        </section>
      </div>
    );
  }

  const activeCats = cats.filter((c) => c.thisMonthCents > 0);
  const totalSpendRaw = activeCats.reduce((s, c) => s + c.thisMonthCents, 0);
  const totalSpend = totalSpendRaw || 1;
  const dayOfMonth = now.getDate();
  const avgDailyBurn = totals ? totals.expenseCents / Math.max(dayOfMonth, 1) : 0;
  const recurringSoon = recurring.filter((item) => daysUntilLabel(item.nextExpected) !== null).slice(0, 6);
  // When nothing is due in the next two weeks (common with historical imports),
  // still surface the user's recurring commitments so the panel stays useful.
  const upcomingRecurring = recurringSoon.length > 0 ? recurringSoon : recurring.slice(0, 5);
  // Accounts with no confirmed balance (e.g. CSV-imported history with no
  // balance field) are excluded rather than silently counted as a real $0.
  const knownAccounts = accounts.filter((account) => account.balance_known);
  const unknownBalanceCount = accounts.length - knownAccounts.length;
  const liquidCents = knownAccounts.filter((account) => account.balance_cents > 0 && !/investment|brokerage|retirement/i.test(account.type)).reduce((sum, account) => sum + account.balance_cents, 0);
  const investedCents = knownAccounts.filter((account) => account.balance_cents > 0 && /investment|brokerage|retirement/i.test(account.type)).reduce((sum, account) => sum + account.balance_cents, 0);
  const creditCents = Math.abs(knownAccounts.filter((account) => account.balance_cents < 0).reduce((sum, account) => sum + account.balance_cents, 0));
  const runwayDays = avgDailyBurn > 0 ? Math.round(liquidCents / avgDailyBurn) : null;
  const showSweep = !sweepDismissed && !!totals && totals.netCents > 5000;
  const celebrateMilestones = milestones.filter((threshold): threshold is number => typeof threshold === "number").filter((threshold) => !dismissedMilestones.includes(threshold));
  const shouldShowMonthlyReview = dayOfMonth >= 28;
  const trendDelta = nwHistory.length >= 2 ? nwHistory[nwHistory.length - 1]!.totalCents - nwHistory[0]!.totalCents : null;
  const trendChipClass = trendDelta === null ? "" : trendDelta >= 0 ? " pos" : " neg";
  const trendText = trendDelta === null ? "Baseline building" : `${trendDelta >= 0 ? "↑" : "↓"} ${money(Math.abs(trendDelta), { currency: primaryCurrency })} over ${range}`;
  const biggestCategory = activeCats[0];
  const briefingText = totals ? `You have ${money(Math.max(totals.netCents, 0))} left from ${monthLabel.toLowerCase()} cash flow. ${needsReview > 0 ? `${needsReview} transactions still need review.` : `${biggestCategory?.label ?? "Spending"} is carrying most of the load this month.`}` : "Your latest local snapshot is ready. Open insights for the full story.";
  const lastMonthSpendTotal = cats.reduce((s, c) => s + c.lastMonthCents, 0);
  const daysInLastMonth = new Date(now.getFullYear(), now.getMonth(), 0).getDate();
  const lastMonthPaceCents = lastMonthSpendTotal * (Math.min(dayOfMonth, daysInLastMonth) / daysInLastMonth);
  let spendNarrative: string | null = null;
  if (lastMonthPaceCents > 0) {
    const pct = Math.round(((lastMonthPaceCents - totalSpendRaw) / lastMonthPaceCents) * 100);
    if (pct > 0) spendNarrative = `You're tracking ${pct}% below last month's pace.`;
    else if (pct < 0) spendNarrative = `You're tracking ${Math.abs(pct)}% above last month's pace.`;
    else spendNarrative = "You're tracking even with last month's pace.";
  }

  return (
    <div className="screen">
      <div className="day-hdr">
        <div><div className="eyebrow"><span className="dot" />{weekday} · {dateLong}</div></div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}><span className="chip"><I.Lock width="11" height="11" /> Local-only</span><span className="chip accent"><span className="dot" />Agent · {minutesAgoLabel(agentStatus?.lastScanAt)}</span></div>
      </div>

      <section className="hero-num">
        <div className="eyebrow" style={{ color: "var(--ink-mute)" }}>Net worth</div>
        <div className="h-display" style={{ color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}><span className="figure money">{money(netWorth, { currency: primaryCurrency })}</span></div>
        <div className="hero-meta">
          <span className={`npill${trendChipClass}`}>{trendText}</span>
          <span>·</span>
          <span>{totalSpendRaw > 0 ? `${money(totalSpendRaw)} spent so far this month` : "Fresh month, fresh baseline."}</span>
          {spendNarrative && <><span>·</span><span>{spendNarrative}</span></>}
        </div>
        {unknownBalanceCount > 0 && (
          <div className="muted" style={{ fontSize: 12.5, marginTop: 8 }} role="status">
            {unknownBalanceCount} account{unknownBalanceCount === 1 ? "" : "s"} {unknownBalanceCount === 1 ? "has" : "have"} no balance set — excluded from the totals above. <Link to="/accounts">Set balances →</Link>
          </div>
        )}
      </section>

      <section>
        <NetWorthChart points={nwHistory} rangeLabel={RANGES.find((r) => r.key === range)!.label} controls={<div className="toolbar">{RANGES.map((r) => <button key={r.key} className={range === r.key ? "on" : ""} onClick={() => setRange(r.key)} aria-pressed={range === r.key} type="button">{r.key}</button>)}</div>} />
      </section>

      <section className="stat-row">
        <div className="stat"><div className="label">Liquid</div><div className="value money">{money(liquidCents, { currency: primaryCurrency }).replace(/,/g, "") }</div><div className="sub">Cash and near-cash accounts</div></div>
        <div className="stat"><div className="label">Invested</div><div className="value money">{money(investedCents, { currency: primaryCurrency })}</div><div className="sub">Brokerage and retirement balances</div></div>
        <div className="stat"><div className="label">Credit</div><div className="value money">{money(creditCents, { currency: primaryCurrency })}</div><div className="sub">Outstanding liabilities on connected accounts</div></div>
        <div className="stat accent"><div className="label">Runway</div><div className="value">{runwayDays !== null ? `${runwayDays}d` : "—"}</div><div className="sub">At current burn · {totals ? money(totals.expenseCents) : "—"} monthly spend</div></div>
      </section>

      <section className="section" style={{ display: "grid", gridTemplateColumns: "minmax(0, 1.35fr) minmax(320px, 0.95fr)", gap: 16 }}>
        <div className="card">
          <div className="eyebrow" style={{ marginBottom: 10 }}><span className="dot" />Morning briefing · 60 seconds</div>
          <div className="h3" style={{ marginBottom: 10 }}>Start with what moved, what needs attention, and what to do next.</div>
          <p className="muted" style={{ marginTop: 0, lineHeight: 1.65, fontSize: 14 }}>{briefingText}</p>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 16 }}>
            <button className="btn sm" type="button" onClick={() => navigate("/insights")}>Read full insights</button>
            <CopilotNudge prompt="Give me the short version of what changed financially this week and what I should do next." label="Ask follow-up ⌘K" variant="accent" />
          </div>
        </div>

        {celebrateMilestones.length > 0 ? (
          <div className="card accent">
            <div className="eyebrow" style={{ color: "var(--accent)", marginBottom: 8 }}><span className="dot" />Milestone unlocked</div>
            <div className="h3" style={{ marginBottom: 10 }}>Net worth crossed {milestoneLabel(celebrateMilestones[0]!, primaryCurrency)}</div>
            <p className="muted" style={{ lineHeight: 1.6 }}>Quiet compounding is working. Take a moment, then decide where the next increment should go.</p>
            <button className="btn ghost sm" type="button" onClick={() => setDismissedMilestones((prev) => [...prev, celebrateMilestones[0]!])}>Dismiss</button>
          </div>
        ) : showSweep && totals ? <SmartSweepCard netCents={totals.netCents} onDismiss={() => setSweepDismissed(true)} /> : <HealthScoreCard score={healthScore} savingsPoints={savingsRateHistory} />}
      </section>

      {shouldShowMonthlyReview && <section className="section"><div className="card"><div className="row" style={{ justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}><div><div className="eyebrow" style={{ marginBottom: 6 }}>Month in review</div><div className="h3">Capture this month’s snapshot before the calendar rolls over.</div></div><button className="btn primary" type="button" disabled={createMonthlyReview.isPending} onClick={async () => { try { const nowDate = new Date(); await createMonthlyReview.mutateAsync({ year: nowDate.getFullYear(), month: nowDate.getMonth() + 1, notes: null }); toast.success("Monthly review saved", { description: "Open Reports to revisit it later." }); navigate("/reports"); } catch { toast.error("Could not create monthly review"); } }}>{createMonthlyReview.isPending ? "Saving…" : "Save review"}</button></div></div></section>}

      {activeCats.length > 0 && <section className="section"><div className="card"><div className="row" style={{ justifyContent: "space-between", gap: 16, alignItems: "flex-end", flexWrap: "wrap", marginBottom: 14 }}><div><div className="eyebrow" style={{ marginBottom: 6 }}>Spent this month</div><div className="figure money" style={{ fontSize: 44, lineHeight: 1 }}>{money(totalSpendRaw)}</div></div><button className="btn sm" type="button" onClick={() => navigate("/categories")}>Open categories →</button></div><div className="stream" style={{ height: 16, marginBottom: 18 }}>{activeCats.map((c) => <span key={c.id} title={`${c.label}: ${money(c.thisMonthCents)}`} style={{ width: `${(c.thisMonthCents / totalSpend) * 100}%`, background: c.color || "var(--ink-faint)" }} />)}</div><div style={{ display: "grid", gridTemplateColumns: "repeat(5, minmax(0, 1fr))", gap: 12 }}>{activeCats.slice(0, 5).map((c) => { const delta = c.thisMonthCents - c.lastMonthCents; const deltaLabel = c.lastMonthCents > 0 ? `${delta >= 0 ? "+" : "-"}${money(Math.abs(delta))} vs last month` : "New activity this month"; return <div key={c.id} className="card tight" style={{ padding: 16, minWidth: 0 }}><div className="row row-sm" style={{ marginBottom: 8 }}><span className="cswatch" style={{ background: c.color || "var(--ink-faint)" }} /><span className="strong" style={{ fontSize: 13.5 }}>{c.label}</span></div><div className="figure money" style={{ fontSize: 20 }}>{money(c.thisMonthCents)}</div><div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>{deltaLabel}</div></div>; })}</div></div></section>}

      <section className="section" style={{ display: "grid", gridTemplateColumns: "minmax(0, 1.1fr) minmax(320px, 0.9fr)", gap: 16 }}>
        <div className="card"><div className="eyebrow" style={{ marginBottom: 10 }}><span className="dot" />Agent · while you were away</div><AgentActivityFeed /><div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 8 }}>{needsReview > 0 && <button className="chip warning" style={{ cursor: "pointer" }} onClick={() => navigate("/accounts")} type="button">{needsReview} transaction{needsReview === 1 ? "" : "s"} need{needsReview === 1 ? "s" : ""} review →</button>}{(agentStatus?.anomalyCount ?? 0) > 0 && <button className="chip warning" style={{ cursor: "pointer" }} onClick={() => navigate("/accounts")} type="button">{agentStatus!.anomalyCount} unusual charge{agentStatus!.anomalyCount === 1 ? "" : "s"} flagged →</button>}{needsReview === 0 && (agentStatus?.anomalyCount ?? 0) === 0 && <span className="muted" style={{ fontSize: 12.5 }}>Nothing needs your attention right now.</span>}</div></div>
        <div className="card"><div className="eyebrow" style={{ marginBottom: 10 }}>{recurringSoon.length > 0 ? "Due in the next two weeks" : "Recurring commitments"}</div>{upcomingRecurring.length === 0 ? <div className="muted">No recurring subscriptions or bills detected yet.</div> : <div className="table-wrap" style={{ border: "none", background: "transparent" }}><table className="tbl"><thead><tr><th>Merchant</th><th>{recurringSoon.length > 0 ? "Due" : "Cadence"}</th><th className="right">Amount</th></tr></thead><tbody>{upcomingRecurring.map((item) => <tr key={`${item.merchantRaw}-${item.nextExpected}`}><td><div className="row row-sm"><span className="cswatch" style={{ background: item.categoryColor || "var(--ink-faint)" }} /><span>{item.merchantRaw}</span></div></td><td className="muted tabular">{daysUntilLabel(item.nextExpected) ?? item.cadence}</td><td className="right"><span className={`money num ${item.lastAmountCents > 0 ? "pos" : ""}`}>{money(Math.abs(item.lastAmountCents))}</span></td></tr>)}</tbody></table></div>}<div style={{ marginTop: 18 }}><div className="eyebrow" style={{ marginBottom: 8 }}>Cashflow trend</div><SavingsRateSparkline points={savingsRateHistory} /></div></div>
      </section>

      <CopilotQuickAsk prompt="Based on my spending this month, what adjustments should I make?" label="Ask Copilot about today" />
    </div>
  );
}

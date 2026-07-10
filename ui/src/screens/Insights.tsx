import { useState, useMemo, useRef, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { useBudgetEnvelopes } from "../api/hooks/budget";
import { useGoals } from "../api/hooks/budget";
import { useQuery } from "@tanstack/react-query";
import { commands, type MonthTotals, type RecurringItem } from "../api/client";
import * as I from "../components/Icons";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";
import { useAgentMemory, useForgetAgentMemory } from "../api/hooks/agentMemory";
import { useTriggerCategorize, useAgentStatus } from "../api/hooks/agent";
import { money } from "../utils/format";
import { monthlyEquivalentCents } from "../utils/recurring";
import { useFinancialMetrics } from "../api/hooks/metrics";
import { CopilotNudge } from "../components/CopilotNudge";
import { getAccountDisplayName } from "../utils/accounts";

function AgentStatusBar() {
  const [tickerIdx, setTickerIdx] = useState(0);
  const triggerCategorize = useTriggerCategorize();
  const { data: status } = useAgentStatus();

  const tickers = useMemo(() => {
    const msgs: string[] = [];
    if (status) {
      if (status.lastScanAt) {
        const mins = Math.round(
          (Date.now() - new Date(status.lastScanAt).getTime()) / 60_000
        );
        const when = mins < 2 ? "just now" : mins < 60 ? `${mins} mins ago` : `${Math.round(mins / 60)}h ago`;
        const categorized = status.lastScanCategorized ?? 0;
        msgs.push(`Last scan: ${when} · ${categorized} categorized`);
      }
      if (status.uncategorizedCount > 0)
        msgs.push(`${status.uncategorizedCount} transaction${status.uncategorizedCount !== 1 ? "s" : ""} uncategorized`);
      if (status.anomalyCount > 0)
        msgs.push(`${status.anomalyCount} anomal${status.anomalyCount !== 1 ? "ies" : "y"} flagged`);
      if (status.overBudgetCount > 0)
        msgs.push(`${status.overBudgetCount} budget envelope${status.overBudgetCount !== 1 ? "s" : ""} over limit`);
      if (status.upcomingBillsCount > 0)
        msgs.push(`${status.upcomingBillsCount} bill${status.upcomingBillsCount !== 1 ? "s" : ""} due soon`);
    }
    if (msgs.length === 0) msgs.push("All clear · no issues found");
    return msgs;
  }, [status]);

  useEffect(() => {
    setTickerIdx(0);
  }, [tickers]);

  useEffect(() => {
    if (tickers.length <= 1) return;
    const t = setInterval(() => setTickerIdx((i) => (i + 1) % tickers.length), 2400);
    return () => clearInterval(t);
  }, [tickers]);

  return (
    <Card tone="accent" className="row-md" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 24 }} tight>
      <div className="row-sm">
        <span className="dot" aria-hidden="true" />
        <span style={{ fontSize: 13.5, fontWeight: 500 }}>Agent · running locally</span>
      </div>
      <div className="grow" style={{ textAlign: "center" }}>
        <span className="num muted" style={{ fontSize: 12.5 }}>
          {status === undefined ? "Initializing…" : tickers[tickerIdx % tickers.length]}
        </span>
      </div>
      <Button
        variant="ghost"
        size="sm"
        loading={triggerCategorize.isPending}
        disabled={triggerCategorize.isPending}
        onClick={async () => {
          try {
            await triggerCategorize.mutateAsync();
            toast.success("Scan complete");
          } catch {
            toast.error("Scan failed");
          }
        }}
      >
        {triggerCategorize.isPending ? "Scanning…" : "Re-run scan"}
      </Button>
    </Card>
  );
}

interface Insight {
  id: string;
  kind: "pattern" | "anomaly" | "subscription" | "goal" | "budget" | "savings";
  headline: string;
  body: string;
  action?: string;
  actionRoute?: string;
  severity: "info" | "warn" | "positive";
}

const KIND_TONES: Record<string, "default" | "accent" | "positive" | "negative" | "warning"> = {
  pattern:      "accent",
  anomaly:      "negative",
  subscription: "warning",
  goal:         "accent",
  budget:       "warning",
  savings:      "positive",
};

const KIND_LABELS: Record<string, string> = {
  pattern:      "Pattern",
  anomaly:      "Anomaly",
  subscription: "Subscription",
  goal:         "Goal",
  budget:       "Budget",
  savings:      "Savings",
};

function InsightCard({ ins, onDismiss }: { ins: Insight; onDismiss: (id: string) => void }) {
  const navigate = useNavigate();
  return (
    <Card
      className="stack stack-md"
      style={{ borderLeftWidth: 3, borderLeftColor: "var(--accent)" }}
    >
      <div className="row-md" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
        <div className="row-sm wrap">
          <Badge tone={KIND_TONES[ins.kind] ?? "default"}>{KIND_LABELS[ins.kind]}</Badge>
          <Badge tone={ins.severity === "warn" ? "warning" : ins.severity === "positive" ? "positive" : "default"}>
            {ins.severity === "warn" ? "needs attention" : ins.severity === "positive" ? "good news" : "FYI"}
          </Badge>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onDismiss(ins.id)}
          aria-label="Dismiss insight"
          style={{ padding: "3px 8px" }}
        >
          <I.X width={12} height={12} />
        </Button>
      </div>

      <div className="stack stack-xs">
        <div style={{ fontSize: 15.5, fontWeight: 600, letterSpacing: "-0.01em" }}>
          {ins.headline}
        </div>
        <p className="muted" style={{ fontSize: 14, lineHeight: 1.6, margin: 0 }}>{ins.body}</p>
      </div>

      {ins.action && (
        <div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => ins.actionRoute && navigate(ins.actionRoute)}
          >
            {ins.action} →
          </Button>
        </div>
      )}
    </Card>
  );
}

export default function Insights() {
  const { data: accounts = [] } = useAccounts();
  const { data: cats = [] } = useCategoriesWithSpending();
  const { data: envelopes = [] } = useBudgetEnvelopes();
  const { data: goals = [] } = useGoals();
  const { data: metrics } = useFinancialMetrics();
  const savingsTarget = metrics?.targetSavingsRatePct ?? 20;

  const { data: totals } = useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      const result = await commands.getMonthTotals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });

  const { data: recurring = [] } = useQuery<RecurringItem[]>({
    queryKey: ["recurring"],
    queryFn: async () => {
      const result = await commands.listRecurring();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });

  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  const { data: memory = [] } = useAgentMemory();
  const forgetMemory = useForgetAgentMemory();
  const [pendingForget, setPendingForget] = useState<Set<string>>(new Set());
  const forgetTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  useEffect(() => {
    const timers = forgetTimers.current;
    return () => { timers.forEach((t) => clearTimeout(t)); timers.clear(); };
  }, []);

  const handleForget = (m: { id: string; description: string }) => {
    setPendingForget((s) => new Set([...s, m.id]));
    const timer = setTimeout(async () => {
      forgetTimers.current.delete(m.id);
      try {
        await forgetMemory.mutateAsync(m.id);
        setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
      } catch {
        setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
        toast.error("Could not forget that memory");
      }
    }, 5000);
    forgetTimers.current.set(m.id, timer);
    toast("Memory forgotten", {
      description: m.description.slice(0, 60),
      action: {
        label: "Undo",
        onClick: () => {
          const t = forgetTimers.current.get(m.id);
          if (t) { clearTimeout(t); forgetTimers.current.delete(m.id); }
          setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
        },
      },
    });
  };

  const visibleMemory = memory.filter((m) => !pendingForget.has(m.id));

  const rawInsights = useMemo<Insight[]>(() => {
    const insights: Insight[] = [];

    if (totals && totals.incomeCents > 0) {
      const rate = totals.savingsRatePct;
      if (rate >= savingsTarget) {
        insights.push({
          id: "savings-good",
          kind: "savings",
          headline: `${rate}% savings rate this month`,
          body: `You're keeping ${money(totals.netCents)} of ${money(totals.incomeCents)} income. That's at or above your ${savingsTarget}% target — well done.`,
          severity: "positive",
        });
      } else if (rate < 0) {
        insights.push({
          id: "savings-deficit",
          kind: "savings",
          headline: `Spending ${money(-totals.netCents)} more than earned this month`,
          body: `Income: ${money(totals.incomeCents)} · Expenses: ${money(totals.expenseCents)}. This month is running a deficit.`,
          action: "Review Budget",
          actionRoute: "/budget",
          severity: "warn",
        });
      } else {
        insights.push({
          id: "savings-low",
          kind: "savings",
          headline: `${rate}% savings rate — room to improve`,
          body: `You kept ${money(totals.netCents)} of ${money(totals.incomeCents)} this month. Reaching your ${savingsTarget}% target would add ${money(Math.round(totals.incomeCents * savingsTarget / 100) - totals.netCents)} to savings.`,
          action: "Open Budget",
          actionRoute: "/budget",
          severity: "info",
        });
      }
    }

    const overBudget = envelopes.filter((e) => e.budgetCents > 0 && e.spentCents > e.budgetCents);
    if (overBudget.length > 0) {
      const worst = overBudget.sort((a, b) => (b.spentCents - b.budgetCents) - (a.spentCents - a.budgetCents))[0];
      if (worst) {
        insights.push({
          id: `budget-over-${worst.categoryId}`,
          kind: "budget",
          headline: `${worst.categoryLabel} is over budget`,
          body: `Spent ${money(worst.spentCents)} vs ${money(worst.budgetCents)} budgeted — ${money(worst.spentCents - worst.budgetCents)} over.${overBudget.length > 1 ? ` Plus ${overBudget.length - 1} other ${overBudget.length - 1 === 1 ? "category" : "categories"}.` : ""}`,
          action: "Open Budget",
          actionRoute: "/budget",
          severity: "warn",
        });
      }
    }

    if (cats.length > 0) {
      const top = [...cats].sort((a, b) => b.thisMonthCents - a.thisMonthCents)[0];
      if (top && top.thisMonthCents > 0) {
        const vsLast = top.lastMonthCents > 0
          ? ` — ${top.thisMonthCents > top.lastMonthCents ? "↑" : "↓"} ${money(Math.abs(top.thisMonthCents - top.lastMonthCents))} vs last month`
          : "";
        insights.push({
          id: `top-cat-${top.id}`,
          kind: "pattern",
          headline: `${top.label} is your biggest expense this month`,
          body: `${money(top.thisMonthCents)} across ${top.txnCount} transactions${vsLast}.`,
          action: "See Categories",
          actionRoute: "/categories",
          severity: "info",
        });
      }
    }

    const spikes = cats.filter(
      (c) => c.lastMonthCents > 0 && c.thisMonthCents > c.lastMonthCents * 1.5 && c.thisMonthCents > 5000
    );
    if (spikes.length > 0) {
      const spike = spikes[0]!;
      const pct = Math.round(((spike.thisMonthCents - spike.lastMonthCents) / spike.lastMonthCents) * 100);
      insights.push({
        id: `spike-${spike.id}`,
        kind: "anomaly",
        headline: `${spike.label} up ${pct}% vs last month`,
        body: `${money(spike.lastMonthCents)} last month → ${money(spike.thisMonthCents)} this month. Worth reviewing.`,
        action: "See Transactions",
        actionRoute: "/accounts",
        severity: "warn",
      });
    }

    const subs = recurring.filter((r) => r.isSubscription && r.lastAmountCents < 0);
    if (subs.length > 0) {
      // Normalize each sub to a monthly figure so annual/weekly plans aren't
      // mis-annualized (a yearly plan is not 12× its charge).
      const monthlySubCost = subs.reduce((s, r) => s + monthlyEquivalentCents(r), 0);
      const annualCost = monthlySubCost * 12;
      insights.push({
        id: "subscriptions-cost",
        kind: "subscription",
        headline: `${subs.length} subscriptions totalling ${money(annualCost)}/year`,
        body: `That's ${money(monthlySubCost)}/month. Review if all are still being used.`,
        action: "See Subscriptions",
        actionRoute: "/recurring",
        severity: "info",
      });
    }

    const activeGoals = goals.filter((g) => g.targetCents > 0 && g.currentCents < g.targetCents);
    if (activeGoals.length > 0) {
      const onTrack = activeGoals.filter((g) => g.monthlyCents > 0);
      if (onTrack.length > 0) {
        insights.push({
          id: "goals-on-track",
          kind: "goal",
          headline: `${onTrack.length} goal${onTrack.length !== 1 ? "s" : ""} on track`,
          body: `${onTrack.map((g) => g.name).join(", ")} ${onTrack.length === 1 ? "is" : "are"} progressing with monthly contributions set.`,
          action: "See Goals",
          actionRoute: "/goals",
          severity: "positive",
        });
      }
      const stalled = activeGoals.filter((g) => g.monthlyCents === 0);
      if (stalled.length > 0) {
        insights.push({
          id: "goals-stalled",
          kind: "goal",
          headline: `${stalled.length} goal${stalled.length !== 1 ? "s" : ""} without a monthly contribution`,
          body: `${stalled.map((g) => g.name).join(", ")} ${stalled.length === 1 ? "has" : "have"} no monthly amount set — they won't progress automatically.`,
          action: "Set contributions",
          actionRoute: "/goals",
          severity: "warn",
        });
      }
    }

    const netWorth = accounts.reduce((s, a) => s + a.balance_cents, 0);
    if (netWorth > 0 && accounts.length > 1) {
      const highest = [...accounts].sort((a, b) => b.balance_cents - a.balance_cents)[0];
      insights.push({
        id: "net-worth",
        kind: "pattern",
        headline: `Net worth across ${accounts.length} accounts: ${money(netWorth)}`,
        body: highest ? `Your highest balance is in ${getAccountDisplayName(highest)}.` : "",
        severity: "info",
      });
    }

    return insights;
  }, [totals, cats, envelopes, goals, recurring, accounts, savingsTarget]);

  const visible = rawInsights.filter((i) => !dismissed.has(i.id));

  const handleDismiss = (id: string) => {
    const ins = rawInsights.find((i) => i.id === id);
    setDismissed((s) => new Set([...s, id]));
    toast("Insight dismissed", {
      description: ins?.headline.slice(0, 60),
      action: {
        label: "Undo",
        onClick: () => setDismissed((s) => { const n = new Set(s); n.delete(id); return n; }),
      },
    });
  };

  const [filter, setFilter] = useState("all");
  const kinds = [...new Set(rawInsights.map((i) => i.kind))];
  const filtered = filter === "all" ? visible : visible.filter((i) => i.kind === filter);

  return (
    <div className="screen">
      <AgentStatusBar />

      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" style={{ background: "var(--accent)", boxShadow: "0 0 6px var(--accent)" }} />Insights · {new Date().toLocaleDateString("en-US", { month: "long" })}</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>What the numbers are saying.</h1>
        </div>
        <div className="row-md wrap">
          {dismissed.size > 0 && (
            <Button variant="ghost" size="sm" onClick={() => setDismissed(new Set())}>
              Restore {dismissed.size} dismissed
            </Button>
          )}
          <CopilotNudge
            prompt="Based on these insights, what should I focus on and what actions should I take?"
            label="Ask Copilot to make a plan"
            variant="accent"
          />
        </div>
      </header>

      <p className="muted" style={{ maxWidth: 660, fontSize: 14, lineHeight: 1.6, marginTop: -12, marginBottom: 24 }}>
        These insights are generated locally from your data — no network calls, no tracking. Each one is a pattern your data surfaced.
      </p>

      {kinds.length > 1 && (
        <nav className="toolbar" style={{ marginBottom: 20, display: "inline-flex" }} aria-label="Insight filters">
          <button className={filter === "all" ? "on" : ""} onClick={() => setFilter("all")}>
            All <span style={{ color: "var(--ink-faint)", marginLeft: 4, fontSize: 11 }}>{visible.length}</span>
          </button>
          {kinds.map((k) => {
            const count = visible.filter((i) => i.kind === k).length;
            if (count === 0) return null;
            return (
              <button key={k} className={filter === k ? "on" : ""} onClick={() => setFilter(k)}>
                {KIND_LABELS[k]} <span style={{ color: "var(--ink-faint)", marginLeft: 4, fontSize: 11 }}>{count}</span>
              </button>
            );
          })}
        </nav>
      )}

      {filtered.length === 0 ? (
        <EmptyState
          icon={<I.Sparkle style={{ color: "var(--accent)", width: 32, height: 32 }} />}
          title={visible.length === 0 ? "No insights yet" : "No insights in this category"}
          description={
            visible.length === 0
              ? "Import more transactions or set budgets and goals to generate insights."
              : "Switch to All to see all active insights."
          }
          compact
        />
      ) : (
        <div className="stack stack-md" role="list" aria-label="Insights">
          {filtered.map((ins) => (
            <div key={ins.id} role="listitem">
              <InsightCard ins={ins} onDismiss={handleDismiss} />
            </div>
          ))}
        </div>
      )}

      {visibleMemory.length > 0 && (
        <section className="stack stack-md" style={{ marginTop: 40 }}>
          <div className="eyebrow">What the agent has learned</div>
          <Card flush>
            <ul className="stack" style={{ margin: 0, padding: 0, listStyle: "none" }}>
              {visibleMemory.map((m) => (
                <li
                  key={m.id}
                  className="row-md"
                  style={{ padding: "10px 16px", borderBottom: "1px solid var(--hairline)", alignItems: "center" }}
                >
                  <div className="grow" style={{ fontSize: 14, minWidth: 0 }}>{m.description}</div>
                  <Button variant="ghost" size="sm" onClick={() => handleForget(m)} aria-label={`Forget: ${m.description}`}>
                    Forget
                  </Button>
                </li>
              ))}
            </ul>
          </Card>
        </section>
      )}
    </div>
  );
}

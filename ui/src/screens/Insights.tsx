import { useState, useMemo } from "react";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { useBudgetEnvelopes } from "../api/hooks/budget";
import { useGoals } from "../api/hooks/budget";
import { useQuery } from "@tanstack/react-query";
import { commands, type MonthTotals, type RecurringItem } from "../api/client";
import * as I from "../components/Icons";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(cents / 100);
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

const KIND_COLORS: Record<string, string> = {
  pattern:      "var(--c-transport)",
  anomaly:      "var(--negative)",
  subscription: "var(--c-subs)",
  goal:         "var(--accent)",
  budget:       "var(--warning)",
  savings:      "var(--positive)",
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
  const color = KIND_COLORS[ins.kind] ?? "var(--ink-mute)";

  return (
    <div className="card" style={{ padding: 22, borderLeft: `3px solid ${color}` }}>
      <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 16, marginBottom: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span className="chip" style={{
            fontSize: 11,
            background: color + "22",
            color,
            border: `1px solid ${color}44`,
          }}>
            {KIND_LABELS[ins.kind]}
          </span>
          <span
            className={`chip ${ins.severity === "warn" ? "warning" : ins.severity === "positive" ? "positive" : ""}`}
            style={{ fontSize: 11 }}
          >
            {ins.severity === "warn" ? "needs attention" : ins.severity === "positive" ? "good news" : "FYI"}
          </span>
        </div>
        <button
          className="btn ghost sm"
          onClick={() => onDismiss(ins.id)}
          style={{ padding: "3px 8px" }}
          aria-label="Dismiss insight"
        >
          <I.X width="12" height="12" />
        </button>
      </div>

      <div style={{ fontSize: 15.5, fontWeight: 600, marginBottom: 8, letterSpacing: "-0.01em" }}>
        {ins.headline}
      </div>
      <div className="muted" style={{ fontSize: 14, lineHeight: 1.6 }}>{ins.body}</div>

      {ins.action && (
        <div style={{ marginTop: 14 }}>
          <button className="btn sm outline">{ins.action} →</button>
        </div>
      )}
    </div>
  );
}

export default function Insights() {
  const { data: accounts = [] } = useAccounts();
  const { data: cats = [] } = useCategoriesWithSpending();
  const { data: envelopes = [] } = useBudgetEnvelopes();
  const { data: goals = [] } = useGoals();

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

  // ── Generate insights from real data ───────────────────────────────────

  const rawInsights = useMemo<Insight[]>(() => {
    const insights: Insight[] = [];

    // 1. Savings rate
    if (totals && totals.incomeCents > 0) {
      const rate = totals.savingsRatePct;
      if (rate >= 20) {
        insights.push({
          id: "savings-good",
          kind: "savings",
          headline: `${rate}% savings rate this month`,
          body: `You're keeping ${fmt(totals.netCents)} of ${fmt(totals.incomeCents)} income. That's above the 20% benchmark — well done.`,
          severity: "positive",
        });
      } else if (rate < 0) {
        insights.push({
          id: "savings-deficit",
          kind: "savings",
          headline: `Spending ${fmt(-totals.netCents)} more than earned this month`,
          body: `Income: ${fmt(totals.incomeCents)} · Expenses: ${fmt(totals.expenseCents)}. This month is running a deficit.`,
          action: "Review Budget",
          actionRoute: "/budget",
          severity: "warn",
        });
      } else {
        insights.push({
          id: "savings-low",
          kind: "savings",
          headline: `${rate}% savings rate — room to improve`,
          body: `You kept ${fmt(totals.netCents)} of ${fmt(totals.incomeCents)} this month. Moving toward 20% would add ${fmt(Math.round(totals.incomeCents * 0.2) - totals.netCents)} to savings.`,
          action: "Open Budget",
          actionRoute: "/budget",
          severity: "info",
        });
      }
    }

    // 2. Budget overruns
    const overBudget = envelopes.filter((e) => e.budgetCents > 0 && e.spentCents > e.budgetCents);
    if (overBudget.length > 0) {
      const worst = overBudget.sort((a, b) => (b.spentCents - b.budgetCents) - (a.spentCents - a.budgetCents))[0];
      if (worst) {
        insights.push({
          id: `budget-over-${worst.categoryId}`,
          kind: "budget",
          headline: `${worst.categoryLabel} is over budget`,
          body: `Spent ${fmt(worst.spentCents)} vs ${fmt(worst.budgetCents)} budgeted — ${fmt(worst.spentCents - worst.budgetCents)} over.${overBudget.length > 1 ? ` Plus ${overBudget.length - 1} other ${overBudget.length - 1 === 1 ? "category" : "categories"}.` : ""}`,
          action: "Open Budget",
          actionRoute: "/budget",
          severity: "warn",
        });
      }
    }

    // 3. Top spending category
    if (cats.length > 0) {
      const top = [...cats].sort((a, b) => b.thisMonthCents - a.thisMonthCents)[0];
      if (top && top.thisMonthCents > 0) {
        const vsLast = top.lastMonthCents > 0
          ? ` — ${top.thisMonthCents > top.lastMonthCents ? "↑" : "↓"} ${fmt(Math.abs(top.thisMonthCents - top.lastMonthCents))} vs last month`
          : "";
        insights.push({
          id: `top-cat-${top.id}`,
          kind: "pattern",
          headline: `${top.label} is your biggest expense this month`,
          body: `${fmt(top.thisMonthCents)} across ${top.txnCount} transactions${vsLast}.`,
          action: "See Categories",
          actionRoute: "/categories",
          severity: "info",
        });
      }
    }

    // 4. Category spike (this month significantly up vs last)
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
        body: `${fmt(spike.lastMonthCents)} last month → ${fmt(spike.thisMonthCents)} this month. Worth reviewing.`,
        action: "See Transactions",
        actionRoute: "/transactions",
        severity: "warn",
      });
    }

    // 5. Subscriptions cost
    const subs = recurring.filter((r) => r.isSubscription && r.lastAmountCents < 0);
    if (subs.length > 0) {
      const monthlySubCost = subs.reduce((s, r) => s + Math.abs(r.lastAmountCents), 0);
      const annualCost = monthlySubCost * 12;
      insights.push({
        id: "subscriptions-cost",
        kind: "subscription",
        headline: `${subs.length} subscriptions totalling ${fmt(annualCost)}/year`,
        body: `That's ${fmt(monthlySubCost)}/month. Review if all are still being used.`,
        action: "See Subscriptions",
        actionRoute: "/recurring",
        severity: "info",
      });
    }

    // 6. Goals on track
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

    // 7. Net worth across accounts
    const netWorth = accounts.reduce((s, a) => s + a.balance_cents, 0);
    if (netWorth > 0 && accounts.length > 1) {
      const highest = [...accounts].sort((a, b) => b.balance_cents - a.balance_cents)[0];
      insights.push({
        id: "net-worth",
        kind: "pattern",
        headline: `Net worth across ${accounts.length} accounts: ${fmt(netWorth)}`,
        body: highest ? `Your highest balance is in ${highest.name}.` : "",
        severity: "info",
      });
    }

    return insights;
  }, [totals, cats, envelopes, goals, recurring, accounts]);

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
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" style={{ background: "var(--accent)", boxShadow: "0 0 6px var(--accent)" }} />
            Insights · {visible.length} active
          </div>
          <h1>What FinSight noticed.</h1>
        </div>
        {dismissed.size > 0 && (
          <button className="btn ghost sm" onClick={() => setDismissed(new Set())}>
            Restore {dismissed.size} dismissed
          </button>
        )}
      </div>

      <p className="muted" style={{ maxWidth: 660, fontSize: 14, lineHeight: 1.6, marginTop: -12, marginBottom: 24 }}>
        These insights are generated locally from your data — no network calls, no tracking. Each one is a pattern your data surfaced.
      </p>

      {/* Kind filter */}
      {kinds.length > 1 && (
        <div className="toolbar" style={{ marginBottom: 20, display: "inline-flex" }}>
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
        </div>
      )}

      {/* Insight cards */}
      {filtered.length === 0 ? (
        <div className="card" style={{ textAlign: "center", padding: "64px 32px" }}>
          <I.Sparkle style={{ color: "var(--accent)", width: 32, height: 32, margin: "0 auto 16px" }} />
          <div style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>
            {visible.length === 0 ? "No insights yet" : "No insights in this category"}
          </div>
          <div className="muted" style={{ fontSize: 14 }}>
            {visible.length === 0
              ? "Import more transactions or set budgets and goals to generate insights."
              : "Switch to All to see all active insights."}
          </div>
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {filtered.map((ins) => (
            <InsightCard key={ins.id} ins={ins} onDismiss={handleDismiss} />
          ))}
        </div>
      )}
    </div>
  );
}

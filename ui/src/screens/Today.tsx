import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useNeedsReviewCount } from "../api/hooks/agent";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { useGoals, useUpdateGoalBalance } from "../api/hooks/budget";
import { useRecurring } from "../api/hooks/recurring";
import { type AccountSummary } from "../api/client";
import { useMonthTotals } from "../api/hooks";
import AgentActivityFeed from "../components/AgentActivityFeed";
import { useNetWorth, useNetWorthHistory } from "../api/hooks/networth";
import NetWorthChart from "../components/NetWorthChart";
import { CopilotNudge } from "../components/CopilotNudge";
import { money } from "../utils/format";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";

const RANGES = [
  { key: "1M", days: 30 }, { key: "3M", days: 90 }, { key: "6M", days: 180 },
  { key: "1Y", days: 365 }, { key: "All", days: 36500 },
] as const;

function AccountDot({ account }: { account: AccountSummary }) {
  return (
    <div
      className="row"
      style={{ justifyContent: "space-between", padding: "10px 0", borderBottom: "1px solid var(--hairline)" }}
    >
      <div className="row row-sm">
        <span
          className="dot"
          style={{
            width: 8,
            height: 8,
            background: account.color || "var(--ink-faint)",
            boxShadow: `0 0 6px ${account.color || "var(--ink-faint)"}`,
          }}
        />
        <div className="stack stack-xs">
          <div style={{ fontSize: 14 }}>{account.name}</div>
          <div className="muted" style={{ fontSize: 12 }}>{account.bank}</div>
        </div>
      </div>
      <div className="num tabular money" style={{ fontSize: 14, fontWeight: 500 }}>
        {money(account.balance_cents, { currency: account.currency })}
      </div>
    </div>
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
      await updateBalance.mutateAsync({
        id: firstGoal.id,
        currentCents: firstGoal.currentCents + netCents,
      });
      toast.success(`Parked ${money(netCents)} in ${firstGoal.name}`);
      onDismiss();
    } catch {
      toast.error("Could not park funds");
    }
  };

  return (
    <Card tone="accent" style={{ marginBottom: 20 }}>
      <div className="eyebrow" style={{ marginBottom: 8, color: "var(--accent)" }}>✦ Opportunity</div>
      <div style={{ fontSize: 14, marginBottom: 12 }}>
        You have <span className="money">{money(netCents)}</span> unallocated this month.
      </div>
      <div className="row row-sm wrap">
        {firstGoal && (
          <Button variant="primary" size="sm" loading={updateBalance.isPending} onClick={() => void handlePark()}>
            Park in {firstGoal.name}
          </Button>
        )}
        <Button size="sm" onClick={() => navigate("/goals")}>Assign to a goal…</Button>
        <Button variant="ghost" size="sm" onClick={onDismiss} aria-label="dismiss">Dismiss</Button>
      </div>
    </Card>
  );
}

function daysUntilLabel(dateStr: string): string | null {
  const diff = Math.round((new Date(dateStr).getTime() - Date.now()) / 86400000);
  if (diff < 0 || diff > 7) return null;
  if (diff === 0) return "today";
  if (diff === 1) return "tomorrow";
  return `in ${diff} days`;
}

function UpcomingRecurring() {
  const navigate = useNavigate();
  const { data: items = [] } = useRecurring();
  const upcoming = items.filter((item) => daysUntilLabel(item.nextExpected) !== null);
  if (upcoming.length === 0) return null;

  return (
    <section className="section" aria-labelledby="due-soon-title">
      <div id="due-soon-title" className="eyebrow" style={{ marginBottom: 8 }}>Due soon</div>
      <div className="row row-sm wrap">
        {upcoming.map((item, i) => {
          const label = daysUntilLabel(item.nextExpected)!;
          const name = item.merchantRaw.length > 18
            ? item.merchantRaw.slice(0, 18) + "…"
            : item.merchantRaw;
          return (
            <Badge key={i}>
              <span
                className="dot"
                style={{
                  width: 7,
                  height: 7,
                  background: item.categoryColor || "var(--ink-faint)",
                }}
              />
              {name}
              <span className="num money" style={{ fontFamily: "var(--mono)", fontSize: 11.5 }}>
                {money(Math.abs(item.lastAmountCents))}
              </span>
              <span className="muted" style={{ fontSize: 11 }}>{label}</span>
            </Badge>
          );
        })}
        <Button variant="ghost" size="sm" onClick={() => navigate("/recurring")}>See all →</Button>
      </div>
    </section>
  );
}

export default function Today() {
  const navigate = useNavigate();
  const { data: accounts = [], isLoading: accLoading } = useAccounts();
  const { data: totals, isLoading: totLoading } = useMonthTotals();
  const { data: cats = [] } = useCategoriesWithSpending();
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const netWorth = useNetWorth();
  const [range, setRange] = useState<typeof RANGES[number]["key"]>("6M");
  const [sweepDismissed, setSweepDismissed] = useState(false);
  const days = RANGES.find((r) => r.key === range)!.days;
  const { data: nwHistory = [] } = useNetWorthHistory(days);

  const now = new Date();
  const dateLabel = now.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" });
  const monthLabel = now.toLocaleString("default", { month: "long" });
  const primaryCurrency = accounts[0]?.currency ?? "USD";
  const isLoading = accLoading || totLoading;

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
      <div className="empty-state">
        <section className="empty-panel" aria-labelledby="today-empty-title">
          <div className="eyebrow">First step</div>
          <h2 id="today-empty-title">No accounts yet. Add your first account to unlock Today.</h2>
          <p>
            Import a CSV statement, add accounts by hand, or load the demo dataset from Settings
            to explore FinSight before connecting real data.
          </p>
          <div className="empty-actions">
            <Button variant="primary" onClick={() => navigate("/onboarding")}>Start setup</Button>
            <Button onClick={() => navigate("/accounts")}>Add manually</Button>
            <Button variant="ghost" onClick={() => navigate("/settings")}>Load demo data</Button>
          </div>
        </section>
      </div>
    );
  }

  const activeCats = cats.filter((c) => c.thisMonthCents > 0);
  const totalSpend = activeCats.reduce((s, c) => s + c.thisMonthCents, 0) || 1;

  const dayOfMonth = now.getDate();
  const avgDailyBurn = totals ? totals.expenseCents / dayOfMonth : 0;
  const runwayDays = avgDailyBurn > 0 ? Math.max(0, Math.round(netWorth / avgDailyBurn)) : null;
  const showSweep = !sweepDismissed && !!totals && totals.netCents > 5000;
  const shouldShowSavingsNudge = !!totals && totals.incomeCents > 0 && totals.savingsRatePct < 10;
  const savingsColor =
    !totals ? "var(--ink)" :
    totals.savingsRatePct < 10 ? "var(--negative)" :
    totals.savingsRatePct < 15 ? "var(--warning)" :
    "var(--accent)";

  return (
    <div className="screen">
      {/* Date header */}
      <header style={{ marginBottom: 24 }}>
        <div className="eyebrow" style={{ marginBottom: 8 }}>
          <span className="dot" />
          {dateLabel}
        </div>

        {/* Net worth hero */}
        <div className="row row-lg wrap" style={{ alignItems: "baseline" }}>
          <div
            className="figure money"
            style={{ color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}
          >
            {money(netWorth, { currency: primaryCurrency })}
          </div>
          <div className="muted" style={{ fontSize: 16 }}>
            net worth · {accounts.length} account{accounts.length !== 1 ? "s" : ""} + assets − liabilities
          </div>
        </div>
      </header>

      {/* Net-worth chart */}
      <section>
        <div className="toolbar" style={{ marginBottom: 10, display: "inline-flex" }}>
          {RANGES.map((r) => (
            <button
              key={r.key}
              className={range === r.key ? "on" : ""}
              onClick={() => setRange(r.key)}
              aria-pressed={range === r.key}
            >
              {r.key}
            </button>
          ))}
        </div>
        <NetWorthChart points={nwHistory} />
      </section>

      {/* Smart Sweep card */}
      {showSweep && totals && (
        <SmartSweepCard netCents={totals.netCents} onDismiss={() => setSweepDismissed(true)} />
      )}

      {/* 4-stat row */}
      {totals && (
        <section className="stat-row">
          <div className="stat">
            <div className="label">{monthLabel} income</div>
            <div className="value figure money num pos">{money(totals.incomeCents)}</div>
            <div className="sub muted">{totals.txnCount} transactions</div>
          </div>
          <div className="stat">
            <div className="label">{monthLabel} expenses</div>
            <div className="value figure money">{money(totals.expenseCents)}</div>
            <div className="sub muted">
              {money(totals.netCents) + (totals.netCents >= 0 ? " saved" : " deficit")}
            </div>
          </div>
          <div
            className="stat"
            style={{
              borderColor: shouldShowSavingsNudge ? "var(--negative)" : totals.savingsRatePct < 15 ? "var(--warning)" : "var(--accent-3)",
              background: shouldShowSavingsNudge ? "var(--surface-2)" : totals.savingsRatePct < 15 ? "var(--warning-2)" : "var(--accent-2)",
            }}
          >
            <div className="label">Savings rate</div>
            <div className={`value figure num${totals.savingsRatePct < 0 ? " neg" : ""}`} style={{ color: savingsColor }}>
              {totals.savingsRatePct}%
            </div>
            <div className="sub muted">
              {shouldShowSavingsNudge ? "Aim to keep 10% of what you earn." : "of income kept"}
            </div>
          </div>
          <div className="stat">
            <div className="label">Runway</div>
            <div
              className="value figure num"
              style={{ color: runwayDays !== null && runwayDays < 30 ? "var(--negative)" : undefined }}
            >
              {runwayDays !== null ? runwayDays.toLocaleString() : "—"}
            </div>
            <div className="sub muted">
              {runwayDays !== null ? "days · at current burn" : "no burn data"}
            </div>
          </div>
        </section>
      )}

      {shouldShowSavingsNudge && totals && (
        <div style={{ marginTop: 14 }}>
          <CopilotNudge
            prompt={`My savings rate is only ${totals.savingsRatePct}%. Help me apply the 'pay yourself first' principle and set up a plan to save at least 10% of my income every month.`}
            label="Pay yourself first — get to 10% savings"
            variant="warning"
          />
        </div>
      )}

      {/* Category stream bar */}
      {activeCats.length > 0 && (
        <section className="section">
          <div className="eyebrow" style={{ marginBottom: 8 }}>Spending this month</div>
          <div className="stream">
            {activeCats.map((c) => (
              <span
                key={c.id}
                title={`${c.label}: ${money(c.thisMonthCents)}`}
                style={{ width: `${(c.thisMonthCents / totalSpend) * 100}%`, background: c.color || "var(--ink-faint)" }}
              />
            ))}
          </div>
          <div className="row row-sm wrap" style={{ marginTop: 8 }}>
            {activeCats.slice(0, 6).map((c) => (
              <span key={c.id} className="row row-xs" style={{ fontSize: 12, color: "var(--ink-mute)" }}>
                <span className="swatch" style={{ background: c.color || "var(--ink-faint)" }} />
                {c.label}
                <span className="tabular" style={{ color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 11 }}>
                  {money(c.thisMonthCents)}
                </span>
              </span>
            ))}
          </div>
        </section>
      )}

      {/* Upcoming recurring chips */}
      <UpcomingRecurring />

      {/* Agent feed + needs-review */}
      <section className="section">
        <AgentActivityFeed />

        {needsReview > 0 && (
          <button
            className="chip warning"
            style={{ marginTop: 12, cursor: "pointer", border: "none", padding: "8px 14px", fontSize: 13 }}
            onClick={() => navigate("/transactions")}
            aria-label={`${needsReview} transactions need review`}
          >
            ⚠ {needsReview} transaction{needsReview === 1 ? "" : "s"} need{needsReview === 1 ? "s" : ""} review →
          </button>
        )}
      </section>

      {/* Account list */}
      {accounts.length > 1 && (
        <section className="section">
          <Card tight header={<div className="eyebrow">All accounts</div>}>
            <div style={{ padding: "0 18px" }}>
              {accounts.map((a) => <AccountDot key={a.id} account={a} />)}
            </div>
          </Card>
        </section>
      )}
    </div>
  );
}

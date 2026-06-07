import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import { useNeedsReviewCount } from "../api/hooks/agent";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { useGoals, useUpdateGoalBalance } from "../api/hooks/budget";
import { useRecurring } from "../api/hooks/recurring";
import { commands, type MonthTotals, type AccountSummary } from "../api/client";
import AgentActivityFeed from "../components/AgentActivityFeed";
import { useNetWorth, useNetWorthHistory } from "../api/hooks/networth";
import NetWorthChart from "../components/NetWorthChart";
import { money } from "../utils/format";

const RANGES = [
  { key: "1M", days: 30 }, { key: "3M", days: 90 }, { key: "6M", days: 180 },
  { key: "1Y", days: 365 }, { key: "All", days: 36500 },
] as const;

function useMonthTotals() {
  return useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      const result = await commands.getMonthTotals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    refetchInterval: 60_000,
  });
}

function AccountDot({ account }: { account: AccountSummary }) {
  return (
    <div style={{
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      padding: "10px 0",
      borderBottom: "1px solid var(--hairline)",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span style={{
          width: 8,
          height: 8,
          borderRadius: 999,
          background: account.color || "var(--ink-faint)",
          boxShadow: `0 0 6px ${account.color || "var(--ink-faint)"}`,
          display: "inline-block",
          flexShrink: 0,
        }} />
        <div>
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
    <div className="card" style={{ padding: "16px 20px", border: "1px solid var(--accent)",
      borderRadius: 10, marginBottom: 20 }}>
      <div className="eyebrow" style={{ marginBottom: 8, color: "var(--accent)" }}>✦ Opportunity</div>
      <div style={{ fontSize: 14, marginBottom: 12 }}>
        You have <span className="money">{money(netCents)}</span> unallocated this month.
      </div>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        {firstGoal && (
          <button className="btn primary sm" disabled={updateBalance.isPending}
            onClick={() => void handlePark()}>
            Park in {firstGoal.name}
          </button>
        )}
        <button className="btn sm" onClick={() => navigate("/goals")}>Assign to a goal…</button>
        <button className="btn ghost sm" onClick={onDismiss} aria-label="dismiss">Dismiss</button>
      </div>
    </div>
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
    <div style={{ marginTop: 16, marginBottom: 16 }}>
      <div className="eyebrow" style={{ marginBottom: 8 }}>Due soon</div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        {upcoming.map((item, i) => {
          const label = daysUntilLabel(item.nextExpected)!;
          const name = item.merchantRaw.length > 18
            ? item.merchantRaw.slice(0, 18) + "…"
            : item.merchantRaw;
          return (
            <span key={i} style={{ display: "inline-flex", alignItems: "center", gap: 6,
              padding: "5px 10px", borderRadius: 999, background: "var(--surface-2)",
              fontSize: 12.5, border: "1px solid var(--line)" }}>
              <span style={{ width: 7, height: 7, borderRadius: 999,
                background: item.categoryColor || "var(--ink-faint)",
                display: "inline-block", flexShrink: 0 }} />
              {name}
              <span className="num money" style={{ fontFamily: "var(--mono)", fontSize: 11.5 }}>
                {money(Math.abs(item.lastAmountCents))}
              </span>
              <span className="muted" style={{ fontSize: 11 }}>{label}</span>
            </span>
          );
        })}
        <button className="btn ghost sm" style={{ fontSize: 12, padding: "4px 10px" }}
          onClick={() => navigate("/recurring")}>See all →</button>
      </div>
    </div>
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

  if (isLoading) return <div className="stub">Loading…</div>;
  if (accounts.length === 0) return <div className="stub">No accounts yet.</div>;

  const activeCats = cats.filter((c) => c.thisMonthCents > 0);
  const totalSpend = activeCats.reduce((s, c) => s + c.thisMonthCents, 0) || 1;

  const dayOfMonth = now.getDate();
  const avgDailyBurn = totals ? totals.expenseCents / dayOfMonth : 0;
  const runwayDays = avgDailyBurn > 0 ? Math.max(0, Math.round(netWorth / avgDailyBurn)) : null;
  const showSweep = !sweepDismissed && !!totals && totals.netCents > 5000;

  return (
    <div className="screen">
      {/* Date header */}
      <div style={{ marginBottom: 24 }}>
        <div className="eyebrow" style={{ marginBottom: 8 }}>
          <span className="dot" />
          {dateLabel}
        </div>

        {/* Net worth hero */}
        <div style={{ display: "flex", alignItems: "baseline", gap: 16, flexWrap: "wrap" }}>
          <div className="figure money" style={{
            fontSize: 64,
            lineHeight: 1,
            letterSpacing: "-0.035em",
            color: netWorth >= 0 ? "var(--ink)" : "var(--negative)",
          }}>
            {money(netWorth, { currency: primaryCurrency })}
          </div>
          <div className="muted" style={{ fontSize: 16 }}>
            net worth · {accounts.length} account{accounts.length !== 1 ? "s" : ""} + assets − liabilities
          </div>
        </div>
      </div>

      {/* Net-worth chart */}
      <div style={{ marginBottom: 20 }}>
        <div className="toolbar" style={{ marginBottom: 10, display: "inline-flex" }}>
          {RANGES.map((r) => (
            <button key={r.key} className={range === r.key ? "on" : ""} onClick={() => setRange(r.key)}>
              {r.key}
            </button>
          ))}
        </div>
        <NetWorthChart points={nwHistory} />
      </div>

      {/* §3b Smart Sweep card */}
      {showSweep && totals && (
        <SmartSweepCard netCents={totals.netCents} onDismiss={() => setSweepDismissed(true)} />
      )}

      {/* §3d 4-stat row (with Runway replacing Accounts) */}
      {totals && (
        <div className="stat-row">
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
          <div className={`stat${totals.savingsRatePct > 0 ? " accent" : ""}`}>
            <div className="label">Savings rate</div>
            <div className={`value figure num${totals.savingsRatePct > 0 ? "" : " neg"}`}>
              {totals.savingsRatePct}%
            </div>
            <div className="sub muted">of income kept</div>
          </div>
          <div className="stat">
            <div className="label">Runway</div>
            <div className="value figure num" style={{
              color: runwayDays !== null && runwayDays < 30 ? "var(--negative)" : undefined,
            }}>
              {runwayDays !== null ? runwayDays.toLocaleString() : "—"}
            </div>
            <div className="sub muted">
              {runwayDays !== null ? "days · at current burn" : "no burn data"}
            </div>
          </div>
        </div>
      )}

      {/* Category stream bar */}
      {activeCats.length > 0 && (
        <div style={{ marginTop: 20 }}>
          <div className="eyebrow" style={{ marginBottom: 8 }}>Spending this month</div>
          <div className="stream" style={{ height: 14, borderRadius: 8 }}>
            {activeCats.map((c) => (
              <span
                key={c.id}
                title={`${c.label}: ${money(c.thisMonthCents)}`}
                style={{
                  width: `${(c.thisMonthCents / totalSpend) * 100}%`,
                  background: c.color || "var(--ink-faint)",
                }}
              />
            ))}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "6px 16px", marginTop: 8 }}>
            {activeCats.slice(0, 6).map((c) => (
              <span key={c.id} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--ink-mute)" }}>
                <span style={{ width: 8, height: 8, borderRadius: 2, background: c.color || "var(--ink-faint)", display: "inline-block" }} />
                {c.label}
                <span className="tabular" style={{ color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 11 }}>
                  {money(c.thisMonthCents)}
                </span>
              </span>
            ))}
          </div>
        </div>
      )}

      {/* §3c Upcoming recurring chips */}
      <UpcomingRecurring />

      {/* Agent feed + needs-review */}
      <div style={{ marginTop: 24 }}>
        <AgentActivityFeed />

        {needsReview > 0 && (
          <button
            onClick={() => navigate("/transactions")}
            className="chip warning"
            style={{ marginTop: 12, cursor: "pointer", border: "none", padding: "8px 14px", fontSize: 13 }}
            aria-label={`${needsReview} transactions need review`}
          >
            ⚠ {needsReview} transaction{needsReview === 1 ? "" : "s"} need{needsReview === 1 ? "s" : ""} review →
          </button>
        )}
      </div>

      {/* Account list */}
      {accounts.length > 1 && (
        <div className="section">
          <div className="card tight">
            <div className="card-head" style={{ padding: "12px 18px" }}>
              <div className="eyebrow">All accounts</div>
            </div>
            <div style={{ padding: "0 18px" }}>
              {accounts.map((a) => <AccountDot key={a.id} account={a} />)}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

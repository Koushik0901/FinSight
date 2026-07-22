import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { useMonthTotals } from "../api/hooks/reports";
import { useAccounts } from "../api/hooks/accounts";
import { useGoals, useCreateGoal, useUpdateGoalMonthly, useProjectGoalGrowth } from "../api/hooks/budget";
import { useGoalExplanations } from "../api/hooks/metrics";
import type { GoalDto, NewGoalInput } from "../api/client";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";
import GoalDrawer from "../components/GoalDrawer";
import EmptyState from "../components/EmptyState";
import { ExplainDrawer } from "../components/ExplainInspector";

type GoalFilter = "all" | "save-by-date" | "build-balance" | "debt-payoff" | "spending-cap" | "sinking-fund";

const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Pay off debt",
  "spending-cap": "Spending cap",
  "sinking-fund": "Sinking fund",
};

/// Written as what the user is saying about the goal, not as abstract levels —
/// "Nice to have" is answerable; "Someday" is a category to decode.
const PRIORITY_LABELS: Record<string, string> = {
  critical: "Must fund first",
  high: "Important",
  normal: "Normal",
  someday: "Nice to have",
};

const STRICTNESS_LABELS: Record<string, string> = {
  hard: "Fixed — the date can't move",
  target: "A target I'm aiming for",
  none: "No real deadline",
};

function paceLabel(goal: GoalDto) {
  // Spending caps invert the usual direction: a full bar is bad, not "ahead".
  if (goal.goalType === "spending-cap") {
    if (goal.targetCents <= 0) return { label: "No cap set", className: "chip warning" };
    const used = goal.currentCents / goal.targetCents;
    if (goal.currentCents > goal.targetCents) return { label: "Over cap", className: "chip negative" };
    if (used > 0.9) return { label: "Near cap", className: "chip warning" };
    return { label: "Within cap", className: "chip positive" };
  }
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return { label: "Funded", className: "chip positive" };
  if (goal.monthlyCents <= 0) return { label: "Needs attention", className: "chip warning" };
  // With a target date, "on track" means the projected ETA lands on or before
  // it — not merely that the bar is nearly full.
  if (goal.targetDate) {
    return isBehindSchedule(goal, monthsToGoal(goal))
      ? { label: "Behind", className: "chip warning" }
      : { label: "On track", className: "chip accent" };
  }
  return { label: "On track", className: "chip accent" };
}

function GoalCard({ goal, onEdit, onExplain, linkedAccountName, onTogglePause, pausePending, pausedByUser }: { goal: GoalDto; onEdit: (goal: GoalDto) => void; onExplain: (goal: GoalDto) => void; linkedAccountName: string | null; onTogglePause: (goal: GoalDto) => void; pausePending: boolean; pausedByUser: boolean }) {
  const pct = goal.targetCents > 0 ? Math.min(100, Math.round((goal.currentCents / goal.targetCents) * 100)) : 0;
  const pace = paceLabel(goal);
  const canPause = goal.goalType !== "spending-cap" && goal.goalType !== "debt-payoff";
  const isPaused = canPause && goal.monthlyCents === 0;

  return (
    <div className="card" style={{ padding: 22 }}>
      <div style={{ display: "grid", gridTemplateColumns: "1.5fr 1fr 1fr", gap: 24, alignItems: "center" }}>
        <div>
          <div className="row row-sm wrap" style={{ marginBottom: 10 }}>
            <span className="chip">{TYPE_LABELS[goal.goalType] || goal.goalType}</span>
            {canPause && pausedByUser && <span className="chip warning">Paused</span>}
            <span className={pace.className}>{pace.label}</span>
          </div>
          <h2 className="h1" style={{ fontSize: 24 }}>{goal.name}</h2>
          <div className="muted" style={{ marginTop: 6 }}>
            {goal.goalType === "debt-payoff"
              ? `Paying ${money(goal.monthlyCents)}/month`
              : goal.goalType === "spending-cap"
                ? `Cap of ${money(goal.targetCents)} this month`
                : `Auto-moves ${money(goal.monthlyCents)}/month`}
            {goal.targetDate && ` · target ${new Date(goal.targetDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}`}
          </div>
          {goal.accountId && linkedAccountName && <div className="muted" style={{ marginTop: 8 }}>Linked to {linkedAccountName}</div>}
        </div>

        <div>
          <div className="eyebrow">{goal.goalType === "spending-cap" ? "This month" : "Progress"}</div>
          <div className={`goal-bar ${goal.goalType === "spending-cap" && goal.currentCents > goal.targetCents ? "negative" : ""}`} style={{ marginTop: 10 }}>
            <span style={{ width: `${pct}%` }} />
          </div>
          <div className="row" style={{ justifyContent: "space-between", marginTop: 8, fontSize: 13 }}>
            <span className="money">{money(goal.currentCents)}</span>
            <span className="money muted">of {money(goal.targetCents)}</span>
          </div>
        </div>

        <div style={{ textAlign: "right" }}>
          <div className="figure" style={{ fontSize: 34 }}>{pct}%</div>
          <div className="row row-sm" style={{ justifyContent: "flex-end", marginTop: 10 }}>
            {goal.goalType !== "spending-cap" && goal.goalType !== "debt-payoff" && (
              <button className="btn ghost sm" type="button" disabled={pausePending} onClick={() => onTogglePause(goal)}>{isPaused ? "Resume" : "Pause"}</button>
            )}
            {goal.goalType !== "spending-cap" && (
              <button className="btn ghost sm" type="button" onClick={() => onExplain(goal)}>Explain</button>
            )}
            <button className="btn outline sm" type="button" onClick={() => onEdit(goal)}>Adjust</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function monthsToGoal(goal: GoalDto, monthlyOverrideCents?: number) {
  const monthly = monthlyOverrideCents ?? goal.monthlyCents;
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return 0;
  if (monthly <= 0) return Infinity;
  return Math.ceil(remaining / monthly);
}

function etaLabel(months: number) {
  if (!Number.isFinite(months)) return "—";
  const date = new Date();
  // Normalize to the 1st before advancing months so month-end dates (e.g. the
  // 31st) don't skip a month via JS Date's day-overflow.
  date.setDate(1);
  date.setMonth(date.getMonth() + months);
  return date.toLocaleDateString("en-US", { month: "short", year: "numeric" });
}

type HorizonRow = {
  goal: GoalDto;
  months: number;
  pct: number;
  xPercent: number;
  needsAttention: boolean;
};

// A goal counts as "behind schedule" if its computed ETA lands later than its
// own targetDate. This is deliberately NOT paceLabel()'s "Needs attention"
// (monthlyCents <= 0): every goal in that state has an infinite ETA and is
// already excluded by the withEta filter below, which would make that branch
// permanently unreachable here. Goals with no targetDate are never flagged.
function isBehindSchedule(goal: GoalDto, months: number): boolean {
  if (!goal.targetDate) return false;
  const eta = new Date();
  eta.setDate(1);
  eta.setMonth(eta.getMonth() + months);
  return eta.getTime() > new Date(goal.targetDate).getTime();
}

export function buildHorizonRows(goals: GoalDto[]): { rows: HorizonRow[]; windowMonths: number } {
  const eligible = goals.filter((goal) => goal.goalType !== "spending-cap");
  const withEta = eligible
    .map((goal) => ({ goal, months: monthsToGoal(goal) }))
    .filter((entry) => Number.isFinite(entry.months));

  if (withEta.length === 0) return { rows: [], windowMonths: 0 };

  const furthest = Math.max(...withEta.map((entry) => entry.months));
  const windowMonths = Math.max(6, furthest + 1);

  const rows: HorizonRow[] = withEta
    .map(({ goal, months }) => ({
      goal,
      months,
      pct: goal.targetCents > 0 ? Math.min(100, (goal.currentCents / goal.targetCents) * 100) : 0,
      xPercent: (months / windowMonths) * 100,
      needsAttention: isBehindSchedule(goal, months),
    }))
    .sort((a, b) => a.months - b.months);

  return { rows, windowMonths };
}

function GoalsHorizon({ goals }: { goals: GoalDto[] }) {
  const { rows, windowMonths } = useMemo(() => buildHorizonRows(goals), [goals]);
  if (rows.length === 0) return null;

  const tickCount = 5;
  const ticks = Array.from({ length: tickCount }, (_, i) => {
    const monthsOut = Math.round((i / (tickCount - 1)) * windowMonths);
    const date = new Date();
    date.setDate(1);
    date.setMonth(date.getMonth() + monthsOut);
    return {
      xPercent: (monthsOut / windowMonths) * 100,
      label: date.toLocaleDateString("en-US", { month: "short", year: monthsOut >= 12 ? "2-digit" : undefined }),
    };
  });

  return (
    <section className="section">
      <div className="day-hdr" style={{ marginBottom: 14 }}>
        <div>
          <div className="eyebrow"><span className="dot" />Horizon</div>
          <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>When each goal lands.</h2>
        </div>
      </div>
      <div className="card" style={{ padding: 26 }}>
        <div style={{ position: "relative", height: 20, marginBottom: 8 }}>
          {ticks.map((tick, i) =>
            i === ticks.length - 1 ? (
              <span key={i} className="muted mono" style={{ position: "absolute", right: 0, fontSize: 11, whiteSpace: "nowrap" }}>{tick.label}</span>
            ) : (
              <span key={i} className="muted mono" style={{ position: "absolute", left: `${tick.xPercent}%`, fontSize: 11, whiteSpace: "nowrap" }}>{tick.label}</span>
            )
          )}
        </div>
        <div style={{ position: "relative", paddingTop: 8 }}>
          <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: 2, background: "var(--accent)", boxShadow: "0 0 8px var(--accent)" }} />
          {rows.map((row) => {
            const color = row.needsAttention ? "var(--negative)" : "var(--accent)";
            const labelOnLeft = row.xPercent > 50;
            const labelStyle = labelOnLeft
              ? { position: "absolute" as const, right: `calc(${100 - row.xPercent}% + 14px)`, top: "50%", transform: "translateY(-50%)", fontSize: 13, whiteSpace: "nowrap" as const, textAlign: "right" as const }
              : { position: "absolute" as const, left: `calc(${row.xPercent}% + 14px)`, top: "50%", transform: "translateY(-50%)", fontSize: 13, whiteSpace: "nowrap" as const };
            return (
              <div key={row.goal.id} style={{ position: "relative", height: 44, display: "flex", alignItems: "center" }}>
                <div style={{ position: "absolute", left: 0, top: "50%", width: `${row.xPercent}%`, height: 1, background: "var(--hairline)" }} />
                <div style={{ position: "absolute", left: 0, top: "50%", width: `${(row.xPercent * row.pct) / 100}%`, height: 2, background: color }} />
                <div style={{ position: "absolute", left: `${row.xPercent}%`, top: "50%", transform: "translate(-50%, -50%)", width: 10, height: 10, borderRadius: "50%", border: `2px solid ${color}`, background: "var(--surface)" }} />
                <div style={labelStyle}>
                  {row.goal.name} <span className="muted mono" style={{ fontSize: 12 }}>· {etaLabel(row.months)} · {money(row.goal.targetCents)}</span>
                  {row.needsAttention && <span className="mono" style={{ fontSize: 12, color: "var(--negative)" }}> · Behind schedule</span>}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function WhatIfScenario({ goals }: { goals: GoalDto[] }) {
  const eligibleGoals = useMemo(() => goals.filter((goal) => goal.goalType !== "spending-cap"), [goals]);
  const [scenarioGoalId, setScenarioGoalId] = useState(eligibleGoals[0]?.id ?? "");
  const [extra, setExtra] = useState(0);
  const updateGoalMonthly = useUpdateGoalMonthly();

  useEffect(() => {
    if (!eligibleGoals.some((goal) => goal.id === scenarioGoalId)) {
      setScenarioGoalId(eligibleGoals[0]?.id ?? "");
    }
  }, [eligibleGoals, scenarioGoalId]);

  const selected = eligibleGoals.find((goal) => goal.id === scenarioGoalId);

  if (!selected) return null;

  const extraCents = extra * 100;
  const baseMonths = monthsToGoal(selected);
  const newMonths = monthsToGoal(selected, selected.monthlyCents + extraCents);
  const bothFinite = Number.isFinite(baseMonths) && Number.isFinite(newMonths);
  const monthsSaved = bothFinite ? Math.max(0, baseMonths - newMonths) : 0;
  const newlyAchievable = !Number.isFinite(baseMonths) && Number.isFinite(newMonths);

  const apply = async () => {
    if (extra === 0) return;
    try {
      await updateGoalMonthly.mutateAsync({ id: selected.id, monthlyCents: selected.monthlyCents + extraCents });
      toast.success(`Applied +${money(extraCents)}/mo to ${selected.name}`, {
        description: newlyAchievable
          ? `New ETA: ${etaLabel(newMonths)} · now on a path to finish`
          : `New ETA: ${etaLabel(newMonths)} · saves ${monthsSaved} ${monthsSaved === 1 ? "month" : "months"}`,
      });
      setExtra(0);
    } catch {
      toast.error("Failed to apply scenario");
    }
  };

  return (
    <section className="section">
      <div className="day-hdr" style={{ marginBottom: 14 }}>
        <div>
          <div className="eyebrow"><span className="dot" />What if · scenario</div>
          <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Move a slider, see the future shift.</h2>
        </div>
      </div>

      <div className="card" style={{ padding: 26 }}>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 32 }}>
          <div>
            <div className="eyebrow" style={{ marginBottom: 10 }}>Goal</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }} role="radiogroup" aria-label="Scenario goal">
              {eligibleGoals.map((goal) => (
                <button
                  key={goal.id}
                  type="button"
                  role="radio"
                  aria-checked={scenarioGoalId === goal.id}
                  onClick={() => setScenarioGoalId(goal.id)}
                  className="btn ghost"
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    padding: "10px 12px",
                    background: scenarioGoalId === goal.id ? "var(--surface-2)" : "transparent",
                    border: `1px solid ${scenarioGoalId === goal.id ? "var(--line)" : "transparent"}`,
                    textAlign: "left",
                  }}
                >
                  <div>
                    <div style={{ fontSize: 14, fontWeight: 500 }}>{goal.name}</div>
                    <div className="muted" style={{ fontSize: 12.5, marginTop: 2 }}>ETA {etaLabel(monthsToGoal(goal))}</div>
                  </div>
                </button>
              ))}
            </div>

            <div style={{ marginTop: 22 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                <span className="eyebrow">Extra per month</span>
                <span className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>+{money(extraCents)}</span>
              </div>
              <input
                type="range"
                min={0}
                max={1500}
                step={50}
                value={extra}
                onChange={(e) => setExtra(Number(e.target.value))}
                aria-label="Extra monthly contribution"
                style={{ width: "100%", accentColor: "var(--accent)" }}
              />
              <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
                <span>$0</span>
                <span>$750</span>
                <span>$1,500</span>
              </div>
            </div>
          </div>

          <div style={{ padding: 22, background: "linear-gradient(180deg, var(--accent-2) 0%, var(--surface-2) 60%)", borderRadius: 12, border: "1px solid var(--accent-3)" }}>
            <div className="eyebrow" style={{ marginBottom: 14 }}>Updated horizon</div>
            <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
              <span className="figure" style={{ fontSize: 56, lineHeight: 1, color: "var(--accent)" }}>
                {Number.isFinite(newMonths) ? newMonths : "—"}
              </span>
              <span className="muted" style={{ fontSize: 16 }}>months to go</span>
            </div>
            <div className="muted" style={{ marginTop: 16, fontSize: 14, lineHeight: 1.55 }}>
              {extra === 0 ? (
                <span>You're on track for the original plan. Drag the slider to see what changes.</span>
              ) : newlyAchievable ? (
                <span>
                  Adding <strong>{money(extraCents)}/mo</strong> puts <strong>{selected.name}</strong> on a path to finish by{" "}
                  <strong>{etaLabel(newMonths)}</strong> — it wasn't projected to complete before.
                </span>
              ) : (
                <span>
                  Adding <strong>{money(extraCents)}/mo</strong> brings <strong>{selected.name}</strong> in by{" "}
                  <strong>{monthsSaved} {monthsSaved === 1 ? "month" : "months"}</strong> — moving the ETA from{" "}
                  <strong>{etaLabel(baseMonths)}</strong> to roughly <strong>{etaLabel(newMonths)}</strong>.
                </span>
              )}
            </div>
            <div className="row row-sm" style={{ marginTop: 20 }}>
              <button
                className="btn primary"
                type="button"
                disabled={extra === 0 || updateGoalMonthly.isPending}
                style={{ opacity: extra === 0 ? 0.5 : 1 }}
                onClick={() => void apply()}
              >
                Apply this scenario
              </button>
              <button className="btn ghost" type="button" onClick={() => setExtra(0)}>Reset</button>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function CompoundGrowthProjector({ goals }: { goals: GoalDto[] }) {
  const eligible = useMemo(
    () => goals.filter((g) => g.goalType !== "spending-cap" && g.goalType !== "sinking-fund" && (g.monthlyCents > 0 || g.currentCents > 0)),
    [goals],
  );
  const [goalId, setGoalId] = useState(eligible[0]?.id ?? "");
  useEffect(() => {
    if (!eligible.some((g) => g.id === goalId)) setGoalId(eligible[0]?.id ?? "");
  }, [eligible, goalId]);

  // 10 / 20 / 30-year horizons for the selected goal (Kiyosaki/Hill compounding).
  const p10 = useProjectGoalGrowth(goalId || undefined, 10);
  const p20 = useProjectGoalGrowth(goalId || undefined, 20);
  const p30 = useProjectGoalGrowth(goalId || undefined, 30);

  const selected = eligible.find((g) => g.id === goalId);
  if (!selected) return null;

  const ratePct = Math.round((p10.data?.annualRate ?? 0.07) * 1000) / 10;
  const horizons = [
    { years: 10, value: p10.data?.valueCents },
    { years: 20, value: p20.data?.valueCents },
    { years: 30, value: p30.data?.valueCents },
  ];
  const contributed = (yrs: number) => selected.currentCents + selected.monthlyCents * 12 * yrs;

  return (
    <section className="section">
      <div className="day-hdr" style={{ marginBottom: 14 }}>
        <div>
          <div className="eyebrow"><span className="dot" />Compound growth</div>
          <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>If it kept compounding.</h2>
        </div>
        {eligible.length > 1 && (
          <select className="control" value={goalId} onChange={(e) => setGoalId(e.target.value)} aria-label="Projected goal" style={{ maxWidth: 220 }}>
            {eligible.map((g) => <option key={g.id} value={g.id}>{g.name}</option>)}
          </select>
        )}
      </div>
      <div className="card" style={{ padding: 26 }}>
        <p className="muted" style={{ marginTop: 0 }}>
          Projecting <strong>{selected.name}</strong> — {money(selected.currentCents)} now plus {money(selected.monthlyCents)}/month, compounding at {ratePct}% a year.
        </p>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 14, marginTop: 8 }}>
          {horizons.map((h) => {
            const growth = h.value != null ? h.value - contributed(h.years) : null;
            return (
              <div key={h.years} className="card tight" style={{ padding: 18 }}>
                <div className="eyebrow">In {h.years} years</div>
                <div className="figure money" style={{ fontSize: 28, marginTop: 8 }}>{h.value != null ? money(h.value) : "—"}</div>
                {growth != null && growth > 0 && (
                  <div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>
                    {money(contributed(h.years))} in · <span style={{ color: "var(--positive)" }}>+{money(growth)} growth</span>
                  </div>
                )}
              </div>
            );
          })}
        </div>
        <div className="muted" style={{ fontSize: 12, marginTop: 12 }}>
          Rate comes from the linked account's APY when set, otherwise a 7% long-run assumption. Estimates, not guarantees.
        </div>
      </div>
    </section>
  );
}

function NewGoalForm({ onClose }: { onClose: () => void }) {
  const createGoal = useCreateGoal();
  const { data: totals } = useMonthTotals();
  const { data: accounts = [] } = useAccounts();
  const [name, setName] = useState("");
  const [goalType, setGoalType] = useState<GoalFilter>("save-by-date");
  const [target, setTarget] = useState("");
  const [monthly, setMonthly] = useState("");
  const [targetDate, setTargetDate] = useState("");
  const [purpose, setPurpose] = useState("");
  const [accountId, setAccountId] = useState("");
  const [priority, setPriority] = useState("normal");
  const [deadlineStrictness, setDeadlineStrictness] = useState("target");

  const submit = async () => {
    if (!name.trim() || !target) {
      toast.error("Name and target amount are required");
      return;
    }

    const input: NewGoalInput = {
      name: name.trim(),
      goalType,
      targetCents: Math.round(Number(target) * 100),
      monthlyCents: Math.round(Number(monthly || 0) * 100),
      targetDate: targetDate || null,
      color: "var(--accent)",
      notes: null,
      purpose: purpose.trim() || null,
      accountId: accountId || null,
      priority,
      // A goal with no date is open-ended whatever the picker says, so don't
      // record a commitment the date cannot back up.
      deadlineStrictness: targetDate ? deadlineStrictness : "none",
    };

    try {
      await createGoal.mutateAsync(input);
      toast.success("Goal created");
      onClose();
    } catch {
      toast.error("Failed to create goal");
    }
  };

  useEffect(() => {
    if (goalType === "build-balance" && !target && (totals?.expenseCents ?? 0) > 0) {
      setTarget(String(Math.round(((totals?.expenseCents ?? 0) * 3) / 100)));
    }
  }, [goalType, target, totals?.expenseCents]);

  return (
    <div className="card" style={{ marginTop: 16 }}>
      <div className="h3">New goal</div>
      <div className="form-grid" style={{ marginTop: 18 }}>
        <label className="stack stack-xs"><span className="muted">Name</span><input className="control" value={name} onChange={(e) => setName(e.target.value)} placeholder="Italy fund" /></label>
        <label className="stack stack-xs"><span className="muted">Type</span><select className="control" value={goalType} onChange={(e) => setGoalType(e.target.value as GoalFilter)}>{Object.entries(TYPE_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
        <label className="stack stack-xs"><span className="muted">Target ($)</span><input className="control" type="number" value={target} onChange={(e) => setTarget(e.target.value)} /></label>
        <label className="stack stack-xs"><span className="muted">Monthly contribution ($)</span><input className="control" type="number" value={monthly} onChange={(e) => setMonthly(e.target.value)} /></label>
        <label className="stack stack-xs"><span className="muted">Target date</span><input className="control" type="date" value={targetDate} onChange={(e) => setTargetDate(e.target.value)} /></label>
        <label className="stack stack-xs"><span className="muted">Priority</span><select className="control" value={priority} onChange={(e) => setPriority(e.target.value)}>{Object.entries(PRIORITY_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
        {/* Only meaningful with a date — an open-ended goal has no deadline to
            be strict about, and offering the choice would imply otherwise. */}
        {targetDate && (
          <label className="stack stack-xs"><span className="muted">Is that date firm?</span><select className="control" value={deadlineStrictness} onChange={(e) => setDeadlineStrictness(e.target.value)}>{Object.entries(STRICTNESS_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
        )}
        <label className="stack stack-xs" style={{ gridColumn: "1 / -1" }}><span className="muted">Linked account</span><select className="control" value={accountId} onChange={(e) => setAccountId(e.target.value)}><option value="">None</option>{accounts.map((account) => <option key={account.id} value={account.id}>{getAccountDisplayName(account)}</option>)}</select></label>
        <label className="stack stack-xs" style={{ gridColumn: "1 / -1" }}><span className="muted">Why this goal?</span><textarea className="control" rows={3} value={purpose} onChange={(e) => setPurpose(e.target.value)} /></label>
      </div>
      <div className="row row-sm" style={{ marginTop: 18 }}><button className="btn primary" type="button" onClick={() => void submit()}>Create goal</button><button className="btn ghost" type="button" onClick={onClose}>Cancel</button></div>
    </div>
  );
}

export default function Goals() {
  const { data: goals = [], isLoading, error } = useGoals();
  const { data: accounts = [] } = useAccounts();
  const [searchParams, setSearchParams] = useSearchParams();
  const [filter, setFilter] = useState<GoalFilter>("all");
  const [creating, setCreating] = useState(false);
  const [editingGoal, setEditingGoal] = useState<GoalDto | null>(null);
  const [pausedPrevious, setPausedPrevious] = useState<Record<string, number>>({});
  const [explainKey, setExplainKey] = useState<string | null>(null);
  const { data: goalExplanations, isLoading: goalExplaining } = useGoalExplanations();
  const updateGoalMonthly = useUpdateGoalMonthly();

  const accountNameById = useMemo(() => new Map(accounts.map((account) => [account.id, getAccountDisplayName(account)])), [accounts]);

  const handleTogglePause = async (goal: GoalDto) => {
    try {
      if (goal.monthlyCents > 0) {
        setPausedPrevious((prev) => ({ ...prev, [goal.id]: goal.monthlyCents }));
        await updateGoalMonthly.mutateAsync({ id: goal.id, monthlyCents: 0 });
        toast.success(`Paused ${goal.name}`, { description: "Monthly auto-contribution set to $0. Resume anytime." });
      } else {
        const restore = pausedPrevious[goal.id];
        if (restore === undefined) {
          toast("No previous amount to restore — use Adjust to set a new monthly contribution.");
          return;
        }
        await updateGoalMonthly.mutateAsync({ id: goal.id, monthlyCents: restore });
        setPausedPrevious((prev) => { const next = { ...prev }; delete next[goal.id]; return next; });
        toast.success(`Resumed ${goal.name} at ${money(restore)}/month`);
      }
    } catch {
      toast.error("Could not update this goal");
    }
  };

  const counts = useMemo(() => goals.reduce<Record<string, number>>((acc, goal) => {
    acc[goal.goalType] = (acc[goal.goalType] ?? 0) + 1;
    return acc;
  }, {}), [goals]);

  const visible = filter === "all" ? goals : goals.filter((goal) => goal.goalType === filter);
  const focusedGoal = useMemo(() => {
    const focus = searchParams.get("focusGoal");
    if (!focus) return null;
    return goals.find((goal) => goal.id === focus || goal.name.toLowerCase() === focus.toLowerCase()) ?? null;
  }, [goals, searchParams]);
  const activeEditingGoal = editingGoal ?? focusedGoal;

  useEffect(() => {
    if (!focusedGoal || editingGoal) return;
    setEditingGoal(focusedGoal);
    const next = new URLSearchParams(searchParams);
    next.delete("focusGoal");
    setSearchParams(next, { replace: true });
  }, [editingGoal, focusedGoal, searchParams, setSearchParams]);

  if (isLoading) return <div className="stub">Loading goals…</div>;
  if (error) return <div className="stub" role="alert">Error loading goals.</div>;

  return (
    <div className="screen screen-goals">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Goals · {goals.length} active</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Things you're moving toward.</h1>
        </div>
        <button className="btn primary" type="button" onClick={() => setCreating((open) => !open)}>+ New goal</button>
      </div>

      <p className="muted" style={{ maxWidth: 720, marginTop: 0 }}>A goal is a horizon line on your future runway. The agent keeps it visible so everyday choices still point toward something larger.</p>

      <div className="toolbar" style={{ marginTop: 8 }}>
        <button className={filter === "all" ? "on" : ""} type="button" onClick={() => setFilter("all")}>All {goals.length}</button>
        <button className={filter === "save-by-date" ? "on" : ""} type="button" onClick={() => setFilter("save-by-date")}>Save by date {counts["save-by-date"] ?? 0}</button>
        <button className={filter === "build-balance" ? "on" : ""} type="button" onClick={() => setFilter("build-balance")}>Build balance {counts["build-balance"] ?? 0}</button>
        <button className={filter === "debt-payoff" ? "on" : ""} type="button" onClick={() => setFilter("debt-payoff")}>Debt payoff {counts["debt-payoff"] ?? 0}</button>
        <button className={filter === "spending-cap" ? "on" : ""} type="button" onClick={() => setFilter("spending-cap")}>Spending cap {counts["spending-cap"] ?? 0}</button>
        <button className={filter === "sinking-fund" ? "on" : ""} type="button" onClick={() => setFilter("sinking-fund")}>Sinking fund {counts["sinking-fund"] ?? 0}</button>
      </div>

      {creating && <NewGoalForm onClose={() => setCreating(false)} />}
      <GoalDrawer open={activeEditingGoal !== null} onClose={() => setEditingGoal(null)} goal={activeEditingGoal} />

      <div className="section" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {visible.length === 0 ? (
          <EmptyState
            compact
            title={goals.length === 0 ? "No goals yet" : "No goals in this filter"}
            description={
              goals.length === 0
                ? "Create a goal — an emergency fund, a trip, a debt payoff — and the agent keeps it on your radar as you spend."
                : "Nothing matches this filter yet. Try another, or create a new goal."
            }
            actions={<button className="btn primary" type="button" onClick={() => setCreating(true)}>+ New goal</button>}
          />
        ) : (
          visible.map((goal) => (
            <GoalCard
              key={goal.id}
              goal={goal}
              onEdit={setEditingGoal}
              onExplain={(g) => setExplainKey(`goal:${g.id}`)}
              linkedAccountName={goal.accountId ? accountNameById.get(goal.accountId) ?? null : null}
              onTogglePause={(g) => void handleTogglePause(g)}
              pausePending={updateGoalMonthly.isPending}
              pausedByUser={goal.id in pausedPrevious}
            />
          ))
        )}
      </div>

      <ExplainDrawer
        explanation={explainKey ? goalExplanations?.[explainKey] : undefined}
        isLoading={goalExplaining}
        open={explainKey !== null}
        onClose={() => setExplainKey(null)}
      />

      <GoalsHorizon goals={goals} />

      {goals.length > 0 && <WhatIfScenario goals={goals} />}

      {goals.some((g) => g.goalType !== "spending-cap" && g.goalType !== "sinking-fund" && (g.monthlyCents > 0 || g.currentCents > 0)) && <CompoundGrowthProjector goals={goals} />}
    </div>
  );
}

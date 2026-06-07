import { useState } from "react";
import { toast } from "sonner";
import { useGoals, useCreateGoal, useUpdateGoalBalance, useArchiveGoal, useUpdateGoalMonthly } from "../api/hooks/budget";
import type { GoalDto, NewGoalInput } from "../api/client";
import * as I from "../components/Icons";
import { money } from "../utils/format";

/** Estimate months to reach target at current monthly pace. */
function monthsTo(goal: GoalDto): number | null {
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return 0;
  if (goal.monthlyCents <= 0) return null;
  return Math.ceil(remaining / goal.monthlyCents);
}

/** Rough "ETA" string from a months count. */
function etaLabel(months: number): string {
  const d = new Date();
  d.setMonth(d.getMonth() + months);
  return d.toLocaleString("default", { month: "short", year: "numeric" });
}

type PaceStatus = "ahead" | "on_track" | "needs_attention";

function paceStatus(goal: GoalDto): PaceStatus | null {
  if (!goal.targetDate || goal.targetCents === 0) return null;
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return null; // already reached
  if (goal.monthlyCents <= 0) return "needs_attention";
  const monthsRemaining = Math.ceil(remaining / goal.monthlyCents);
  const target = new Date(goal.targetDate);
  const now = new Date();
  const monthsExpected =
    (target.getFullYear() - now.getFullYear()) * 12 +
    (target.getMonth() - now.getMonth());
  if (monthsExpected <= 0) return "needs_attention";
  if (monthsRemaining < monthsExpected * 0.85) return "ahead";
  if (monthsRemaining > monthsExpected * 1.15) return "needs_attention";
  return "on_track";
}

const PACE_LABELS: Record<PaceStatus, { label: string; cls: string }> = {
  ahead: { label: "Ahead", cls: "positive" },
  on_track: { label: "On track", cls: "" },
  needs_attention: { label: "Needs attention", cls: "warning" },
};

type GoalType = "all" | "save-by-date" | "build-balance" | "debt-payoff" | "spending-cap";

const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Debt payoff",
  "spending-cap": "Spending cap",
};

const GOAL_COLORS = ["#C9F950", "#34D399", "#60A5FA", "#A78BFA", "#FB923C", "#F472B6", "#2DD4BF"];

function daysUntil(dateStr: string): number {
  return (new Date(dateStr).getTime() - Date.now()) / 86400000;
}

function GoalCard({ goal }: { goal: GoalDto }) {
  const updateBalance = useUpdateGoalBalance();
  const archiveGoal = useArchiveGoal();
  const [editingBalance, setEditingBalance] = useState(false);
  const [balanceVal, setBalanceVal] = useState(String(Math.round(goal.currentCents / 100)));
  const [confirmArchive, setConfirmArchive] = useState(false);

  const pct = goal.targetCents > 0 ? Math.min(100, (goal.currentCents / goal.targetCents) * 100) : 0;
  const months = monthsTo(goal);
  const color = goal.color || "var(--accent)";

  const saveBalance = async () => {
    const cents = Math.round(parseFloat(balanceVal || "0") * 100);
    try {
      await updateBalance.mutateAsync({ id: goal.id, currentCents: cents });
      toast.success("Balance updated");
      setEditingBalance(false);
    } catch {
      toast.error("Failed to update balance");
    }
  };

  const handleArchive = async () => {
    if (!confirmArchive) { setConfirmArchive(true); return; }
    try {
      await archiveGoal.mutateAsync(goal.id);
      toast.success("Goal archived");
    } catch {
      setConfirmArchive(false);
      toast.error("Failed to archive goal");
    }
  };

  return (
    <div className="card" style={{ padding: 22, borderLeft: `3px solid ${color}` }}>
      <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", marginBottom: 14 }}>
        <div>
          <div style={{ fontSize: 15.5, fontWeight: 600, marginBottom: 3 }}>{goal.name}</div>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginTop: 4 }}>
            <span className="chip" style={{ fontSize: 11 }}>{TYPE_LABELS[goal.goalType] || goal.goalType}</span>
            {(() => {
              const pace = paceStatus(goal);
              if (!pace) return null;
              const { label, cls } = PACE_LABELS[pace];
              return <span className={`chip ${cls}`} style={{ fontSize: 11 }}>{label}</span>;
            })()}
          </div>
        </div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          {confirmArchive ? (
            <>
              <button className="btn sm" style={{ background: "var(--negative)", borderColor: "var(--negative)", color: "#fff" }} onClick={() => void handleArchive()}>Confirm</button>
              <button className="btn sm ghost" onClick={() => setConfirmArchive(false)}>Cancel</button>
            </>
          ) : (
            <button className="btn sm ghost" onClick={() => void handleArchive()} title="Archive goal">
              <I.Trash />
            </button>
          )}
        </div>
      </div>

      {/* Progress bar */}
      <div style={{ marginBottom: 14 }}>
        <div className="goal-bar" style={{ height: 8 }}>
          <span style={{ width: pct + "%", background: color, boxShadow: `0 0 10px ${color}55` }} />
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8, fontSize: 12.5 }}>
          <div>
            {editingBalance ? (
              <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span className="muted" style={{ fontSize: 13 }}>$</span>
                <input
                  type="number"
                  min="0"
                  value={balanceVal}
                  onChange={(e) => setBalanceVal(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") void saveBalance(); if (e.key === "Escape") setEditingBalance(false); }}
                  autoFocus
                  style={{ width: 80, background: "var(--surface-2)", border: "1px solid var(--accent)", borderRadius: 5, padding: "2px 6px", fontSize: 13, color: "var(--ink)", outline: "none" }}
                />
                <button className="btn sm primary" onClick={() => void saveBalance()} style={{ padding: "3px 9px" }}>Save</button>
                <button className="btn sm ghost" onClick={() => setEditingBalance(false)} style={{ padding: "3px 7px" }}>✕</button>
              </span>
            ) : (
              <button
                onClick={() => setEditingBalance(true)}
                className="btn ghost sm"
                style={{ padding: "2px 6px", fontSize: 12.5 }}
              >
                <span className="num money">{money(goal.currentCents)}</span>
                <I.Pencil width="11" height="11" style={{ marginLeft: 4 }} />
              </button>
            )}
          </div>
          <span className="muted">of {money(goal.targetCents)}</span>
        </div>
      </div>

      {/* Stats */}
      <div style={{ display: "flex", gap: 18, fontSize: 12.5 }}>
        <div>
          <div className="muted" style={{ fontSize: 11, textTransform: "uppercase", letterSpacing: "0.06em", marginBottom: 2 }}>Monthly</div>
          <div className="num">{money(goal.monthlyCents)}</div>
        </div>
        {months !== null && months > 0 && (
          <div>
            <div className="muted" style={{ fontSize: 11, textTransform: "uppercase", letterSpacing: "0.06em", marginBottom: 2 }}>ETA</div>
            <div className="num">{etaLabel(months)}</div>
          </div>
        )}
        {pct >= 100 && (
          <div>
            <span className="chip positive" style={{ fontSize: 11 }}>🎉 Reached!</span>
          </div>
        )}
        {goal.targetDate && (
          <div>
            <div className="muted" style={{ fontSize: 11, textTransform: "uppercase", letterSpacing: "0.06em", marginBottom: 2 }}>Target date</div>
            <div style={{ fontSize: 12.5 }}>{new Date(goal.targetDate).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</div>
          </div>
        )}
      </div>
    </div>
  );
}

function NewGoalForm({ onClose }: { onClose: () => void }) {
  const createGoal = useCreateGoal();
  const [name, setName] = useState("");
  const [type, setType] = useState<string>("save-by-date");
  const [target, setTarget] = useState("");
  const [monthly, setMonthly] = useState("");
  const [targetDate, setTargetDate] = useState("");
  const [colorIdx, setColorIdx] = useState(0);

  const submit = async () => {
    if (!name.trim() || !target) { toast.error("Name and target amount are required"); return; }
    const input: NewGoalInput = {
      name: name.trim(),
      goalType: type,
      targetCents: Math.round(parseFloat(target) * 100),
      monthlyCents: Math.round(parseFloat(monthly || "0") * 100),
      targetDate: targetDate || null,
      color: GOAL_COLORS[colorIdx] ?? "#C9F950",
      notes: null,
    };
    try {
      await createGoal.mutateAsync(input);
      toast.success("Goal created");
      onClose();
    } catch {
      toast.error("Failed to create goal");
    }
  };

  return (
    <div className="card" style={{ padding: 28, marginBottom: 24 }}>
      <div className="h3" style={{ marginBottom: 20 }}>New goal</div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>NAME</label>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Italy trip, Emergency fund…"
            style={{ width: "100%", background: "var(--surface-2)", border: "1px solid var(--line-2)", borderRadius: 7, padding: "8px 12px", fontSize: 14, color: "var(--ink)", outline: "none" }}
          />
        </div>
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>TYPE</label>
          <select
            value={type}
            onChange={(e) => setType(e.target.value)}
            style={{ width: "100%", background: "var(--surface-2)", border: "1px solid var(--line-2)", borderRadius: 7, padding: "8px 12px", fontSize: 14, color: "var(--ink)", outline: "none" }}
          >
            {Object.entries(TYPE_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
          </select>
        </div>
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>TARGET ($)</label>
          <input type="number" min="0" value={target} onChange={(e) => setTarget(e.target.value)} placeholder="5000" style={{ width: "100%", background: "var(--surface-2)", border: "1px solid var(--line-2)", borderRadius: 7, padding: "8px 12px", fontSize: 14, color: "var(--ink)", outline: "none" }} />
        </div>
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>MONTHLY CONTRIBUTION ($)</label>
          <input type="number" min="0" value={monthly} onChange={(e) => setMonthly(e.target.value)} placeholder="500" style={{ width: "100%", background: "var(--surface-2)", border: "1px solid var(--line-2)", borderRadius: 7, padding: "8px 12px", fontSize: 14, color: "var(--ink)", outline: "none" }} />
        </div>
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>TARGET DATE (optional)</label>
          <input type="date" value={targetDate} onChange={(e) => setTargetDate(e.target.value)} style={{ width: "100%", background: "var(--surface-2)", border: "1px solid var(--line-2)", borderRadius: 7, padding: "8px 12px", fontSize: 14, color: "var(--ink)", outline: "none" }} />
        </div>
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>COLOR</label>
          <div style={{ display: "flex", gap: 8 }}>
            {GOAL_COLORS.map((c, i) => (
              <button
                key={c}
                onClick={() => setColorIdx(i)}
                style={{ width: 24, height: 24, borderRadius: 999, background: c, border: colorIdx === i ? "2px solid var(--ink)" : "2px solid transparent", cursor: "pointer" }}
              />
            ))}
          </div>
        </div>
      </div>
      <div style={{ display: "flex", gap: 8, marginTop: 20 }}>
        <button className="btn primary" onClick={() => void submit()}>Create goal</button>
        <button className="btn ghost" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}

export default function Goals() {
  const { data: goals = [], isLoading, error } = useGoals();
  const [typeFilter, setTypeFilter] = useState<GoalType>("all");
  const [showNew, setShowNew] = useState(false);

  const typeCounts = goals.reduce<Record<string, number>>((m, g) => {
    m[g.goalType] = (m[g.goalType] || 0) + 1;
    return m;
  }, {});

  const visible = typeFilter === "all" ? goals : goals.filter((g) => g.goalType === typeFilter);

  const updateMonthly = useUpdateGoalMonthly();

  const sinkingFunds = goals.filter(
    (g) =>
      g.goalType === "save-by-date" &&
      g.targetDate != null &&
      daysUntil(g.targetDate) > 0 &&
      daysUntil(g.targetDate) <= 365
  );

  // What-if scenario
  const [scenarioId, setScenarioId] = useState<string | null>(null);
  const [extra, setExtra] = useState(0);
  const scenarioGoal = goals.find((g) => g.id === scenarioId) ?? goals[0];
  const baseMonths = scenarioGoal ? monthsTo(scenarioGoal) : null;
  const newMonths = scenarioGoal && scenarioGoal.monthlyCents + extra * 100 > 0
    ? Math.ceil((scenarioGoal.targetCents - scenarioGoal.currentCents) / (scenarioGoal.monthlyCents + extra * 100))
    : baseMonths;
  const monthsSaved = baseMonths !== null && newMonths !== null ? Math.max(0, baseMonths - newMonths) : 0;

  const handleApply = async () => {
    if (!scenarioGoal || extra === 0 || newMonths === null) return;
    const newMonthly = scenarioGoal.monthlyCents + extra * 100;
    try {
      await updateMonthly.mutateAsync({ id: scenarioGoal.id, monthlyCents: newMonthly });
      toast.success(`Applied +${money(extra * 100)}/mo to ${scenarioGoal.name}`, {
        description: newMonths > 0 ? `ETA now ${etaLabel(newMonths)}` : "Goal reached this month!",
      });
      setExtra(0);
    } catch {
      toast.error("Failed to apply change");
    }
  };

  if (isLoading) return <div className="stub">Loading goals…</div>;
  if (error)     return <div className="stub">Error loading goals.</div>;

  return (
    <div className="screen">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Goals · {goals.length} active
          </div>
          <h1>Things you're moving toward.</h1>
        </div>
        <button className="btn" onClick={() => setShowNew(true)}>
          <I.Plus /> New goal
        </button>
      </div>

      <p className="muted" style={{ maxWidth: 660, fontSize: 14, lineHeight: 1.6, marginTop: -12, marginBottom: 24 }}>
        A goal is a horizon line on your future runway. Set a target, commit a monthly amount, and watch the ETA shift as your balance grows.
      </p>

      {showNew && <NewGoalForm onClose={() => setShowNew(false)} />}

      {goals.length === 0 && !showNew ? (
        <div className="card" style={{ textAlign: "center", padding: "64px 32px" }}>
          <I.Goal style={{ color: "var(--ink-faint)", width: 32, height: 32, margin: "0 auto 16px" }} />
          <div style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>No goals yet</div>
          <div className="muted" style={{ fontSize: 14, marginBottom: 24 }}>Create your first goal to track savings, debt payoff, or spending caps.</div>
          <button className="btn primary" onClick={() => setShowNew(true)}><I.Plus /> Create a goal</button>
        </div>
      ) : (
        <>
          {/* Type tabs */}
          {goals.length > 0 && (
            <div className="toolbar" style={{ marginBottom: 20, display: "inline-flex" }}>
              <button className={typeFilter === "all" ? "on" : ""} onClick={() => setTypeFilter("all")}>
                All <span style={{ color: "var(--ink-faint)", marginLeft: 4, fontSize: 11 }}>{goals.length}</span>
              </button>
              {Object.entries(TYPE_LABELS).map(([k, v]) => typeCounts[k] ? (
                <button key={k} className={typeFilter === k ? "on" : ""} onClick={() => setTypeFilter(k as GoalType)}>
                  {v} <span style={{ color: "var(--ink-faint)", marginLeft: 4, fontSize: 11 }}>{typeCounts[k]}</span>
                </button>
              ) : null)}
            </div>
          )}

          {/* Goal cards */}
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {visible.map((g) => <GoalCard key={g.id} goal={g} />)}
            {visible.length === 0 && (
              <div className="card tight" style={{ textAlign: "center", padding: "32px 24px", color: "var(--ink-mute)", fontSize: 14 }}>
                No goals in this category.
              </div>
            )}
          </div>

          {/* Sinking funds */}
          {sinkingFunds.length > 0 && (
            <div className="section" style={{ marginTop: 28 }}>
              <div className="eyebrow" style={{ marginBottom: 12 }}>
                <span className="dot" /><span>Sinking funds</span> · due within a year
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
                {sinkingFunds.map((g) => {
                  const pct = g.targetCents > 0
                    ? Math.min(100, (g.currentCents / g.targetCents) * 100)
                    : 0;
                  const color = g.color || "var(--accent)";
                  return (
                    <div key={g.id} className="card" style={{ padding: 16, borderLeft: `3px solid ${color}` }}>
                      <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 4 }}>{g.name}</div>
                      {g.targetDate && (
                        <span className="chip" style={{ fontSize: 11, marginBottom: 8, display: "inline-block" }}>
                          {new Date(g.targetDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}
                        </span>
                      )}
                      <div className="goal-bar" style={{ height: 6, marginBottom: 8 }}>
                        <span style={{ width: pct + "%", background: color }} />
                      </div>
                      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12.5 }}>
                        <span className="muted">{Math.round(pct)}%</span>
                        <span className="num">{money(g.targetCents - g.currentCents)} left</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* What-if scenario */}
          {goals.length > 0 && scenarioGoal && (
            <div className="section">
              <div className="screen-header" style={{ marginBottom: 16 }}>
                <div className="screen-header-text">
                  <div className="screen-eyebrow"><span className="dot" />What if · scenario</div>
                  <h1 style={{ fontSize: 22 }}>Move a slider, see the future shift.</h1>
                </div>
              </div>
              <div className="card" style={{ padding: 26 }}>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 32 }}>
                  <div>
                    <div className="eyebrow" style={{ marginBottom: 10 }}>Goal</div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                      {goals.map((g) => (
                        <button
                          key={g.id}
                          onClick={() => { setScenarioId(g.id); setExtra(0); }}
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "center",
                            padding: "10px 12px",
                            borderRadius: 8,
                            background: scenarioGoal.id === g.id ? "var(--surface-2)" : "transparent",
                            border: `1px solid ${scenarioGoal.id === g.id ? "var(--line-2)" : "transparent"}`,
                            cursor: "pointer",
                            textAlign: "left",
                          }}
                        >
                          <div>
                            <div style={{ fontSize: 14, fontWeight: 500 }}>{g.name}</div>
                            <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>
                              {money(g.monthlyCents)}/mo · {monthsTo(g) !== null ? `ETA ${etaLabel(monthsTo(g)!)}` : "no pace set"}
                            </div>
                          </div>
                          {scenarioGoal.id === g.id && <I.Check style={{ color: "var(--accent)" }} />}
                        </button>
                      ))}
                    </div>

                    <div style={{ marginTop: 22 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                        <span className="eyebrow">Extra per month</span>
                        <span className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>+{money(extra * 100)}</span>
                      </div>
                      <input
                        type="range"
                        min="0"
                        max="1500"
                        step="50"
                        value={extra}
                        onChange={(e) => setExtra(parseInt(e.target.value))}
                        style={{ width: "100%", accentColor: "var(--accent)" }}
                      />
                      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 11.5, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
                        <span>$0</span><span>$750</span><span>$1,500</span>
                      </div>
                    </div>
                  </div>

                  <div style={{ padding: 22, background: "linear-gradient(180deg, var(--accent-2) 0%, var(--surface-2) 60%)", borderRadius: 12, border: "1px solid var(--accent-3)" }}>
                    <div className="eyebrow" style={{ marginBottom: 14 }}>Updated horizon</div>
                    {newMonths !== null ? (
                      <>
                        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
                          <span className="figure" style={{ fontSize: 56, lineHeight: 1, color: "var(--accent)" }}>{Math.max(0, newMonths)}</span>
                          <span className="muted" style={{ fontSize: 16 }}>months to go</span>
                        </div>
                        <div style={{ marginTop: 16, fontSize: 14, lineHeight: 1.55, color: "var(--ink-2)" }}>
                          {extra === 0
                            ? "Drag the slider to see what happens when you contribute more each month."
                            : `Adding ${money(extra * 100)}/mo brings ${scenarioGoal.name} in by ${monthsSaved} month${monthsSaved !== 1 ? "s" : ""} — ETA ${newMonths > 0 ? etaLabel(newMonths) : "this month"}.`
                          }
                        </div>
                      </>
                    ) : (
                      <div className="muted" style={{ fontSize: 14 }}>Set a monthly contribution on this goal to see the timeline.</div>
                    )}
                    <div style={{ marginTop: 20, display: "flex", gap: 8 }}>
                      <button className="btn ghost sm" onClick={() => setExtra(0)} style={{ opacity: extra === 0 ? 0.4 : 1 }}>
                        Reset
                      </button>
                      {extra > 0 && newMonths !== null && (
                        <button
                          className="btn primary sm"
                          disabled={updateMonthly.isPending}
                          onClick={() => void handleApply()}
                        >
                          Apply +{money(extra * 100)}/mo →
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

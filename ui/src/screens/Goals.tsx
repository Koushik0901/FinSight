import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useMonthTotals } from "../api/hooks";
import { useAccounts } from "../api/hooks/accounts";
import { useLiabilities } from "../api/hooks/assets";
import { useGoals, useCreateGoal, useUpdateGoalBalance, useArchiveGoal, useUpdateGoalMonthly, useUpdateGoalPurpose, useProjectGoalGrowth } from "../api/hooks/budget";
import type { GoalDto, NewGoalInput } from "../api/client";
import * as I from "../components/Icons";
import { money } from "../utils/format";
import { CopilotNudge } from "../components/CopilotNudge";
import Card from "../components/Card";
import ProgressBar from "../components/ProgressBar";
import Badge from "../components/Badge";
import Button from "../components/Button";
import Input from "../components/Input";
import Select from "../components/Select";
import TextArea from "../components/TextArea";
import Swatch from "../components/Swatch";
import EmptyState from "../components/EmptyState";

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

const PACE_LABELS: Record<PaceStatus, { label: string; tone: "positive" | "default" | "warning" }> = {
  ahead: { label: "Ahead", tone: "positive" },
  on_track: { label: "On track", tone: "default" },
  needs_attention: { label: "Needs attention", tone: "warning" },
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
  const updatePurpose = useUpdateGoalPurpose();
  const [editingBalance, setEditingBalance] = useState(false);
  const [balanceVal, setBalanceVal] = useState(String(Math.round(goal.currentCents / 100)));
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [showProjection, setShowProjection] = useState(false);
  const [editingPurpose, setEditingPurpose] = useState(false);
  const [purposeVal, setPurposeVal] = useState(goal.purpose ?? "");
  const { data: proj10 } = useProjectGoalGrowth(goal.id, 10);
  const { data: proj20 } = useProjectGoalGrowth(goal.id, 20);
  const { data: proj30 } = useProjectGoalGrowth(goal.id, 30);
  const { data: liabilities = [] } = useLiabilities();
  const { data: accounts = [] } = useAccounts();
  const linkedLiability = liabilities.find((l) => l.id === goal.liabilityId);
  const linkedAccount = accounts.find((a) => a.id === goal.accountId);

  const pct = goal.targetCents > 0 ? Math.min(100, (goal.currentCents / goal.targetCents) * 100) : 0;
  const months = monthsTo(goal);
  const color = goal.color || "var(--accent)";
  const isLinkedToLiability = !!goal.liabilityId;

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

  const savePurpose = async () => {
    const trimmed = purposeVal.trim();
    try {
      await updatePurpose.mutateAsync({ id: goal.id, purpose: trimmed || null });
      toast.success(trimmed ? "Why updated" : "Why cleared");
      setEditingPurpose(false);
    } catch {
      toast.error("Failed to save why");
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
    <Card style={{ borderLeft: `3px solid ${color}` }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", marginBottom: 14 }}>
        <div>
          <div className="strong" style={{ fontSize: 15.5, marginBottom: 3 }}>{goal.name}</div>
          <div className="row row-sm wrap" style={{ marginTop: 4 }}>
            <Badge>{TYPE_LABELS[goal.goalType] || goal.goalType}</Badge>
            {(() => {
              const pace = paceStatus(goal);
              if (!pace) return null;
              const { label, tone } = PACE_LABELS[pace];
              return <Badge tone={tone}>{label}</Badge>;
            })()}
          </div>
        </div>
        <div className="row row-sm" style={{ alignItems: "center" }}>
          {confirmArchive ? (
            <>
              <Button variant="danger" size="sm" onClick={() => void handleArchive()}>Confirm</Button>
              <Button variant="ghost" size="sm" onClick={() => setConfirmArchive(false)}>Cancel</Button>
            </>
          ) : (
            <Button variant="ghost" size="sm" onClick={() => void handleArchive()} title="Archive goal">
              <I.Trash />
            </Button>
          )}
        </div>
      </div>

      {/* Progress bar */}
      <div style={{ marginBottom: 14 }}>
        <div style={{ "--accent": color } as React.CSSProperties}>
          <ProgressBar value={goal.currentCents} max={goal.targetCents || 1} aria-label={`${goal.name} progress`} />
        </div>
        <div className="row" style={{ justifyContent: "space-between", marginTop: 8, fontSize: 12.5 }}>
          <div>
            {isLinkedToLiability ? (
              <span className="num money">{money(goal.currentCents)}</span>
            ) : editingBalance ? (
              <span className="row row-sm" style={{ alignItems: "center" }}>
                <span className="muted" style={{ fontSize: 13 }}>$</span>
                <input
                  type="number"
                  min="0"
                  value={balanceVal}
                  onChange={(e) => setBalanceVal(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") void saveBalance(); if (e.key === "Escape") setEditingBalance(false); }}
                  autoFocus
                  className="control"
                  style={{ width: 80 }}
                />
                <Button variant="primary" size="sm" onClick={() => void saveBalance()}>Save</Button>
                <Button variant="ghost" size="sm" onClick={() => setEditingBalance(false)}>✕</Button>
              </span>
            ) : (
              <Button
                onClick={() => setEditingBalance(true)}
                variant="ghost"
                size="sm"
                style={{ padding: "2px 6px", fontSize: 12.5 }}
              >
                <span className="num money">{money(goal.currentCents)}</span>
                <I.Pencil width="11" height="11" style={{ marginLeft: 4 }} />
              </Button>
            )}
          </div>
          <span className="muted money">of {money(goal.targetCents)}</span>
        </div>
        {linkedLiability && (
          <div className="row row-sm" style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 8 }}>
            Linked to {linkedLiability.name} · {linkedLiability.aprPct ?? "—"}% APR · updates automatically
          </div>
        )}
        {linkedAccount && (
          <div className="row row-sm" style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 8 }}>
            Linked to {linkedAccount.name} · {(proj10?.annualRate ?? 0.07) * 100}% APY
          </div>
        )}
      </div>

      {/* Stats */}
      <div className="row row-lg" style={{ fontSize: 12.5 }}>
        <div>
          <div className="eyebrow" style={{ marginBottom: 2 }}>Monthly</div>
          <div className="num money">{money(goal.monthlyCents)}</div>
        </div>
        {months !== null && months > 0 && (
          <div>
            <div className="eyebrow" style={{ marginBottom: 2 }}>ETA</div>
            <div className="num">{etaLabel(months)}</div>
          </div>
        )}
        {pct >= 100 && (
          <div>
            <Badge tone="positive">🎉 Reached!</Badge>
          </div>
        )}
        {goal.targetDate && (
          <div>
            <div className="eyebrow" style={{ marginBottom: 2 }}>Target date</div>
            <div style={{ fontSize: 12.5 }}>{new Date(goal.targetDate).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</div>
          </div>
        )}
      </div>

      {/* Purpose / Why */}
      <div style={{ marginTop: 14, borderTop: "1px solid var(--line)", paddingTop: 12 }}>
        {editingPurpose ? (
          <div>
            <TextArea
              label="Why this goal?"
              value={purposeVal}
              onChange={(e) => setPurposeVal(e.target.value)}
              placeholder="What will this mean for you? Describe your motivation…"
              rows={3}
            />
            <div className="row row-sm" style={{ marginTop: 6 }}>
              <Button variant="primary" size="sm" onClick={() => void savePurpose()}>Save</Button>
              <Button variant="ghost" size="sm" onClick={() => { setPurposeVal(goal.purpose ?? ""); setEditingPurpose(false); }}>Cancel</Button>
            </div>
          </div>
        ) : goal.purpose ? (
          <div className="row row-md" style={{ alignItems: "flex-start" }}>
            <span style={{ fontSize: 13 }}>💡</span>
            <div className="grow">
              <span style={{ fontSize: 12.5, color: "var(--ink-mute)", fontStyle: "italic" }}>{goal.purpose}</span>
            </div>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setEditingPurpose(true)}
              style={{ padding: "2px 6px", flexShrink: 0 }}
              title="Edit why"
            >
              <I.Pencil width="11" height="11" />
            </Button>
          </div>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setEditingPurpose(true)}
            style={{ padding: 0, border: "none", background: "transparent", color: "var(--ink-faint)", fontSize: 12.5 }}
          >
            + Add your why
          </Button>
        )}
      </div>

      {goal.monthlyCents > 0 && (
        <div style={{ marginTop: 16 }}>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowProjection((open) => !open)}
            style={{ padding: 0, border: "none", background: "transparent", color: "var(--accent)" }}
          >
            {showProjection ? "Hide projection" : "See projection →"}
          </Button>
          {showProjection && (
            <Card tone="accent" style={{ marginTop: 12 }}>
              <div className="eyebrow" style={{ marginBottom: 8 }}>Compound Growth</div>
              <div style={{ fontSize: 13.5, marginBottom: 10 }}>
                If you invest <span className="money">{money(goal.monthlyCents)}</span>/month for the long run at {(proj10?.annualRate ?? 0.07) * 100}% APY:
              </div>
              <div className="stack stack-sm">
                {[proj10, proj20, proj30].map((p) =>
                  p ? (
                    <div key={p.years} className="row" style={{ justifyContent: "space-between", gap: 12, fontSize: 13.5 }}>
                      <span className="muted">{p.years} years</span>
                      <span className="money" style={{ fontWeight: 600 }}>{money(p.valueCents)}</span>
                    </div>
                  ) : null
                )}
              </div>
            </Card>
          )}
        </div>
      )}
    </Card>
  );
}

function NewGoalForm({ onClose }: { onClose: () => void }) {
  const createGoal = useCreateGoal();
  const { data: totals } = useMonthTotals();
  const { data: liabilities = [] } = useLiabilities();
  const { data: accounts = [] } = useAccounts();
  const [name, setName] = useState("");
  const [type, setType] = useState<string>("save-by-date");
  const [target, setTarget] = useState("");
  const [monthly, setMonthly] = useState("");
  const [targetDate, setTargetDate] = useState("");
  const [colorIdx, setColorIdx] = useState(0);
  const [purpose, setPurpose] = useState("");
  const [liabilityId, setLiabilityId] = useState("");
  const [accountId, setAccountId] = useState("");
  const emergencyBaseCents = totals?.expenseCents ?? 0;
  const showEmergencyQuickFill = type === "build-balance" || name.toLowerCase().includes("emergency");
  const linkableLiabilities = liabilities.filter((l) => l.balanceCents > 0);
  const savingsAccounts = accounts.filter((a) => a.type === "Savings");
  const selectedLiability = liabilities.find((l) => l.id === liabilityId);

  useEffect(() => {
    if (selectedLiability?.originalBalanceCents && !target) {
      setTarget(String(selectedLiability.originalBalanceCents / 100));
    }
  }, [liabilityId, selectedLiability, target]);

  const handleLiabilityChange = (id: string) => {
    setLiabilityId(id);
    if (id) setAccountId("");
  };
  const handleAccountChange = (id: string) => {
    setAccountId(id);
    if (id) setLiabilityId("");
  };

  const quickFillTarget = (months: number) => {
    if (emergencyBaseCents <= 0) return;
    setTarget(String(Math.round((emergencyBaseCents * months) / 100)));
  };

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
      purpose: purpose.trim() || null,
      liabilityId: liabilityId || null,
      accountId: accountId || null,
    };
    try {
      await createGoal.mutateAsync(input);
      toast.success("Goal created");
      onClose();
    } catch {
      toast.error("Failed to create goal");
    }
  };

  const emergencyHint = showEmergencyQuickFill && emergencyBaseCents > 0 ? (
    <>
      <div className="row row-sm" style={{ marginTop: 8 }}>
        <Button variant="ghost" size="sm" type="button" onClick={() => quickFillTarget(3)}>Quick fill: 3 months</Button>
        <Button variant="ghost" size="sm" type="button" onClick={() => quickFillTarget(6)}>Quick fill: 6 months</Button>
      </div>
      {!target && (
        <div className="muted money" style={{ fontSize: 12, marginTop: 8 }}>
          Based on your avg. monthly expenses, a 3-month emergency fund would be {money(emergencyBaseCents * 3)}.
        </div>
      )}
    </>
  ) : null;

  return (
    <Card className="goal-form" style={{ marginBottom: 24 }}>
      <div className="h3" style={{ marginBottom: 20 }}>New goal</div>
      <div className="form-grid">
        <Input
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Italy trip, Emergency fund…"
        />
        <Select label="Type" value={type} onChange={(e) => setType(e.target.value)}>
          {Object.entries(TYPE_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
        </Select>
        <Input
          label="Target ($)"
          type="number"
          min="0"
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          placeholder="5000"
          hint={emergencyHint}
        />
        <Input
          label="Monthly contribution ($)"
          type="number"
          min="0"
          value={monthly}
          onChange={(e) => setMonthly(e.target.value)}
          placeholder="500"
        />
        <Input
          label="Target date (optional)"
          type="date"
          value={targetDate}
          onChange={(e) => setTargetDate(e.target.value)}
        />
        <div>
          <label style={{ fontSize: 12, color: "var(--ink-faint)", display: "block", marginBottom: 6 }}>COLOR</label>
          <div className="row row-sm">
            {GOAL_COLORS.map((c, i) => (
              <Swatch
                key={c}
                color={c}
                selected={colorIdx === i}
                onClick={() => setColorIdx(i)}
                label={`Choose ${c}`}
              />
            ))}
          </div>
        </div>
        <Select label="Linked liability (optional)" value={liabilityId} onChange={(e) => handleLiabilityChange(e.target.value)}>
          <option value="">None</option>
          {linkableLiabilities.map((l) => (
            <option key={l.id} value={l.id}>{l.name} · {money(l.balanceCents)}</option>
          ))}
        </Select>
        <Select label="Linked savings account (optional)" value={accountId} onChange={(e) => handleAccountChange(e.target.value)}>
          <option value="">None</option>
          {savingsAccounts.map((a) => (
            <option key={a.id} value={a.id}>{a.bank} {a.name} · {a.apy_pct ?? "—"}% APY</option>
          ))}
        </Select>
        <div style={{ gridColumn: "1 / -1" }}>
          <TextArea
            label="Why this goal? (optional)"
            value={purpose}
            onChange={(e) => setPurpose(e.target.value)}
            placeholder="What will this mean for you? Describe your motivation — a strong 'why' makes it real."
            rows={2}
          />
        </div>
      </div>
      <div className="row row-sm" style={{ marginTop: 20 }}>
        <Button variant="primary" onClick={() => void submit()}>Create goal</Button>
        <Button variant="ghost" onClick={onClose}>Cancel</Button>
      </div>
    </Card>
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

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        Loading goals…
      </div>
    );
  }
  if (error) {
    return (
      <div className="stub" role="alert" aria-live="assertive">
        Error loading goals.
      </div>
    );
  }

  return (
    <div className="screen screen-goals">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Goals · {goals.length} active
          </div>
          <h1>Things you're moving toward.</h1>
        </div>
        <div className="row row-sm" style={{ alignItems: "center" }}>
          {goals.length > 0 && (
            <CopilotNudge
              prompt="How should I prioritize and optimize my savings goals? Show me the tradeoffs between contributing more to each goal."
              label="Optimize my goals"
              variant="accent"
            />
          )}
          <Button onClick={() => setShowNew(true)}>
            <I.Plus /> New goal
          </Button>
        </div>
      </div>

      <p className="muted" style={{ maxWidth: 660, fontSize: 14, lineHeight: 1.6, marginTop: -12, marginBottom: 24 }}>
        A goal is a horizon line on your future runway. Set a target, commit a monthly amount, and watch the ETA shift as your balance grows.
      </p>

      {showNew && <NewGoalForm onClose={() => setShowNew(false)} />}

      {goals.length === 0 && !showNew ? (
        <EmptyState
          icon={<I.Goal style={{ width: 32, height: 32 }} />}
          title="No goals yet"
          description="Create your first goal to track savings, debt payoff, or spending caps."
          actions={
            <Button variant="primary" onClick={() => setShowNew(true)}>
              <I.Plus /> Create a goal
            </Button>
          }
        />
      ) : (
        <>
          {/* Type tabs */}
          {goals.length > 0 && (
            <div className="toolbar" style={{ marginBottom: 20, display: "inline-flex" }} role="tablist" aria-label="Goal type filter">
              <button className={typeFilter === "all" ? "on" : ""} onClick={() => setTypeFilter("all")} role="tab" aria-selected={typeFilter === "all"}>
                All <span className="muted" style={{ marginLeft: 4, fontSize: 11 }}>{goals.length}</span>
              </button>
              {Object.entries(TYPE_LABELS).map(([k, v]) => typeCounts[k] ? (
                <button key={k} className={typeFilter === k ? "on" : ""} onClick={() => setTypeFilter(k as GoalType)} role="tab" aria-selected={typeFilter === k}>
                  {v} <span className="muted" style={{ marginLeft: 4, fontSize: 11 }}>{typeCounts[k]}</span>
                </button>
              ) : null)}
            </div>
          )}

          {/* Goal cards */}
          <div className="stack stack-md">
            {visible.map((g) => <GoalCard key={g.id} goal={g} />)}
            {visible.length === 0 && (
              <Card tight style={{ textAlign: "center", color: "var(--ink-mute)" }}>
                No goals in this category.
              </Card>
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
                    <Card key={g.id} style={{ borderLeft: `3px solid ${color}` }}>
                      <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 4 }}>{g.name}</div>
                      {g.targetDate && (
                        <div style={{ marginBottom: 8 }}>
                          <Badge>
                            {new Date(g.targetDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}
                          </Badge>
                        </div>
                      )}
                      <div style={{ "--accent": color } as React.CSSProperties}>
                        <ProgressBar value={g.currentCents} max={g.targetCents || 1} size="sm" aria-label={`${g.name} progress`} />
                      </div>
                      <div className="row" style={{ justifyContent: "space-between", fontSize: 12.5, marginTop: 8 }}>
                        <span className="muted">{Math.round(pct)}%</span>
                        <span className="num money">{money(g.targetCents - g.currentCents)} left</span>
                      </div>
                    </Card>
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
              <Card>
                <div className="form-grid" style={{ gap: 32 }}>
                  <div>
                    <div className="eyebrow" style={{ marginBottom: 10 }}>Goal</div>
                    <div className="stack stack-sm">
                      {goals.map((g) => (
                        <button
                          key={g.id}
                          onClick={() => { setScenarioId(g.id); setExtra(0); }}
                          className="btn text"
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "center",
                            padding: "10px 12px",
                            borderRadius: 8,
                            background: scenarioGoal.id === g.id ? "var(--surface-2)" : "transparent",
                            border: `1px solid ${scenarioGoal.id === g.id ? "var(--line-2)" : "transparent"}`,
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
                      <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                        <span className="eyebrow">Extra per month</span>
                        <span className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>+{money(extra * 100)}</span>
                      </div>
                      <input
                        className="goal-range"
                        type="range"
                        min="0"
                        max="1500"
                        step="50"
                        value={extra}
                        onChange={(e) => setExtra(parseInt(e.target.value))}
                        aria-label="Extra monthly contribution"
                      />
                      <div className="row" style={{ justifyContent: "space-between", marginTop: 6, fontSize: 11.5, color: "var(--ink-faint)" }}>
                        <span className="num">$0</span><span className="num">$750</span><span className="num">$1,500</span>
                      </div>
                    </div>
                  </div>

                  <Card tone="accent">
                    <div className="eyebrow" style={{ marginBottom: 14 }}>Updated horizon</div>
                    {newMonths !== null ? (
                      <>
                        <div className="row" style={{ alignItems: "baseline", gap: 10 }}>
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
                    <div className="row row-sm" style={{ marginTop: 20 }}>
                      <Button variant="ghost" size="sm" onClick={() => setExtra(0)} style={{ opacity: extra === 0 ? 0.4 : 1 }}>
                        Reset
                      </Button>
                      {extra > 0 && newMonths !== null && (
                        <Button
                          variant="primary"
                          size="sm"
                          disabled={updateMonthly.isPending}
                          onClick={() => void handleApply()}
                        >
                          Apply +{money(extra * 100)}/mo →
                        </Button>
                      )}
                    </div>
                  </Card>
                </div>
              </Card>
            </div>
          )}
        </>
      )}
    </div>
  );
}

import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { useMonthTotals } from "../api/hooks/reports";
import { useAccounts } from "../api/hooks/accounts";
import { useLiabilities } from "../api/hooks/assets";
import { useGoals, useCreateGoal } from "../api/hooks/budget";
import type { GoalDto, NewGoalInput } from "../api/client";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";

type GoalFilter = "all" | "save-by-date" | "build-balance" | "debt-payoff" | "spending-cap";

const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Pay off debt",
  "spending-cap": "Spending cap",
};

function paceLabel(goal: GoalDto) {
  const pct = goal.targetCents > 0 ? goal.currentCents / goal.targetCents : 0;
  if (pct > 0.9) return { label: "Ahead", className: "chip positive" };
  if (goal.monthlyCents <= 0) return { label: "Needs attention", className: "chip warning" };
  return { label: "On track", className: "chip accent" };
}

function GoalCard({ goal }: { goal: GoalDto }) {
  const pct = goal.targetCents > 0 ? Math.min(100, Math.round((goal.currentCents / goal.targetCents) * 100)) : 0;
  const pace = paceLabel(goal);

  return (
    <div className="card" style={{ padding: 22 }}>
      <div style={{ display: "grid", gridTemplateColumns: "1.5fr 1fr 1fr", gap: 24, alignItems: "center" }}>
        <div>
          <div className="row row-sm wrap" style={{ marginBottom: 10 }}>
            <span className="chip">{TYPE_LABELS[goal.goalType] || goal.goalType}</span>
            <span className="chip">Personal</span>
            <span className={pace.className}>{pace.label}</span>
          </div>
          <h2 className="h1" style={{ fontSize: 24 }}>{goal.name}</h2>
          <div className="muted" style={{ marginTop: 6 }}>
            {goal.goalType === "debt-payoff"
              ? `Paying ${money(goal.monthlyCents, { currency: "USD" })}/month`
              : goal.goalType === "spending-cap"
                ? `Cap of ${money(goal.targetCents, { currency: "USD" })} this month`
                : `Auto-moves ${money(goal.monthlyCents, { currency: "USD" })}/month`}
            {goal.targetDate && ` · target ${new Date(goal.targetDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}`}
          </div>
          {goal.liabilityId && <div className="muted" style={{ marginTop: 8 }}>Linked to Car loan</div>}
        </div>

        <div>
          <div className="eyebrow">PROGRESS</div>
          <div className={`goal-bar ${goal.goalType === "spending-cap" && goal.currentCents > goal.targetCents ? "negative" : ""}`} style={{ marginTop: 10 }}>
            <span style={{ width: `${pct}%` }} />
          </div>
          <div className="row" style={{ justifyContent: "space-between", marginTop: 8, fontSize: 13 }}>
            <span className="money">{money(goal.currentCents, { currency: "USD" })}</span>
            <span className="money muted">of {money(goal.targetCents, { currency: "USD" })}</span>
          </div>
        </div>

        <div style={{ textAlign: "right" }}>
          <div className="figure" style={{ fontSize: 34 }}>{pct}%</div>
          <div className="row row-sm" style={{ justifyContent: "flex-end", marginTop: 10 }}>
            <button className="btn ghost sm" type="button">Pause</button>
            <button className="btn outline sm" type="button">Adjust</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function NewGoalForm({ onClose }: { onClose: () => void }) {
  const createGoal = useCreateGoal();
  const { data: totals } = useMonthTotals();
  const { data: liabilities = [] } = useLiabilities();
  const { data: accounts = [] } = useAccounts();
  const [name, setName] = useState("");
  const [goalType, setGoalType] = useState<GoalFilter>("save-by-date");
  const [target, setTarget] = useState("");
  const [monthly, setMonthly] = useState("");
  const [targetDate, setTargetDate] = useState("");
  const [purpose, setPurpose] = useState("");
  const [liabilityId, setLiabilityId] = useState("");
  const [accountId, setAccountId] = useState("");

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
        <label className="stack stack-xs"><span className="muted">Linked liability</span><select className="control" value={liabilityId} onChange={(e) => setLiabilityId(e.target.value)}><option value="">None</option>{liabilities.map((liability) => <option key={liability.id} value={liability.id}>{liability.name}</option>)}</select></label>
        <label className="stack stack-xs" style={{ gridColumn: "1 / -1" }}><span className="muted">Linked savings account</span><select className="control" value={accountId} onChange={(e) => setAccountId(e.target.value)}><option value="">None</option>{accounts.map((account) => <option key={account.id} value={account.id}>{getAccountDisplayName(account)}</option>)}</select></label>
        <label className="stack stack-xs" style={{ gridColumn: "1 / -1" }}><span className="muted">Why this goal?</span><textarea className="control" rows={3} value={purpose} onChange={(e) => setPurpose(e.target.value)} /></label>
      </div>
      <div className="row row-sm" style={{ marginTop: 18 }}><button className="btn primary" type="button" onClick={() => void submit()}>Create goal</button><button className="btn ghost" type="button" onClick={onClose}>Cancel</button></div>
    </div>
  );
}

export default function Goals() {
  const { data: goals = [], isLoading, error } = useGoals();
  const [filter, setFilter] = useState<GoalFilter>("all");
  const [creating, setCreating] = useState(false);

  const counts = useMemo(() => goals.reduce<Record<string, number>>((acc, goal) => {
    acc[goal.goalType] = (acc[goal.goalType] ?? 0) + 1;
    return acc;
  }, {}), [goals]);

  const visible = filter === "all" ? goals : goals.filter((goal) => goal.goalType === filter);
  const sinkingFunds = goals.filter((goal) => goal.goalType === "save-by-date");

  if (isLoading) return <div className="stub">Loading goals…</div>;
  if (error) return <div className="stub" role="alert">Error loading goals.</div>;

  return (
    <div className="screen screen-goals">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />GOALS · {goals.length} ACTIVE</div>
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
      </div>

      {creating && <NewGoalForm onClose={() => setCreating(false)} />}

      <div className="section" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {visible.map((goal) => <GoalCard key={goal.id} goal={goal} />)}
      </div>

      <section className="section">
        <div className="day-hdr" style={{ marginBottom: 14 }}>
          <div>
            <div className="eyebrow"><span className="dot" />SINKING FUNDS · {sinkingFunds.length}</div>
            <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Sinking funds</h2>
          </div>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 14 }}>
          {sinkingFunds.map((goal) => {
            const pct = goal.targetCents > 0 ? Math.min(100, Math.round((goal.currentCents / goal.targetCents) * 100)) : 0;
            return (
              <div key={goal.id} className="card tight">
                <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                  <div><div className="h3">{goal.name}</div><div className="muted" style={{ fontSize: 12.5 }}>{goal.targetDate ? `Due ${new Date(goal.targetDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}` : "No due date"}</div></div>
                  <div className="figure">{pct}%</div>
                </div>
                <div className="goal-bar" style={{ marginTop: 12, height: 5 }}><span style={{ width: `${pct}%` }} /></div>
                <div className="row" style={{ justifyContent: "space-between", marginTop: 8, fontSize: 12.5 }}><span className="money">{money(goal.currentCents, { currency: "USD" })}</span><span className="money muted">of {money(goal.targetCents, { currency: "USD" })}</span></div>
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { useBudgetEnvelopes, useBudgetHistory, useSetBudget } from "../api/hooks/budget";
import { useMonthTotals } from "../api/hooks/reports";
import { commands, type BudgetEnvelope, type SpendingBreakdown } from "../api/client";
import PlanNextMonthModal from "./PlanNextMonthModal";
import EmptyState from "../components/EmptyState";
import { CopilotQuickAsk } from "../components/CopilotQuickAsk";
import { money } from "../utils/format";

type SortKey = "group" | "stress" | "size" | "activity";

function envelopeStatus(env: BudgetEnvelope) {
  if (env.budgetCents <= 0) return { label: "No budget set", tone: "warning" as const, severity: 2 };
  const pct = (env.spentCents / env.budgetCents) * 100;
  if (env.spentCents > env.budgetCents) {
    return { label: `Over by ${money(env.spentCents - env.budgetCents, { currency: "USD" })}`, tone: "negative" as const, severity: 3 };
  }
  if (pct > 90) return { label: "Tight", tone: "warning" as const, severity: 2 };
  if (pct > 60) return { label: "On pace", tone: "accent" as const, severity: 1 };
  return { label: "Plenty left", tone: "positive" as const, severity: 0 };
}

function BudgetInput({ envelope, onClose }: { envelope: BudgetEnvelope; onClose: () => void }) {
  const setBudget = useSetBudget();
  const [value, setValue] = useState(envelope.budgetCents > 0 ? String(Math.round(envelope.budgetCents / 100)) : "");

  const save = async () => {
    const amountCents = Math.round(Number(value || 0) * 100);
    try {
      await setBudget.mutateAsync({ categoryId: envelope.categoryId, amountCents });
      toast.success("Budget saved", { description: `${envelope.categoryLabel} · ${money(amountCents, { currency: "USD" })}` });
      onClose();
    } catch {
      toast.error("Failed to save budget");
    }
  };

  return (
    <div className="card tight" style={{ marginTop: 12, padding: 16 }}>
      <div className="eyebrow">Adjust monthly budget</div>
      <div className="row row-sm" style={{ marginTop: 10, alignItems: "center", flexWrap: "wrap" }}>
        <input
          className="control"
          type="number"
          min="0"
          step="10"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void save();
            if (e.key === "Escape") onClose();
          }}
          aria-label={`Budget amount for ${envelope.categoryLabel}`}
          style={{ maxWidth: 180 }}
        />
        <button className="btn primary sm" type="button" onClick={() => void save()}>Save</button>
        <button className="btn ghost sm" type="button" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}

function EnvelopeCard({ env, editing, onEdit }: { env: BudgetEnvelope; editing: boolean; onEdit: () => void }) {
  const status = envelopeStatus(env);
  const remaining = env.budgetCents - env.spentCents;
  const pct = env.budgetCents > 0 ? Math.min(100, (env.spentCents / env.budgetCents) * 100) : 0;
  const toneClass = status.tone === "negative" ? "negative" : status.tone === "warning" ? "warning" : status.tone === "positive" ? "positive" : "accent";
  const daysLeft = Math.max(1, new Date(new Date().getFullYear(), new Date().getMonth() + 1, 0).getDate() - new Date().getDate());
  const perDay = remaining > 0 ? Math.round(remaining / daysLeft) : 0;

  return (
    <div
      className="card"
      style={{
        padding: 22,
        borderColor: status.tone === "negative" ? "var(--negative)" : status.tone === "warning" ? "var(--warning)" : "var(--line)",
        background: status.tone === "negative"
          ? "linear-gradient(180deg, var(--negative-2) 0%, var(--surface) 70%)"
          : status.tone === "warning"
            ? "linear-gradient(180deg, var(--warning-2) 0%, var(--surface) 70%)"
            : "var(--surface)",
      }}
    >
      <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
        <div>
          <div className="row row-sm" style={{ alignItems: "center", marginBottom: 8 }}>
            <span className="cswatch" style={{ background: env.categoryColor || "var(--accent)" }} />
            <strong>{env.categoryLabel}</strong>
            <span className="muted" style={{ fontSize: 12 }}>{env.txnCount} txn{env.txnCount === 1 ? "" : "s"}</span>
          </div>
          <div className="figure money" style={{ fontSize: 34, lineHeight: 1, color: remaining < 0 ? "var(--negative)" : "var(--ink)" }}>
            {money(Math.abs(remaining), { currency: "USD" })}
          </div>
          <div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>{remaining < 0 ? "over budget" : "left to spend"}</div>
        </div>
        <span className={`chip ${toneClass}`}>{status.label}</span>
      </div>

      <div className="goal-bar" style={{ marginTop: 16, height: 7 }}>
        <span
          style={{
            width: `${pct}%`,
            background: status.tone === "negative" ? "var(--negative)" : status.tone === "warning" ? "var(--warning)" : env.categoryColor || "var(--accent)",
            boxShadow: status.tone === "negative" ? "0 0 12px var(--negative-2)" : status.tone === "warning" ? "0 0 12px var(--warning-2)" : `0 0 12px ${env.categoryColor || "var(--accent-3)"}`,
          }}
        />
      </div>

      <div className="hero-meta" style={{ justifyContent: "space-between", marginTop: 10 }}>
        <span className="money">{money(env.spentCents, { currency: "USD" })} spent</span>
        <span className="money">of {money(env.budgetCents, { currency: "USD" })}</span>
      </div>

      {status.tone === "negative" && <button className="btn outline sm" type="button" style={{ marginTop: 14, width: "100%" }}>Cover from another envelope</button>}

      {status.tone === "warning" && remaining > 0 && (
        <div className="card tight" style={{ marginTop: 14, padding: 12, background: "var(--warning-2)", borderColor: "var(--warning)" }}>
          <div className="muted" style={{ fontSize: 12.5 }}>
            About <span className="money strong">{money(perDay * 100, { currency: "USD" })}</span>/day left to stay under.
          </div>
        </div>
      )}

      <div className="row row-sm" style={{ marginTop: 14 }}>
        <button className="btn ghost sm" type="button" onClick={onEdit}>{editing ? "Editing…" : env.budgetCents > 0 ? "Adjust budget" : "Set budget"}</button>
      </div>
    </div>
  );
}

export default function Budget() {
  const { data: envelopes = [], isLoading, error } = useBudgetEnvelopes();
  const { data: history = [] } = useBudgetHistory(5);
  const { data: totals } = useMonthTotals();
  const { data: breakdown } = useQuery<SpendingBreakdown>({
    queryKey: ["spending-breakdown"],
    queryFn: async () => {
      const result = await commands.getSpendingBreakdown();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
  const [sort, setSort] = useState<SortKey>("group");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showPlan, setShowPlan] = useState(false);

  const now = new Date();
  const totalDays = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
  const today = now.getDate();
  const monthLabel = now.toLocaleDateString("en-US", { month: "long", year: "numeric" });
  const monthPct = Math.round((today / totalDays) * 100);

  const sorted = useMemo(() => [...envelopes].sort((a, b) => {
    if (sort === "stress") return envelopeStatus(b).severity - envelopeStatus(a).severity || b.spentCents - a.spentCents;
    if (sort === "size") return b.budgetCents - a.budgetCents;
    if (sort === "activity") return b.txnCount - a.txnCount;
    return (a.groupLabel || "").localeCompare(b.groupLabel || "") || a.categoryLabel.localeCompare(b.categoryLabel);
  }), [envelopes, sort]);

  const totalBudget = sorted.reduce((sum, env) => sum + env.budgetCents, 0);
  const totalSpent = sorted.reduce((sum, env) => sum + env.spentCents, 0);
  const projectedEom = today > 0 ? Math.round((totalSpent / today) * totalDays) : 0;
  const remaining = totalBudget - totalSpent;
  const toBudget = (totals?.incomeCents ?? 0) - totalBudget;
  const attention = sorted.filter((env) => envelopeStatus(env).severity >= 2);
  const grouped = Object.entries(sorted.reduce<Record<string, BudgetEnvelope[]>>((acc, env) => {
    const key = sort === "group" ? env.groupLabel || "Other" : "All envelopes";
    acc[key] ||= [];
    acc[key].push(env);
    return acc;
  }, {}));

  const insight = attention.length > 0
    ? `${attention.length} envelope${attention.length === 1 ? "" : "s"} need attention. ${projectedEom > totalBudget ? "You are trending over plan." : "The rest of the month still fits the plan."}`
    : projectedEom > totalBudget
      ? "You are trending over plan even though no single envelope is flashing red yet."
      : "The month is on pace right now — most envelopes still have room.";

  const totalTagged = breakdown ? breakdown.fixedCents + breakdown.investmentsCents + breakdown.savingsCents + breakdown.guiltFreeCents + breakdown.untaggedCents : 0;

  if (isLoading) return <div className="stub" aria-live="polite" aria-busy="true">Loading budget…</div>;
  if (error) return <div className="stub" role="alert">Error loading budget.</div>;

  return (
    <div className="screen screen-budget">
      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />BUDGET · {monthLabel.toUpperCase()} · DAY {today} OF {totalDays}</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Where the plan stands today.</h1>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}>
          <button className="btn primary" type="button" onClick={() => setShowPlan(true)}>Plan next month</button>
          <div className="toolbar"><button className="on" type="button">Envelope</button><button type="button">Tracking</button></div>
        </div>
      </header>

      <div className="card accent" style={{ padding: 28 }}>
        <div style={{ display: "grid", gridTemplateColumns: "1.4fr 3fr", gap: 24 }}>
          <div>
            <div className="eyebrow">MONTH PROGRESS</div>
            <div className="hero-num">
              <div className="figure money" style={{ fontSize: 56, lineHeight: 1, color: remaining < 0 ? "var(--negative)" : "var(--accent)" }}>{money(Math.max(remaining, 0), { currency: "USD" })}</div>
              <div className="muted">left to spend</div>
            </div>
            <div style={{ position: "relative", height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 4 }}>
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${monthPct}%`, background: "var(--ink-faint)", opacity: 0.4, borderRadius: 999 }} title="Time elapsed" />
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${totalBudget > 0 ? Math.min(100, (totalSpent / totalBudget) * 100) : 0}%`, background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} title="Spent" />
            </div>
            <div className="hero-meta" style={{ marginTop: 10 }}>
              <span>{monthPct}% through {now.toLocaleString("en-US", { month: "long" })}</span>
              <span>{totalBudget > 0 ? Math.round((totalSpent / totalBudget) * 100) : 0}% spent</span>
              <span>{totalDays - today} days left</span>
            </div>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 14 }}>
            <div className="stat"><div className="label">Budgeted</div><div className="value money">{money(totalBudget, { currency: "USD" })}</div><div className="sub">Across {sorted.length} envelopes</div></div>
            <div className="stat"><div className="label">Spent so far</div><div className="value money">{money(totalSpent, { currency: "USD" })}</div><div className="sub">{today > 0 ? money(Math.round(totalSpent / today), { currency: "USD" }) : money(0, { currency: "USD" })}/day pace</div></div>
            <div className="stat accent"><div className="label">Projected EOM</div><div className="value money">{money(projectedEom, { currency: "USD" })}</div><div className="sub">{projectedEom > totalBudget ? <span className="npill neg">Over plan</span> : <span className="npill pos">Under plan</span>}</div></div>
          </div>
        </div>
        <p className="muted" style={{ marginTop: 18, marginBottom: 0, maxWidth: 900 }}>{insight}</p>
      </div>

      <div className="card tight" style={{ marginTop: 16, padding: 18, display: "grid", gridTemplateColumns: "1.7fr auto", gap: 16, alignItems: "center" }}>
        <div>
          <div className="eyebrow"><span className="dot" />TO BUDGET · UNASSIGNED</div>
          <div className="row row-sm wrap" style={{ alignItems: "baseline", marginTop: 8 }}>
            <div className="figure money" style={{ fontSize: 32, color: toBudget >= 0 ? "var(--accent)" : "var(--negative)" }}>{money(Math.abs(toBudget), { currency: "USD" })}</div>
            <div className="muted">of <span className="money">{money(totals?.incomeCents ?? 0, { currency: "USD" })}</span> income · <span className="money">{money(totalBudget, { currency: "USD" })}</span> assigned</div>
          </div>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}><button className="btn outline sm" type="button">Assign to a goal</button><button className="btn sm" type="button">Park in House Fund</button></div>
      </div>

      {breakdown && totalTagged > 0 && <div className="card tight" style={{ marginTop: 16 }}><div className="eyebrow">SPENDING MIX</div><div className="stream" style={{ marginTop: 10, height: 16, borderRadius: 6 }}><span style={{ width: `${(breakdown.fixedCents / totalTagged) * 100}%`, background: "var(--ink-mute)" }} /><span style={{ width: `${(breakdown.investmentsCents / totalTagged) * 100}%`, background: "var(--accent)" }} /><span style={{ width: `${(breakdown.savingsCents / totalTagged) * 100}%`, background: "var(--positive)" }} /><span style={{ width: `${(breakdown.guiltFreeCents / totalTagged) * 100}%`, background: "var(--c-dining)" }} /><span style={{ width: `${(breakdown.untaggedCents / totalTagged) * 100}%`, background: "var(--ink-faint)" }} /></div></div>}

      {attention.length > 0 && <section className="section"><div className="day-hdr" style={{ marginBottom: 14 }}><div><div className="eyebrow"><span className="dot" />NEEDS A GLANCE · {attention.length}</div><h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Just these — the rest is fine.</h2></div></div><div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 14 }}>{attention.map((env) => <div key={env.categoryId}><EnvelopeCard env={env} editing={editingId === env.categoryId} onEdit={() => setEditingId(env.categoryId)} />{editingId === env.categoryId && <BudgetInput envelope={env} onClose={() => setEditingId(null)} />}</div>)}</div></section>}

      <section className="section">
        <div className="day-hdr" style={{ marginBottom: 14 }}><div><div className="eyebrow"><span className="dot" />ALL ENVELOPES</div><h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Each one, on its own.</h2></div><div className="toolbar"><button className={sort === "group" ? "on" : ""} type="button" onClick={() => setSort("group")}>By group</button><button className={sort === "stress" ? "on" : ""} type="button" onClick={() => setSort("stress")}>By stress</button><button className={sort === "size" ? "on" : ""} type="button" onClick={() => setSort("size")}>By size</button><button className={sort === "activity" ? "on" : ""} type="button" onClick={() => setSort("activity")}>By activity</button></div></div>
        {sorted.length === 0 ? <EmptyState title="No envelopes yet" description="Import transactions or set a budget to see the month take shape." /> : <div style={{ display: "flex", flexDirection: "column", gap: 28 }}>{grouped.map(([label, items]) => <div key={label}><div className="eyebrow" style={{ marginBottom: 12 }}>{label}</div><div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 14 }}>{items.map((env) => <div key={env.categoryId}><EnvelopeCard env={env} editing={editingId === env.categoryId} onEdit={() => setEditingId(env.categoryId)} />{editingId === env.categoryId && <BudgetInput envelope={env} onClose={() => setEditingId(null)} />}</div>)}</div></div>)}</div>}
      </section>

      {history.length > 0 && <section className="section"><div className="eyebrow" style={{ marginBottom: 12 }}>Spending history · last 5 months</div><div className="card flush"><table className="tbl"><thead><tr><th>Category</th>{history[0]?.monthly.map((m) => <th key={m.month} className="right">{m.label}</th>)}</tr></thead><tbody>{history.map((row) => <tr key={row.categoryId}><td><span className="cswatch" style={{ background: row.color || "var(--accent)" }} /> {row.label}</td>{row.monthly.map((m) => <td key={m.month} className="right"><span className="money">{money(m.cents, { currency: "USD" })}</span></td>)}</tr>)}</tbody></table></div></section>}

      <CopilotQuickAsk prompt="Looking at my current budget, what should I rebalance first to improve my financial health?" label="Ask Copilot about budget" />
      {showPlan && <PlanNextMonthModal onClose={() => setShowPlan(false)} />}
    </div>
  );
}

import { useState } from "react";
import { toast } from "sonner";
import { useBudgetEnvelopes, useSetBudget } from "../api/hooks/budget";
import type { BudgetEnvelope } from "../api/client";
import * as I from "../components/Icons";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

type SortKey = "group" | "stress" | "size";

function envelopeStatus(e: BudgetEnvelope) {
  if (e.budgetCents === 0) return { label: "No budget set", tone: "neutral", severity: 0 };
  const pct = (e.spentCents / e.budgetCents) * 100;
  if (e.spentCents > e.budgetCents) return { label: `Over by ${fmt(e.spentCents - e.budgetCents)}`, tone: "negative", severity: 3 };
  if (pct > 90) return { label: "Tight", tone: "warning", severity: 2 };
  if (pct > 60) return { label: "On pace", tone: "neutral", severity: 1 };
  return { label: "Plenty left", tone: "positive", severity: 0 };
}

function BudgetInput({ envelope, onClose }: { envelope: BudgetEnvelope; onClose: () => void }) {
  const setBudget = useSetBudget();
  const [value, setValue] = useState(
    envelope.budgetCents > 0 ? String(Math.round(envelope.budgetCents / 100)) : ""
  );

  const save = async () => {
    const cents = Math.round(parseFloat(value || "0") * 100);
    try {
      await setBudget.mutateAsync({ categoryId: envelope.categoryId, amountCents: cents });
      toast.success("Budget saved", { description: `${envelope.categoryLabel}: ${fmt(cents)}/mo` });
      onClose();
    } catch {
      toast.error("Failed to save budget");
    }
  };

  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
      <span style={{ color: "var(--ink-mute)", fontSize: 13 }}>$</span>
      <input
        type="number"
        min="0"
        step="10"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") void save(); if (e.key === "Escape") onClose(); }}
        autoFocus
        style={{
          width: 80,
          background: "var(--surface-2)",
          border: "1px solid var(--accent)",
          borderRadius: 6,
          padding: "4px 8px",
          fontSize: 13,
          color: "var(--ink)",
          outline: "none",
        }}
      />
      <button className="btn sm primary" onClick={() => void save()} style={{ padding: "4px 10px" }}>Save</button>
      <button className="btn sm ghost" onClick={onClose} style={{ padding: "4px 8px" }}>✕</button>
    </div>
  );
}

function EnvelopeCard({ env, onEdit }: { env: BudgetEnvelope; onEdit: () => void }) {
  const status = envelopeStatus(env);
  const pct = env.budgetCents > 0 ? Math.min(100, (env.spentCents / env.budgetCents) * 100) : 0;
  const remaining = env.budgetCents - env.spentCents;
  const color = env.categoryColor || "var(--ink-mute)";

  return (
    <div
      className="card tight"
      style={{
        padding: 18,
        borderColor: status.tone === "negative" ? "var(--negative)" : status.tone === "warning" ? "var(--warning)" : "var(--line)",
        cursor: "pointer",
      }}
      onClick={onEdit}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ width: 10, height: 10, borderRadius: 3, background: color, flexShrink: 0, display: "inline-block" }} />
          <span style={{ fontSize: 13.5, fontWeight: 500 }}>{env.categoryLabel}</span>
        </div>
        <span
          className={`chip ${status.tone === "negative" ? "negative" : status.tone === "warning" ? "warning" : status.tone === "positive" ? "positive" : ""}`}
          style={{ fontSize: 11 }}
        >
          {status.label}
        </span>
      </div>

      {/* Progress bar */}
      <div style={{ height: 5, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginBottom: 10 }}>
        <div style={{
          width: pct + "%",
          height: "100%",
          background: status.tone === "negative" ? "var(--negative)" : color,
          borderRadius: 999,
          transition: "width .3s",
        }} />
      </div>

      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12.5 }}>
        <span className="muted">{fmt(env.spentCents)} spent</span>
        {env.budgetCents > 0 ? (
          <span className={`num ${remaining < 0 ? "neg" : ""}`}>
            {remaining >= 0 ? fmt(remaining) + " left" : fmt(-remaining) + " over"}
          </span>
        ) : (
          <span className="muted">No budget · click to set</span>
        )}
      </div>

      {env.budgetCents > 0 && (
        <div className="muted" style={{ fontSize: 12, marginTop: 4, fontFamily: "var(--mono)" }}>
          of {fmt(env.budgetCents)} · {env.txnCount} txn{env.txnCount !== 1 ? "s" : ""}
        </div>
      )}
    </div>
  );
}

export default function Budget() {
  const { data: envelopes = [], isLoading, error } = useBudgetEnvelopes();
  const [sort, setSort] = useState<SortKey>("group");
  const [editingId, setEditingId] = useState<string | null>(null);

  const now = new Date();
  const todayDay = now.getDate();
  const totalDays = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
  const monthPct = (todayDay / totalDays) * 100;
  const monthLabel = now.toLocaleString("default", { month: "long", year: "numeric" });

  const totalBudget = envelopes.reduce((s, e) => s + e.budgetCents, 0);
  const totalSpent = envelopes.reduce((s, e) => s + e.spentCents, 0);
  const projectedEom = todayDay > 0 ? Math.round((totalSpent / todayDay) * totalDays) : 0;

  const sorted = [...envelopes].sort((a, b) => {
    if (sort === "stress") return envelopeStatus(b).severity - envelopeStatus(a).severity || b.spentCents - a.spentCents;
    if (sort === "size")   return b.budgetCents - a.budgetCents;
    return (a.groupLabel || "").localeCompare(b.groupLabel || "") || a.categoryLabel.localeCompare(b.categoryLabel);
  });

  const attention = sorted.filter((e) => envelopeStatus(e).severity >= 2);

  if (isLoading) return <div className="stub">Loading budget…</div>;
  if (error)     return <div className="stub">Error loading budget.</div>;

  const noData = envelopes.length === 0;

  return (
    <div className="screen">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Budget · {monthLabel} · day {todayDay} of {totalDays}
          </div>
          <h1>Where the plan stands today.</h1>
        </div>
        <div className="toolbar">
          <button className={sort === "group" ? "on" : ""} onClick={() => setSort("group")}>By group</button>
          <button className={sort === "stress" ? "on" : ""} onClick={() => setSort("stress")}>By stress</button>
          <button className={sort === "size" ? "on" : ""} onClick={() => setSort("size")}>By size</button>
        </div>
      </div>

      {noData ? (
        <div className="card" style={{ textAlign: "center", padding: "64px 32px" }}>
          <I.Lego style={{ color: "var(--ink-faint)", width: 32, height: 32, margin: "0 auto 16px" }} />
          <div style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>No envelopes yet</div>
          <div className="muted" style={{ fontSize: 14, marginBottom: 24, maxWidth: 400, margin: "0 auto 24px" }}>
            Import transactions first, then click any category card below to set a monthly budget.
          </div>
        </div>
      ) : (
        <>
          {/* Month progress card */}
          <div className="card" style={{ background: "linear-gradient(135deg, var(--accent-2) 0%, var(--surface) 60%)", border: "1px solid var(--accent-3)" }}>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 28, alignItems: "start" }}>
              <div>
                <div className="eyebrow" style={{ marginBottom: 10 }}>Left to spend</div>
                <div className="figure money" style={{ fontSize: 44, lineHeight: 1, color: totalBudget > 0 && totalSpent > totalBudget ? "var(--negative)" : "var(--accent)" }}>
                  {fmt(Math.max(0, totalBudget - totalSpent))}
                </div>
                <div style={{ marginTop: 14, position: "relative", height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                  <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: monthPct + "%", background: "var(--ink-faint)", opacity: 0.25, borderRadius: 999 }} />
                  {totalBudget > 0 && (
                    <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: Math.min(100, (totalSpent / totalBudget) * 100) + "%", background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} />
                  )}
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 11.5, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
                  <span>{Math.round(monthPct)}% through month</span>
                  <span>{totalDays - todayDay}d left</span>
                </div>
              </div>
              <div>
                <div className="eyebrow" style={{ marginBottom: 8 }}>Spent so far</div>
                <div className="figure money" style={{ fontSize: 28 }}>{fmt(totalSpent)}</div>
                <div className="muted" style={{ fontSize: 12.5, marginTop: 4 }}>
                  {totalBudget > 0 ? `of ${fmt(totalBudget)} budgeted` : "no budget set"}
                </div>
              </div>
              <div>
                <div className="eyebrow" style={{ marginBottom: 8 }}>Projected EOM</div>
                <div className="figure money" style={{ fontSize: 28 }}>{fmt(projectedEom)}</div>
                {totalBudget > 0 && (
                  <div style={{ marginTop: 4 }}>
                    {projectedEom <= totalBudget ? (
                      <span className="chip positive" style={{ fontSize: 11 }}>
                        {fmt(totalBudget - projectedEom)} under plan
                      </span>
                    ) : (
                      <span className="chip negative" style={{ fontSize: 11 }}>
                        {fmt(projectedEom - totalBudget)} over plan
                      </span>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Needs attention */}
          {attention.length > 0 && (
            <div className="section">
              <div className="eyebrow" style={{ marginBottom: 12 }}>
                <span className="dot" style={{ background: "var(--negative)", boxShadow: "0 0 6px var(--negative)" }} />
                Needs a glance · {attention.length}
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 12 }}>
                {attention.map((e) =>
                  editingId === e.categoryId ? (
                    <div key={e.categoryId} className="card tight" style={{ padding: 18 }}>
                      <div style={{ fontSize: 13.5, fontWeight: 500, marginBottom: 10 }}>{e.categoryLabel}</div>
                      <BudgetInput envelope={e} onClose={() => setEditingId(null)} />
                    </div>
                  ) : (
                    <EnvelopeCard key={e.categoryId} env={e} onEdit={() => setEditingId(e.categoryId)} />
                  )
                )}
              </div>
            </div>
          )}

          {/* All envelopes */}
          <div className="section">
            <div className="eyebrow" style={{ marginBottom: 12 }}>
              All envelopes · {sorted.length}
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 12 }}>
              {sorted.map((e) =>
                editingId === e.categoryId ? (
                  <div key={e.categoryId} className="card tight" style={{ padding: 18 }}>
                    <div style={{ fontSize: 13.5, fontWeight: 500, marginBottom: 10 }}>{e.categoryLabel}</div>
                    <BudgetInput envelope={e} onClose={() => setEditingId(null)} />
                  </div>
                ) : (
                  <EnvelopeCard key={e.categoryId} env={e} onEdit={() => setEditingId(e.categoryId)} />
                )
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

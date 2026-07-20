import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { useBudgetEnvelopes, useBudgetHistory, useSetBudget, useGoals, useContributeToGoal } from "../api/hooks/budget";
import { useMonthTotals } from "../api/hooks/reports";
import { commands, type BudgetEnvelope, type SpendingBreakdown } from "../api/client";
import PlanNextMonthModal from "./PlanNextMonthModal";
import EmptyState from "../components/EmptyState";
import { money } from "../utils/format";

type SortKey = "group" | "stress" | "size" | "activity";

function envelopeStatus(env: BudgetEnvelope) {
  const available = env.budgetCents + env.carryoverCents;
  if (available <= 0 && env.budgetCents <= 0) return { label: "No budget set", tone: "warning" as const, severity: 2 };
  const pct = available > 0 ? (env.spentCents / available) * 100 : 100;
  if (env.spentCents > available) {
    return { label: `Over by ${money(env.spentCents - available)}`, tone: "negative" as const, severity: 3 };
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
      toast.success("Budget saved", { description: `${envelope.categoryLabel} · ${money(amountCents)}` });
      onClose();
    } catch {
      toast.error("Failed to save budget");
    }
  };

  return (
    <div className="card tight" style={{ marginTop: 12, padding: 16 }}>
      <div className="eyebrow"><span className="dot" />Adjust monthly budget</div>
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

function EnvelopeCard({ env, editing, onEdit, donor }: { env: BudgetEnvelope; editing: boolean; onEdit: () => void; donor: BudgetEnvelope | null }) {
  const status = envelopeStatus(env);
  const available = env.budgetCents + env.carryoverCents;
  const remaining = available - env.spentCents;
  const pct = available > 0 ? Math.min(100, (env.spentCents / available) * 100) : 0;
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
            {money(Math.abs(remaining))}
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
        <span className="money">{money(env.spentCents)} spent</span>
        <span className="money">of {money(available)}</span>
      </div>

      {env.carryoverCents !== 0 && (
        <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--hairline)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 12 }}>Carried from last month</span>
          <span className="money" style={{ fontSize: 12.5, color: env.carryoverCents > 0 ? "var(--positive)" : "var(--negative)" }}>
            {env.carryoverCents > 0 ? "+" : ""}{money(env.carryoverCents)}
          </span>
        </div>
      )}

      {status.tone === "negative" && (
        <button
          className="btn outline sm"
          type="button"
          style={{ marginTop: 14, width: "100%" }}
          onClick={() => {
            if (!donor) {
              toast("No envelope has spare room to cover this right now.");
              return;
            }
            const donorRemaining = donor.budgetCents - donor.spentCents;
            toast(`${donor.categoryLabel} has ${money(donorRemaining)} unspent — often the best donor.`, {
              description: "Adjust each envelope's budget below to move the amount over.",
            });
          }}
        >
          Cover from another envelope
        </button>
      )}

      {status.tone === "warning" && remaining > 0 && (
        <div className="card tight" style={{ marginTop: 14, padding: 12, background: "var(--warning-2)", borderColor: "var(--warning)" }}>
          <div className="muted" style={{ fontSize: 12.5 }}>
            About <span className="money strong">{money(perDay)}</span>/day left to stay under.
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
  const navigate = useNavigate();
  const { data: envelopes = [], isLoading, error } = useBudgetEnvelopes();
  const { data: history = [] } = useBudgetHistory(5);
  const { data: totals } = useMonthTotals();
  const { data: goals = [] } = useGoals();
  const contribute = useContributeToGoal();
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
  const [searchParams, setSearchParams] = useSearchParams();
  const pendingScrollRef = useRef<string | null>(null);

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
  const totalCarryover = sorted.reduce((sum, env) => sum + env.carryoverCents, 0);
  const totalAvailable = totalBudget + totalCarryover;
  const totalSpent = sorted.reduce((sum, env) => sum + env.spentCents, 0);
  const projectedEom = today > 0 ? Math.round((totalSpent / today) * totalDays) : 0;
  const remaining = totalAvailable - totalSpent;
  const toBudget = (totals?.incomeCents ?? 0) - totalBudget;
  const unbudgeted = sorted.filter((env) => env.budgetCents <= 0 && env.spentCents <= 0 && env.carryoverCents === 0);
  // Unbudgeted categories aren't "in trouble" (severity>=2 from "No budget
  // set" is really "unconfigured") — they get their own section below instead
  // of also cluttering "Needs a glance".
  const attention = sorted.filter((env) => envelopeStatus(env).severity >= 2 && !unbudgeted.includes(env));
  const grouped = Object.entries(sorted.filter((env) => !unbudgeted.includes(env)).reduce<Record<string, BudgetEnvelope[]>>((acc, env) => {
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

  // Deep-link support: ?focusCategory=<id-or-label> opens that envelope's
  // editor, matching the focus idiom used by Accounts, Goals and Recurring.
  // The Copilot links here after applying a budget change so the user can see
  // the result rather than take "done" on trust.
  const focusedCategoryId = useMemo(() => {
    const focus = searchParams.get("focusCategory");
    if (!focus) return null;
    // Accept the label as well as the id — a link may be built from either.
    const match = envelopes.find(
      (env) => env.categoryId === focus || env.categoryLabel.toLowerCase() === focus.toLowerCase(),
    );
    return match?.categoryId ?? null;
  }, [envelopes, searchParams]);

  useEffect(() => {
    if (!searchParams.has("focusCategory")) return;
    // Wait for envelopes before deciding — otherwise a slow load looks like
    // "category not found" and the param is dropped before it can match.
    if (isLoading) return;
    if (focusedCategoryId && !editingId) {
      setEditingId(focusedCategoryId);
      pendingScrollRef.current = focusedCategoryId;
    }
    // Clear the param either way, so a stale link to a deleted category does
    // not stick in the URL and re-fire on every render.
    const next = new URLSearchParams(searchParams);
    next.delete("focusCategory");
    setSearchParams(next, { replace: true });
  }, [focusedCategoryId, editingId, isLoading, searchParams, setSearchParams]);

  // Runs after every render until the focused envelope is on screen: the row
  // does not exist yet on the render that sets `editingId`.
  useEffect(() => {
    const target = pendingScrollRef.current;
    if (!target) return;
    const el = document.querySelector(`[data-envelope-id="${CSS.escape(target)}"]`);
    if (!el) return;
    pendingScrollRef.current = null;
    // jsdom and older webviews do not implement scrollIntoView.
    el.scrollIntoView?.({ block: "center" });
  });

  const donorFor = (categoryId: string): BudgetEnvelope | null => {
    const candidates = sorted.filter((env) => env.categoryId !== categoryId && env.budgetCents - env.spentCents > 0);
    if (candidates.length === 0) return null;
    return candidates.reduce((best, env) => (env.budgetCents - env.spentCents > best.budgetCents - best.spentCents ? env : best));
  };

  // Only manual (non-account-linked) goals can be parked into: a linked goal's
  // balance is synced from its account, so a manual bump double-counts.
  const parkableGoal = goals.find((goal) => !goal.accountId) ?? null;

  const handleParkInGoal = async () => {
    const firstGoal = parkableGoal;
    if (!firstGoal) {
      toast("No manual goals to park funds in yet — create one on the Goals screen.");
      return;
    }
    if (toBudget <= 0) {
      toast("Nothing unassigned to park right now.");
      return;
    }
    try {
      await contribute.mutateAsync({ id: firstGoal.id, amountCents: toBudget, note: "Parked unassigned budget", source: "sweep" });
      toast.success(`Parked ${money(toBudget)} in ${firstGoal.name}`);
    } catch {
      toast.error("Could not park funds");
    }
  };

  if (isLoading) {
    // Mirrors the real grid so the page does not collapse to one line and then
    // snap back into three columns once data lands.
    return (
      <div className="budget-loading" aria-live="polite" aria-busy="true">
        <span className="sr-only">Loading budget…</span>
        <div className="skeleton heading" style={{ width: 220 }} />
        <div className="budget-grid" aria-hidden="true">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="card budget-skel-card">
              <div className="skeleton text" style={{ width: "55%" }} />
              <div className="skeleton" style={{ height: 34, width: "45%", margin: "10px 0 8px" }} />
              <div className="skeleton text" style={{ width: "35%" }} />
              <div className="skeleton" style={{ height: 6, marginTop: 16 }} />
            </div>
          ))}
        </div>
      </div>
    );
  }
  if (error) return <div className="stub" role="alert">Error loading budget.</div>;

  return (
    <div className="screen screen-budget">
      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Budget · {monthLabel} · day {today} of {totalDays}</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Where the plan stands today.</h1>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}>
          <button className="btn primary" type="button" onClick={() => setShowPlan(true)}>Plan next month</button>
        </div>
      </header>

      <div className="card accent" style={{ padding: 28 }}>
        <div style={{ display: "grid", gridTemplateColumns: "1.4fr 3fr", gap: 24 }}>
          <div>
            <div className="eyebrow"><span className="dot" />Month progress</div>
            <div className="hero-num">
              <div className="figure money" style={{ fontSize: 56, lineHeight: 1, color: remaining < 0 ? "var(--negative)" : "var(--accent)" }}>{money(Math.max(remaining, 0))}</div>
              <div className="muted">left to spend</div>
            </div>
            <div style={{ position: "relative", height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 4 }}>
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${monthPct}%`, background: "var(--ink-faint)", opacity: 0.4, borderRadius: 999 }} title="Time elapsed" />
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${totalAvailable > 0 ? Math.min(100, (totalSpent / totalAvailable) * 100) : 0}%`, background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} title="Spent" />
            </div>
            <div className="hero-meta" style={{ marginTop: 10 }}>
              <span>{monthPct}% through {now.toLocaleString("en-US", { month: "long" })}</span>
              <span>{totalBudget > 0 ? Math.round((totalSpent / totalBudget) * 100) : 0}% spent</span>
              <span>{totalDays - today} days left</span>
            </div>
          </div>
          <div className="budget-grid">
            <div className="stat"><div className="label">Budgeted</div><div className="value money">{money(totalBudget)}</div><div className="sub">Across {sorted.length} envelopes</div></div>
            <div className="stat"><div className="label">Spent so far</div><div className="value money">{money(totalSpent)}</div><div className="sub">{today > 0 ? money(Math.round(totalSpent / today)) : money(0)}/day pace</div></div>
            <div className="stat accent"><div className="label">Projected EOM</div><div className="value money">{money(projectedEom)}</div><div className="sub">{projectedEom > totalAvailable ? <span className="npill neg">Over by {money(projectedEom - totalAvailable)}</span> : <span className="npill pos">Under by {money(totalAvailable - projectedEom)}</span>}</div></div>
          </div>
        </div>
        <p className="muted" style={{ marginTop: 18, marginBottom: 0, maxWidth: 900 }}>{insight}</p>
      </div>

      <div className="card tight" style={{ marginTop: 16, padding: 18, display: "grid", gridTemplateColumns: "1.7fr auto", gap: 16, alignItems: "center" }}>
        <div>
          <div className="eyebrow"><span className="dot" />To budget · unassigned</div>
          <div className="row row-sm wrap" style={{ alignItems: "baseline", marginTop: 8 }}>
            <div className="figure money" style={{ fontSize: 32, color: toBudget >= 0 ? "var(--accent)" : "var(--negative)" }}>{money(Math.abs(toBudget))}</div>
            <div className="muted">of <span className="money">{money(totals?.incomeCents ?? 0)}</span> income · <span className="money">{money(totalBudget)}</span> assigned</div>
          </div>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}><button className="btn outline sm" type="button" onClick={() => navigate("/goals")}>Assign to a goal</button><button className="btn sm" type="button" disabled={contribute.isPending} onClick={() => void handleParkInGoal()}>Park in {parkableGoal?.name ?? "a goal"}</button></div>
      </div>

      {breakdown && totalTagged > 0 && <div className="card tight" style={{ marginTop: 16 }}><div className="eyebrow"><span className="dot" />Spending mix</div><div className="stream" style={{ marginTop: 10, height: 16, borderRadius: 6 }}><span style={{ width: `${(breakdown.fixedCents / totalTagged) * 100}%`, background: "var(--ink-mute)" }} /><span style={{ width: `${(breakdown.investmentsCents / totalTagged) * 100}%`, background: "var(--accent)" }} /><span style={{ width: `${(breakdown.savingsCents / totalTagged) * 100}%`, background: "var(--positive)" }} /><span style={{ width: `${(breakdown.guiltFreeCents / totalTagged) * 100}%`, background: "var(--c-dining)" }} /><span style={{ width: `${(breakdown.untaggedCents / totalTagged) * 100}%`, background: "var(--ink-faint)" }} /></div></div>}

      {attention.length > 0 && <section className="section"><div className="day-hdr" style={{ marginBottom: 14 }}><div><div className="eyebrow"><span className="dot" />Needs a glance · {attention.length}</div><h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Just these — the rest is fine.</h2></div></div><div className="budget-grid">{attention.map((env) => <div key={env.categoryId} data-envelope-id={env.categoryId}><EnvelopeCard env={env} editing={editingId === env.categoryId} onEdit={() => setEditingId(env.categoryId)} donor={donorFor(env.categoryId)} />{editingId === env.categoryId && <BudgetInput envelope={env} onClose={() => setEditingId(null)} />}</div>)}</div></section>}

      <section className="section">
        <div className="day-hdr" style={{ marginBottom: 14 }}><div><div className="eyebrow"><span className="dot" />All envelopes</div><h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Each one, on its own.</h2></div><div className="toolbar"><button className={sort === "group" ? "on" : ""} type="button" onClick={() => setSort("group")}>By group</button><button className={sort === "stress" ? "on" : ""} type="button" onClick={() => setSort("stress")}>By stress</button><button className={sort === "size" ? "on" : ""} type="button" onClick={() => setSort("size")}>By size</button><button className={sort === "activity" ? "on" : ""} type="button" onClick={() => setSort("activity")}>By activity</button></div></div>
        {sorted.length === 0 ? <EmptyState title="No envelopes yet" description="Import transactions or set a budget to see the month take shape." /> : <div style={{ display: "flex", flexDirection: "column", gap: 28 }}>{grouped.map(([label, items]) => {
          const groupSpent = items.reduce((sum, env) => sum + env.spentCents, 0);
          const groupBudget = items.reduce((sum, env) => sum + env.budgetCents, 0);
          return (
            <div key={label}>
              <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", marginBottom: 12 }}>
                <div className="eyebrow">{label}</div>
                {sort === "group" && <span className="muted mono" style={{ fontSize: 12.5 }}>{money(groupSpent)} / {money(groupBudget)}</span>}
              </div>
              <div className="budget-grid">{items.map((env) => <div key={env.categoryId} data-envelope-id={env.categoryId}><EnvelopeCard env={env} editing={editingId === env.categoryId} onEdit={() => setEditingId(env.categoryId)} donor={donorFor(env.categoryId)} />{editingId === env.categoryId && <BudgetInput envelope={env} onClose={() => setEditingId(null)} />}</div>)}</div>
            </div>
          );
        })}</div>}
      </section>

      {unbudgeted.length > 0 && (
        <section className="section">
          <div className="day-hdr" style={{ marginBottom: 14 }}>
            <div>
              <div className="eyebrow"><span className="dot" />Not yet budgeted · {unbudgeted.length}</div>
              <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>These don't have a plan yet.</h2>
            </div>
          </div>
          <div className="budget-grid">
            {unbudgeted.map((env) => (
              <div key={env.categoryId} data-envelope-id={env.categoryId} className="card tight" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 10 }}>
                <div className="row row-sm" style={{ alignItems: "center" }}>
                  <span className="cswatch" style={{ background: env.categoryColor || "var(--accent)" }} />
                  <strong>{env.categoryLabel}</strong>
                </div>
                {editingId === env.categoryId ? (
                  <BudgetInput envelope={env} onClose={() => setEditingId(null)} />
                ) : (
                  <button className="btn outline sm" type="button" onClick={() => setEditingId(env.categoryId)}>Set budget</button>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      {history.length > 0 && (
        <section className="section">
          <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot" />Spending history · last 5 months</div>
          <div className="card flush">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Category</th>
                  {history[0]?.monthly.map((m) => <th key={m.month} className="right">{m.label}</th>)}
                  <th className="right">Your typical</th>
                </tr>
              </thead>
              <tbody>
                {history.map((row) => {
                  const typicalCents = Math.round(
                    row.monthly.reduce((sum, m) => sum + m.spentCents, 0) / Math.max(1, row.monthly.length),
                  );
                  return (
                    <tr key={row.categoryId}>
                      <td><span className="cswatch" style={{ background: row.color || "var(--accent)" }} /> {row.label}</td>
                      {row.monthly.map((m) => {
                        const over = m.budgetedCents > 0 && m.spentCents > m.budgetedCents;
                        return (
                          <td key={m.month} className="right">
                            <span className={`money ${over ? "neg" : ""}`}>{money(m.spentCents)}</span>
                            {m.budgetedCents > 0 && <span className="muted" style={{ fontSize: 11, display: "block" }}>of {money(m.budgetedCents)}</span>}
                          </td>
                        );
                      })}
                      <td className="right"><span className="money muted">{money(typicalCents)}</span></td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>
      )}

      {showPlan && <PlanNextMonthModal onClose={() => setShowPlan(false)} />}
    </div>
  );
}

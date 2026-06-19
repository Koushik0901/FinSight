import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { useBudgetEnvelopes, useBudgetHistory, useSetBudget } from "../api/hooks/budget";
import { useMonthTotals } from "../api/hooks";
import { commands } from "../api/client";
import type { BudgetEnvelope, SpendingBreakdown } from "../api/client";
import * as I from "../components/Icons";
import PlanNextMonthModal from "./PlanNextMonthModal";
import { CopilotNudge } from "../components/CopilotNudge";
import Card from "../components/Card";
import ProgressBar from "../components/ProgressBar";
import Badge from "../components/Badge";
import Button from "../components/Button";
import EmptyState from "../components/EmptyState";
import Table, { TableHead, TableBody, TableHeader, TableCell } from "../components/Table";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

type SortKey = "group" | "stress" | "size" | "activity";

const SPENDING_TARGETS = [
  { key: "fixed", label: "Fixed", rangeLabel: "50–60%", color: "var(--ink-mute)" },
  { key: "investments", label: "Investments", rangeLabel: "10%+", color: "var(--accent)" },
  { key: "savings", label: "Savings", rangeLabel: "5–10%", color: "#34D399" },
  { key: "guiltFree", label: "Guilt-free", rangeLabel: "20–35%", color: "#FB923C" },
] as const;

function inRange(key: typeof SPENDING_TARGETS[number]["key"], pct: number) {
  if (key === "fixed") return pct >= 50 && pct <= 60;
  if (key === "investments") return pct >= 10;
  if (key === "savings") return pct >= 5 && pct <= 10;
  return pct >= 20 && pct <= 35;
}

function ConsciousSpendingSplit({ breakdown }: { breakdown: SpendingBreakdown }) {
  const totalSpending =
    breakdown.fixedCents +
    breakdown.investmentsCents +
    breakdown.savingsCents +
    breakdown.guiltFreeCents +
    breakdown.untaggedCents;
  const taggedSpending =
    breakdown.fixedCents +
    breakdown.investmentsCents +
    breakdown.savingsCents +
    breakdown.guiltFreeCents;

  if (breakdown.totalIncomeCents <= 0 && totalSpending <= 0) return null;

  if (taggedSpending <= 0) {
    return (
      <Card style={{ marginBottom: 20 }}>
        <div className="card-head" style={{ paddingBottom: 12 }}>
          <div>
            <div className="eyebrow">Conscious Spending Split</div>
            <div className="h3" style={{ marginTop: 6 }}>Tag your categories with spending types to see your allocation breakdown</div>
          </div>
        </div>
        <div className="muted" style={{ fontSize: 13.5 }}>
          Start with Fixed, Investments, Savings, and Guilt-free categories to compare your real-life spending against the framework.
        </div>
      </Card>
    );
  }

  const values = {
    fixed: breakdown.fixedCents,
    investments: breakdown.investmentsCents,
    savings: breakdown.savingsCents,
    guiltFree: breakdown.guiltFreeCents,
  } as const;

  return (
    <Card style={{ marginBottom: 20 }}>
      <div className="card-head" style={{ paddingBottom: 12 }}>
        <div>
          <div className="eyebrow">Conscious Spending Split</div>
          <div className="h3" style={{ marginTop: 6 }}>How this month's spending is allocated</div>
        </div>
        <div className="muted" style={{ fontSize: 12.5 }}>
          <span className="money">{fmt(totalSpending)}</span> spent
          {breakdown.totalIncomeCents > 0 && (
            <>
              {" "}· <span className="money">{fmt(breakdown.totalIncomeCents)}</span> income
            </>
          )}
        </div>
      </div>

      <div className="stream" style={{ height: 14 }}>
        {SPENDING_TARGETS.map((segment) => {
          const cents = values[segment.key];
          return (
            <span
              key={segment.key}
              title={`${segment.label} · ${Math.round(totalSpending > 0 ? (cents / totalSpending) * 100 : 0)}%`}
              style={{
                width: totalSpending > 0 ? `${(cents / totalSpending) * 100}%` : "0%",
                background: segment.color,
              }}
            />
          );
        })}
      </div>

      <div className="responsive-grid" style={{ gridTemplateColumns: "repeat(4, minmax(0, 1fr))", marginTop: 14 }}>
        {SPENDING_TARGETS.map((segment) => {
          const cents = values[segment.key];
          const pct = totalSpending > 0 ? Math.round((cents / totalSpending) * 100) : 0;
          const ok = inRange(segment.key, pct);
          return (
            <div key={segment.key} style={{ minWidth: 0 }}>
              <div className="row row-sm" style={{ marginBottom: 6 }}>
                <span className="dot" style={{ width: 8, height: 8, background: segment.color }} />
                <span style={{ fontSize: 13.5, fontWeight: 600 }}>{segment.label}</span>
              </div>
              <div className="row row-sm wrap">
                <Badge tone={ok ? "positive" : "negative"}>{pct}%</Badge>
                <span className="muted" style={{ fontSize: 11.5 }}>{segment.rangeLabel}</span>
              </div>
              <div className="money muted" style={{ fontSize: 12, marginTop: 6 }}>{fmt(cents)}</div>
            </div>
          );
        })}
      </div>

      {breakdown.untaggedCents > 0 && (
        <div style={{ marginTop: 12, fontSize: 12.5, color: "var(--ink-faint)" }}>
          Untagged spending still needs sorting: <span className="money">{fmt(breakdown.untaggedCents)}</span>
        </div>
      )}
    </Card>
  );
}

function envelopeStatus(e: BudgetEnvelope) {
  if (e.budgetCents === 0) return { label: "No budget set", tone: "neutral" as const, severity: 0 };
  const pct = (e.spentCents / e.budgetCents) * 100;
  if (e.spentCents > e.budgetCents) return { label: `Over by ${fmt(e.spentCents - e.budgetCents)}`, tone: "negative" as const, severity: 3 };
  if (pct > 90) return { label: "Tight", tone: "warning" as const, severity: 2 };
  if (pct > 60) return { label: "On pace", tone: "neutral" as const, severity: 1 };
  return { label: "Plenty left", tone: "positive" as const, severity: 0 };
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
    <div className="budget-inline-edit row row-sm">
      <span style={{ color: "var(--ink-mute)", fontSize: 13 }}>$</span>
      <input
        type="number"
        min="0"
        step="10"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") void save(); if (e.key === "Escape") onClose(); }}
        autoFocus
        aria-label={`Budget amount for ${envelope.categoryLabel}`}
      />
      <Button size="sm" variant="primary" onClick={() => void save()}>Save</Button>
      <Button size="sm" variant="ghost" onClick={onClose} aria-label="Cancel budget edit">✕</Button>
    </div>
  );
}

function EnvelopeCard({ env, onEdit }: { env: BudgetEnvelope; onEdit: () => void }) {
  const status = envelopeStatus(env);
  const pct = env.budgetCents > 0 ? Math.min(100, (env.spentCents / env.budgetCents) * 100) : 0;
  const remaining = env.budgetCents - env.spentCents;
  const color = env.categoryColor || "var(--ink-mute)";

  const cardTone = status.tone === "negative" ? "warn" : status.tone === "warning" ? "warn" : "default";

  return (
    <Card
      tight
      tone={cardTone}
      style={{
        borderColor: status.tone === "negative" ? "var(--negative)" : status.tone === "warning" ? "var(--warning)" : undefined,
        cursor: "pointer",
      }}
      onClick={onEdit}
    >
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 10 }}>
        <div className="row row-sm">
          <span className="swatch" style={{ background: color }} />
          <span style={{ fontSize: 13.5, fontWeight: 500 }}>{env.categoryLabel}</span>
        </div>
        <Badge tone={status.tone === "neutral" ? "default" : status.tone}>{status.label}</Badge>
      </div>

                <div style={{ marginBottom: 10 }}>
                  <ProgressBar
                    value={pct}
                    max={100}
                    size="sm"
                    tone={status.tone === "negative" ? "negative" : status.tone === "warning" ? "warning" : "default"}
                    aria-label={`${env.categoryLabel} budget progress`}
                  />
                </div>

      <div className="row" style={{ justifyContent: "space-between", fontSize: 12.5 }}>
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
    </Card>
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
  const todayDay = now.getDate();
  const totalDays = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
  const monthPct = (todayDay / totalDays) * 100;
  const monthLabel = now.toLocaleString("default", { month: "long", year: "numeric" });

  const totalBudget = envelopes.reduce((s, e) => s + e.budgetCents, 0);
  const totalSpent = envelopes.reduce((s, e) => s + e.spentCents, 0);
  const projectedEom = todayDay > 0 ? Math.round((totalSpent / todayDay) * totalDays) : 0;

  const sorted = [...envelopes].sort((a, b) => {
    if (sort === "stress")   return envelopeStatus(b).severity - envelopeStatus(a).severity || b.spentCents - a.spentCents;
    if (sort === "size")     return b.budgetCents - a.budgetCents;
    if (sort === "activity") return b.txnCount - a.txnCount;
    return (a.groupLabel || "").localeCompare(b.groupLabel || "") || a.categoryLabel.localeCompare(b.categoryLabel);
  });

  const totalBudgetSet = envelopes.reduce((s, e) => s + e.budgetCents, 0);
  const toBudget = (totals?.incomeCents ?? 0) - totalBudgetSet;

  const attention = sorted.filter((e) => envelopeStatus(e).severity >= 2);

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        <span className="spinner" aria-hidden="true" />
        <span style={{ marginTop: 12 }}>Loading budget…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="stub" role="alert" aria-live="assertive">
        Error loading budget.
      </div>
    );
  }

  const noData = envelopes.length === 0;

  return (
    <div className="screen screen-budget">
      {/* Header */}
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Budget · {monthLabel} · day {todayDay} of {totalDays}
          </div>
          <h1>Where the plan stands today.</h1>
        </div>
        <div className="toolbar">
          <button className={sort === "group" ? "on" : ""} onClick={() => setSort("group")} aria-pressed={sort === "group"}>By group</button>
          <button className={sort === "stress" ? "on" : ""} onClick={() => setSort("stress")} aria-pressed={sort === "stress"}>By stress</button>
          <button className={sort === "size" ? "on" : ""} onClick={() => setSort("size")} aria-pressed={sort === "size"}>By size</button>
          <button className={sort === "activity" ? "on" : ""} onClick={() => setSort("activity")} aria-pressed={sort === "activity"}>By activity</button>
          <Button onClick={() => setShowPlan(true)}>Plan next month →</Button>
        </div>
      </header>

      {breakdown && <ConsciousSpendingSplit breakdown={breakdown} />}

      {/* To Budget tracker */}
      {totals && (
        <Card style={{ marginBottom: 20, padding: "10px 16px" }}>
          <div className="row row-md" style={{ fontSize: 13 }}>
            <span className="dot" style={{ background: "var(--accent)", boxShadow: "0 0 6px var(--accent)" }} />
            <span className="muted">To Budget · unassigned</span>
            <span className="num money" style={{ fontSize: 18, fontWeight: 600, color: toBudget >= 0 ? "var(--accent)" : "var(--negative)" }}>
              {fmt(Math.abs(toBudget))}
              {toBudget < 0 ? " over" : ""}
            </span>
            <span className="muted" style={{ marginLeft: "auto", fontSize: 12 }}>
              of {fmt(totals.incomeCents)} income · {fmt(totalBudgetSet)} assigned
            </span>
            {attention.length > 0 && (
              <CopilotNudge
                prompt={`I have ${attention.length} budget ${attention.length === 1 ? "category" : "categories"} over limit (${attention.map((e) => e.categoryLabel).join(", ")}). How should I fix this?`}
                label={`Fix ${attention.length} overage${attention.length !== 1 ? "s" : ""}`}
                variant="warning"
                count={attention.length}
              />
            )}
          </div>
        </Card>
      )}

      {noData ? (
        <EmptyState
          icon={<I.Lego style={{ color: "var(--ink-faint)", width: 32, height: 32, margin: "0 auto" }} />}
          title="No envelopes yet"
          description="Import transactions first, then click any category card below to set a monthly budget."
        />
      ) : (
        <>
          {/* Month progress card */}
          <Card tone="accent">
            <div className="responsive-grid" style={{ gridTemplateColumns: "1fr 1fr 1fr", gap: 28, alignItems: "start" }}>
              <div>
                <div className="eyebrow" style={{ marginBottom: 10 }}>Left to spend</div>
                <div className="figure money" style={{ fontSize: 44, lineHeight: 1, color: totalBudget > 0 && totalSpent > totalBudget ? "var(--negative)" : "var(--accent)" }}>
                  {fmt(Math.max(0, totalBudget - totalSpent))}
                </div>
                <div style={{ position: "relative", height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginBottom: 4 }}>
                  <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: monthPct + "%", background: "var(--ink-faint)", opacity: 0.25, borderRadius: 999 }} />
                  {totalBudget > 0 && (
                    <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: Math.min(100, (totalSpent / totalBudget) * 100) + "%", background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} />
                  )}
                </div>
                <div className="row" style={{ justifyContent: "space-between", marginTop: 6, fontSize: 11.5, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
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
                      <Badge tone="positive">{fmt(totalBudget - projectedEom)} under plan</Badge>
                    ) : (
                      <Badge tone="negative">{fmt(projectedEom - totalBudget)} over plan</Badge>
                    )}
                  </div>
                )}
              </div>
            </div>
          </Card>

          {/* Needs attention */}
          {attention.length > 0 && (
            <section className="section">
              <div className="eyebrow" style={{ marginBottom: 12 }}>
                <span className="dot" style={{ background: "var(--negative)", boxShadow: "0 0 6px var(--negative)" }} />
                Needs a glance · {attention.length}
              </div>
              <div className="responsive-grid" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 12 }}>
                {attention.map((e) =>
                  editingId === e.categoryId ? (
                    <Card key={e.categoryId} tight style={{ padding: 18 }}>
                      <div style={{ fontSize: 13.5, fontWeight: 500, marginBottom: 10 }}>{e.categoryLabel}</div>
                      <BudgetInput envelope={e} onClose={() => setEditingId(null)} />
                    </Card>
                  ) : (
                    <EnvelopeCard key={e.categoryId} env={e} onEdit={() => setEditingId(e.categoryId)} />
                  )
                )}
              </div>
            </section>
          )}

          {/* All envelopes */}
          <section className="section">
            <div className="eyebrow" style={{ marginBottom: 12 }}>
              All envelopes · {sorted.length}
            </div>
            <div className="responsive-grid" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 12 }}>
              {sorted.map((e) =>
                editingId === e.categoryId ? (
                  <Card key={e.categoryId} tight style={{ padding: 18 }}>
                    <div style={{ fontSize: 13.5, fontWeight: 500, marginBottom: 10 }}>{e.categoryLabel}</div>
                    <BudgetInput envelope={e} onClose={() => setEditingId(null)} />
                  </Card>
                ) : (
                  <EnvelopeCard key={e.categoryId} env={e} onEdit={() => setEditingId(e.categoryId)} />
                )
              )}
            </div>
          </section>
        </>
      )}

      {history.length > 0 && (
        <section className="section">
          <div className="eyebrow" style={{ marginBottom: 12 }}>Spending history · last 5 months</div>
          <Card flush>
            <Table wrap={false}>
              <TableHead>
                <tr>
                  <TableHeader>Category</TableHeader>
                  {history[0]!.monthly.map(m => (
                    <TableHeader key={m.month} right scope="col">
                      {m.month.slice(5)}
                    </TableHeader>
                  ))}
                </tr>
              </TableHead>
              <TableBody>
                {history.map(cat => (
                  <tr key={cat.categoryId}>
                    <TableCell>
                      <span className="swatch" style={{ background: cat.color || "var(--ink-faint)" }} />
                      {cat.label}
                    </TableCell>
                    {cat.monthly.map(m => (
                      <TableCell key={m.month} right>
                        <span className="num money">{m.cents > 0 ? fmt(m.cents) : "—"}</span>
                      </TableCell>
                    ))}
                  </tr>
                ))}
              </TableBody>
            </Table>
          </Card>
        </section>
      )}

      {showPlan && <PlanNextMonthModal onClose={() => setShowPlan(false)} />}
    </div>
  );
}

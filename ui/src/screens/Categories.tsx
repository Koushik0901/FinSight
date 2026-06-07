import { useState } from "react";
import type { ReactNode } from "react";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import type { CategoryWithSpending } from "../api/client";
import { money } from "../utils/format";

function PaceBar({ value, compare, color }: { value: number; compare: number; color: string }) {
  const max = Math.max(value, compare, 1);
  const pct = Math.min(120, (value / max) * 100);
  const over = value > compare && compare > 0;
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <div style={{ flex: 1, height: 6, background: "var(--hairline)", borderRadius: 999, overflow: "hidden", maxWidth: 180 }}>
        <div style={{ width: pct + "%", height: "100%", background: over ? "var(--negative)" : color, borderRadius: 999 }} />
      </div>
      <span className="num tabular" style={{ fontSize: 12, color: over ? "var(--negative)" : "var(--ink-faint)", minWidth: 32 }}>
        {Math.round(pct)}%
      </span>
    </div>
  );
}

export default function Categories() {
  const [scope, setScope] = useState<"month" | "avg" | "year">("month");
  const { data: cats = [], isLoading, error } = useCategoriesWithSpending();

  // Filter to non-zero categories and sort by spend desc
  const active = cats
    .filter((c) =>
      c.thisMonthCents > 0 ||
      c.lastMonthCents > 0 ||
      (scope === "year" && c.yearTotalCents > 0)
    )
    .sort((a, b) => b.thisMonthCents - a.thisMonthCents);

  const valueFor = (c: CategoryWithSpending) => {
    if (scope === "avg") return Math.round((c.thisMonthCents + c.lastMonthCents) / 2);
    if (scope === "year") return c.yearTotalCents;
    return c.thisMonthCents;
  };
  const compareFor = (c: CategoryWithSpending) =>
    scope === "avg" ? c.thisMonthCents : c.lastMonthCents;

  const totalThis = active.reduce((s, c) => s + valueFor(c), 0);
  const totalLast = active.reduce((s, c) => s + compareFor(c), 0);

  const delta = totalLast > 0 ? ((totalThis - totalLast) / totalLast) * 100 : 0;

  const now = new Date();
  const monthLabel = now.toLocaleString("default", { month: "long", year: "numeric" });
  const lastMonthLabel = new Date(now.getFullYear(), now.getMonth() - 1, 1)
    .toLocaleString("default", { month: "long" });

  // §6c: AI insight sentence
  const hasLastMonthData = active.some((c) => c.lastMonthCents > 0);
  let insightJSX: ReactNode = null;
  if (scope === "month" && hasLastMonthData && active.length >= 2) {
    const withDelta = active.map((c) => ({ ...c, delta: c.thisMonthCents - c.lastMonthCents }));
    const topGainer = withDelta.reduce((best, c) => c.delta < best.delta ? c : best);
    const topRiser  = withDelta.reduce((best, c) => c.delta > best.delta ? c : best);
    if (topGainer.delta < 0 && topRiser.delta > 0) {
      insightJSX = (
        <div className="muted" style={{ fontSize: 13, fontStyle: "italic", marginBottom: 12 }}>
          ✦ <strong>{topGainer.label}</strong> dropped {money(Math.abs(topGainer.delta))} — biggest improvement.{" "}
          <strong>{topRiser.label}</strong> rose by {money(topRiser.delta)}.
        </div>
      );
    }
  }

  if (isLoading) return <div className="stub">Loading categories…</div>;
  if (error)     return <div className="stub">Error loading categories.</div>;
  if (active.length === 0) return <div className="stub">No spending data yet. Import some transactions to see categories here.</div>;

  return (
    <div className="screen">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            Categories · {scope === "avg" ? "trailing average" : monthLabel}
          </div>
          <h1>Where the money is going.</h1>
        </div>
        <div className="toolbar">
          <button className={scope === "month" ? "on" : ""} onClick={() => setScope("month")}>
            This month
          </button>
          <button className={scope === "avg" ? "on" : ""} onClick={() => setScope("avg")}>
            vs. last month
          </button>
          <button className={scope === "year" ? "on" : ""} onClick={() => setScope("year")}>
            Year to date
          </button>
        </div>
      </div>

      {insightJSX}

      {/* Summary card */}
      <div className="card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 14 }}>
          <div>
            <div className="eyebrow" style={{ marginBottom: 6 }}>
              {scope === "avg" ? "Average" : "Total spent"}
            </div>
            <div className="figure money" style={{ fontSize: 44, lineHeight: 1 }}>
              {money(totalThis)}
            </div>
          </div>
          {scope !== "year" && totalLast > 0 && (
            <div style={{ textAlign: "right" }}>
              <div className="muted" style={{ fontSize: 13 }}>vs. {lastMonthLabel}</div>
              <div
                className={`num ${totalThis < totalLast ? "pos" : "neg"}`}
                style={{ fontSize: 18 }}
              >
                {totalThis < totalLast ? "↓" : "↑"}{" "}
                {money(Math.abs(totalLast - totalThis))} · {Math.abs(Math.round(delta))}%
              </div>
            </div>
          )}
        </div>

        {/* Category stream bar */}
        <div className="stream" style={{ height: 18, borderRadius: 6 }}>
          {active.map((c) => (
            <span
              key={c.id}
              title={`${c.label} · ${money(valueFor(c))}`}
              style={{
                width: totalThis > 0 ? `${(valueFor(c) / totalThis) * 100}%` : "0%",
                background: c.color || "var(--ink-faint)",
              }}
            />
          ))}
        </div>
      </div>

      {/* Full table */}
      <div className="section">
        <div className="card flush">
          <div className="card-head">
            <div className="h3">All categories</div>
            <div className="muted" style={{ fontSize: 13 }}>
              Sorted by spend · {active.length} active
            </div>
          </div>
          <table className="tbl">
            <thead>
              <tr>
                <th style={{ width: "32%" }}>Category</th>
                <th>Pace vs. {lastMonthLabel}</th>
                <th className="right">{scope === "avg" ? "Average" : scope === "year" ? "Year to date" : "This month"}</th>
                <th className="right">{lastMonthLabel}</th>
                <th className="right">Transactions</th>
                <th className="right">Budget</th>
              </tr>
            </thead>
            <tbody>
              {active.map((c) => {
                const v = valueFor(c);
                const cmp = scope === "year" ? 0 : compareFor(c);
                const color = c.color || "var(--ink-mute)";
                return (
                  <tr key={c.id}>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                        <span
                          style={{
                            width: 22,
                            height: 22,
                            borderRadius: 6,
                            background: color + "22",
                            border: `1px solid ${color}44`,
                            flexShrink: 0,
                          }}
                        />
                        <span style={{ fontSize: 14 }}>{c.label}</span>
                        <span className="muted" style={{ fontSize: 12 }}>{c.groupLabel}</span>
                      </div>
                    </td>
                    <td style={{ paddingTop: 8, paddingBottom: 8 }}>
                      <PaceBar value={v} compare={cmp} color={color} />
                    </td>
                    <td className="right num tabular money">{money(v)}</td>
                    <td className="right num tabular muted">{cmp > 0 ? money(cmp) : "—"}</td>
                    <td className="right num tabular muted">{c.txnCount}</td>
                    <td className="right num tabular" style={{ color: c.budgetCents > 0 && c.thisMonthCents > c.budgetCents ? "var(--negative)" : "var(--ink-mute)" }}>
                      {c.budgetCents > 0 ? money(c.budgetCents) : "—"}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

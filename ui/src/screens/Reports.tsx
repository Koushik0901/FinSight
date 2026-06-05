import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type ReportData, type MonthSummary } from "../api/client";
import { money } from "../utils/format";

function useReportData() {
  return useQuery<ReportData>({
    queryKey: ["report-data"],
    queryFn: async () => {
      const result = await commands.getReportData();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
}

// ── Inline SVG bar chart ─────────────────────────────────────────────────

function BarChart({ months, scope }: { months: MonthSummary[]; scope: "6" | "12" }) {
  const data = scope === "6" ? months.slice(-6) : months;
  const maxVal = Math.max(...data.flatMap((m) => [m.incomeCents, m.expenseCents]), 1);
  const W = 100 / data.length;
  const barW = W * 0.35;

  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)", padding: "22px 4px 12px" }}>
      <div style={{ padding: "0 20px 16px", display: "flex", alignItems: "baseline", justifyContent: "space-between" }}>
        <div className="h3">Income vs expenses</div>
        <div style={{ display: "flex", gap: 16, fontSize: 12, color: "var(--ink-mute)" }}>
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <span style={{ width: 10, height: 10, borderRadius: 3, background: "var(--positive)", display: "inline-block" }} />Income
          </span>
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <span style={{ width: 10, height: 10, borderRadius: 3, background: "var(--negative)", display: "inline-block" }} />Expenses
          </span>
        </div>
      </div>
      <svg viewBox="0 0 100 40" preserveAspectRatio="none" style={{ width: "100%", height: 180, display: "block" }}>
        {data.map((m, i) => {
          const incH = (m.incomeCents / maxVal) * 36;
          const expH = (m.expenseCents / maxVal) * 36;
          const x = i * W + W / 2;
          return (
            <g key={m.month}>
              {/* Income bar */}
              <rect
                x={x - barW - 0.4}
                y={38 - incH}
                width={barW}
                height={incH}
                fill="var(--positive)"
                opacity={0.8}
                rx={0.5}
              />
              {/* Expense bar */}
              <rect
                x={x + 0.4}
                y={38 - expH}
                width={barW}
                height={expH}
                fill="var(--negative)"
                opacity={0.8}
                rx={0.5}
              />
            </g>
          );
        })}
      </svg>
      {/* Month labels */}
      <div style={{ display: "flex", padding: "4px 4px 0", justifyContent: "space-around" }}>
        {data.map((m) => (
          <span key={m.month} style={{ fontSize: 11, color: "var(--ink-faint)", fontFamily: "var(--mono)", textAlign: "center", width: `${100 / data.length}%` }}>
            {m.label}
          </span>
        ))}
      </div>
    </div>
  );
}

// ── Net worth line (cumulative net) ───────────────────────────────────────

function NetLine({ months }: { months: MonthSummary[] }) {
  let running = 0;
  const points = months.map((m) => {
    running += m.netCents;
    return running;
  });
  const maxAbs = Math.max(Math.abs(Math.min(...points)), Math.abs(Math.max(...points)), 1);
  const W = 100 / (points.length - 1 || 1);

  const pathD = points
    .map((v, i) => {
      const x = i * W;
      const y = 35 - ((v / maxAbs) * 30);
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  const isPositive = points[points.length - 1] ?? 0;

  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)", padding: "22px 4px 12px" }}>
      <div style={{ padding: "0 20px 16px" }}>
        <div className="h3">Cumulative net (12-month running total)</div>
        <div className={`figure num ${isPositive >= 0 ? "pos" : "neg"}`} style={{ fontSize: 24, marginTop: 4 }}>
          {money(points[points.length - 1] ?? 0)}
        </div>
      </div>
      <svg viewBox="0 0 100 40" preserveAspectRatio="none" style={{ width: "100%", height: 100, display: "block" }}>
        <line x1="0" y1="35" x2="100" y2="35" stroke="var(--hairline)" strokeWidth="0.5" />
        <path
          d={pathD}
          fill="none"
          stroke={isPositive >= 0 ? "var(--positive)" : "var(--negative)"}
          strokeWidth="1.2"
        />
        {points.map((v, i) => (
          <circle
            key={i}
            cx={(i * W).toFixed(1)}
            cy={(35 - (v / maxAbs) * 30).toFixed(1)}
            r="1"
            fill={v >= 0 ? "var(--positive)" : "var(--negative)"}
          />
        ))}
      </svg>
      <div style={{ display: "flex", padding: "4px 4px 0", justifyContent: "space-around" }}>
        {months.map((m) => (
          <span key={m.month} style={{ fontSize: 11, color: "var(--ink-faint)", fontFamily: "var(--mono)", textAlign: "center", width: `${100 / months.length}%` }}>
            {m.label}
          </span>
        ))}
      </div>
    </div>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────

export default function Reports() {
  const { data, isLoading, error } = useReportData();
  const [barScope, setBarScope] = useState<"6" | "12">("6");

  if (isLoading) return <div className="stub">Computing reports…</div>;
  if (error)     return <div className="stub">Error computing reports.</div>;
  if (!data || data.monthly.every((m) => m.incomeCents === 0 && m.expenseCents === 0)) {
    return (
      <div className="screen">
        <div className="screen-header">
          <div className="screen-header-text">
            <div className="screen-eyebrow">Reports</div>
            <h1>See the shape of your money over time.</h1>
          </div>
        </div>
        <div className="card" style={{ textAlign: "center", padding: "64px 32px" }}>
          <div style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>No data yet</div>
          <div className="muted" style={{ fontSize: 14 }}>Import transactions to see reports here.</div>
        </div>
      </div>
    );
  }

  // Aggregate KPIs from last 12 months
  const totalIncome  = data.monthly.reduce((s, m) => s + m.incomeCents, 0);
  const totalExpense = data.monthly.reduce((s, m) => s + m.expenseCents, 0);
  const netTotal     = totalIncome - totalExpense;
  const savingsRate  = totalIncome > 0 ? Math.round((netTotal / totalIncome) * 100) : 0;
  const activeMonths = data.monthly.filter((m) => m.incomeCents + m.expenseCents > 0).length || 1;
  const avgMonthlySpend = Math.round(totalExpense / activeMonths);

  return (
    <div className="screen">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">Reports · last 12 months</div>
          <h1>See the shape of your money over time.</h1>
        </div>
        <div className="toolbar">
          <button className={barScope === "6" ? "on" : ""} onClick={() => setBarScope("6")}>6M</button>
          <button className={barScope === "12" ? "on" : ""} onClick={() => setBarScope("12")}>12M</button>
        </div>
      </div>

      {/* KPI row */}
      <div className="stat-row">
        <div className="stat">
          <div className="label">12-month income</div>
          <div className="value figure money">{money(totalIncome)}</div>
          <div className="sub muted">{money(Math.round(totalIncome / activeMonths))}/mo avg</div>
        </div>
        <div className="stat">
          <div className="label">12-month expenses</div>
          <div className="value figure money">{money(totalExpense)}</div>
          <div className="sub muted">{money(avgMonthlySpend)}/mo avg</div>
        </div>
        <div className={`stat ${netTotal >= 0 ? "accent" : ""}`}>
          <div className="label">Net (12-month)</div>
          <div className={`value figure money ${netTotal >= 0 ? "" : "neg"}`}>{money(netTotal)}</div>
          <div className="sub muted">{netTotal >= 0 ? "saved" : "deficit"}</div>
        </div>
        <div className={`stat ${savingsRate > 0 ? "accent" : ""}`}>
          <div className="label">Savings rate</div>
          <div className="value figure">{savingsRate}%</div>
          <div className="sub muted">of income kept</div>
        </div>
      </div>

      {/* Charts */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 24 }}>
        <BarChart months={data.monthly} scope={barScope} />
        <NetLine months={data.monthly} />
      </div>

      {/* Tables */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 16 }}>
        {/* Top categories */}
        <div className="card flush">
          <div className="card-head">
            <div className="h3">Top categories</div>
            <div className="muted" style={{ fontSize: 12 }}>12-month spend</div>
          </div>
          <table className="tbl">
            <thead>
              <tr>
                <th>Category</th>
                <th className="right">Total</th>
                <th className="right">Txns</th>
              </tr>
            </thead>
            <tbody>
              {data.topCategories.map((c) => (
                <tr key={c.categoryId}>
                  <td>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <span style={{ width: 8, height: 8, borderRadius: 2, background: c.color || "var(--ink-faint)", display: "inline-block", flexShrink: 0 }} />
                      <span style={{ fontSize: 14 }}>{c.label}</span>
                    </div>
                  </td>
                  <td className="right num tabular money" style={{ fontSize: 13.5 }}>{money(c.totalCents)}</td>
                  <td className="right muted" style={{ fontSize: 13, fontFamily: "var(--mono)" }}>{c.txnCount}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Top merchants */}
        <div className="card flush">
          <div className="card-head">
            <div className="h3">Top merchants</div>
            <div className="muted" style={{ fontSize: 12 }}>12-month spend</div>
          </div>
          <table className="tbl">
            <thead>
              <tr>
                <th>Merchant</th>
                <th className="right">Total</th>
                <th className="right">Txns</th>
              </tr>
            </thead>
            <tbody>
              {data.topMerchants.map((m, i) => (
                <tr key={`${m.merchantRaw}-${i}`}>
                  <td>
                    <div>
                      <div style={{ fontSize: 14 }}>{m.merchantRaw}</div>
                      <div className="muted" style={{ fontSize: 12 }}>{m.categoryLabel || "Uncategorized"}</div>
                    </div>
                  </td>
                  <td className="right num tabular money" style={{ fontSize: 13.5 }}>{money(m.totalCents)}</td>
                  <td className="right muted" style={{ fontSize: 13, fontFamily: "var(--mono)" }}>{m.txnCount}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

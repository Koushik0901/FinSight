import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type ReportData, type MonthSummary } from "../api/client";
import { money } from "../utils/format";

function useReportData(scope: "month" | "quarter" | "year" | "all") {
  return useQuery<ReportData>({
    queryKey: ["report-data", scope],
    queryFn: async () => {
      const result = await commands.getReportData(scope);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
}

// ── Data model ───────────────────────────────────────────────────────────────

type WidgetVisibility = {
  barChart: boolean;
  netLine: boolean;
  donut: boolean;
  yoy: boolean;
  categories: boolean;
  merchants: boolean;
};

type ReportTab = {
  id: string;
  name: string;
  scope: "month" | "quarter" | "year" | "all";
  widgets: WidgetVisibility;
};

const DEFAULT_TAB: ReportTab = {
  id: "default",
  name: "Overview",
  scope: "year",
  widgets: { barChart: true, netLine: true, donut: true, yoy: true, categories: true, merchants: true },
};

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

// ── Donut chart ───────────────────────────────────────────────────────────

function DonutChart({ categories, totalCents }: {
  categories: { label: string; color: string; totalCents: number }[];
  totalCents: number;
}) {
  const fmtK = (cents: number) => {
    if (cents >= 100_000) return `$${(cents / 100_000).toFixed(1)}k`;
    return `$${(cents / 100).toFixed(0)}`;
  };

  const total = categories.reduce((s, c) => s + c.totalCents, 0) || 1;
  let cumAngle = -Math.PI / 2;
  const slices = categories.map(c => {
    const share = c.totalCents / total;
    const start = cumAngle;
    cumAngle += share * 2 * Math.PI * 0.99;
    const end = cumAngle;
    cumAngle += share * 2 * Math.PI * 0.01;
    return { ...c, start, end, share };
  });

  // When only one category, arc degenerates — render a filled circle instead
  if (slices.length === 1) {
    return (
      <div className="card" style={{ padding: 20 }}>
        <div className="eyebrow" style={{ marginBottom: 14 }}>Spending by category</div>
        <div style={{ display: "flex", gap: 24, alignItems: "flex-start" }}>
          <svg viewBox="0 0 100 100" width={120} height={120} style={{ flexShrink: 0 }}>
            <circle cx={50} cy={50} r={48} fill={slices[0]!.color || "var(--accent)"} />
            <circle cx={50} cy={50} r={32} fill="var(--bg)" />
            <text x={50} y={53} textAnchor="middle" fontSize={9}
              fill="var(--ink)" fontFamily="var(--mono)">
              {fmtK(totalCents)}
            </text>
          </svg>
          <div style={{ display: "flex", flexDirection: "column", gap: 6, flex: 1 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12 }}>
              <span style={{ width: 8, height: 8, borderRadius: 2,
                background: slices[0]!.color || "var(--accent)", flexShrink: 0 }} />
              <span style={{ flex: 1 }}>{slices[0]!.label}</span>
              <span className="muted" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>100%</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const arc = (cx: number, cy: number, r: number, start: number, end: number) => {
    const x1 = cx + r * Math.cos(start);
    const y1 = cy + r * Math.sin(start);
    const x2 = cx + r * Math.cos(end);
    const y2 = cy + r * Math.sin(end);
    const large = end - start > Math.PI ? 1 : 0;
    return `M ${cx} ${cy} L ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2} Z`;
  };

  return (
    <div className="card" style={{ padding: 20 }}>
      <div className="eyebrow" style={{ marginBottom: 14 }}>Spending by category</div>
      <div style={{ display: "flex", gap: 24, alignItems: "flex-start" }}>
        <svg viewBox="0 0 100 100" width={120} height={120} style={{ flexShrink: 0 }}>
          {slices.map((s, i) => (
            <path key={i} d={arc(50, 50, 48, s.start, s.end)}
              fill={s.color || "var(--ink-faint)"} />
          ))}
          <circle cx={50} cy={50} r={32} fill="var(--bg)" />
          <text x={50} y={53} textAnchor="middle" fontSize={9}
            fill="var(--ink)" fontFamily="var(--mono)">
            {fmtK(totalCents)}
          </text>
        </svg>
        <div style={{ display: "flex", flexDirection: "column", gap: 6, flex: 1 }}>
          {slices.slice(0, 8).map((s, i) => (
            <div key={i} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12 }}>
              <span style={{ width: 8, height: 8, borderRadius: 2,
                background: s.color || "var(--ink-faint)", flexShrink: 0 }} />
              <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis",
                whiteSpace: "nowrap" }}>{s.label}</span>
              <span className="muted" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                {Math.round(s.share * 100)}%
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Year-over-year line chart ─────────────────────────────────────────────

function YoYChart({ thisYear, lastYear }: {
  thisYear: { label: string; expenseCents: number }[];
  lastYear: { expenseCents: number }[];
}) {
  if (thisYear.length === 0) return null;
  const allVals = [...thisYear.map(m => m.expenseCents), ...lastYear.map(m => m.expenseCents)];
  const maxVal = Math.max(...allVals, 1);
  const W = 100, H = 50;
  const n = thisYear.length;
  const pts = (data: number[]) =>
    data.map((v, i) =>
      `${(i / Math.max(n - 1, 1)) * W},${H - (v / maxVal) * (H - 4)}`
    ).join(" ");

  // Determine which x-axis labels to show
  const labelIndices = n <= 6
    ? thisYear.map((_, i) => i)
    : thisYear.map((_, i) => i).filter(i => i % Math.ceil(n / 6) === 0);

  return (
    <div className="card" style={{ padding: 20 }}>
      <div className="eyebrow" style={{ marginBottom: 14 }}>Year-over-year expenses</div>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none"
        style={{ width: "100%", height: 100, display: "block" }}>
        {lastYear.length > 0 && (
          <polyline points={pts(lastYear.map(m => m.expenseCents))}
            fill="none" stroke="var(--ink-mute)" strokeWidth={1}
            strokeDasharray="2 1" />
        )}
        <polyline points={pts(thisYear.map(m => m.expenseCents))}
          fill="none" stroke="var(--accent)" strokeWidth={1.5} />
      </svg>
      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 4 }}>
        {labelIndices.map(i => (
          <span key={i} className="muted" style={{ fontSize: 10, fontFamily: "var(--mono)" }}>
            {thisYear[i]!.label}
          </span>
        ))}
      </div>
      <div style={{ display: "flex", gap: 16, marginTop: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}>
          <span style={{ width: 16, height: 2, background: "var(--accent)", display: "inline-block" }} />
          This year
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}>
          <span style={{ width: 16, height: 2, borderBottom: "2px dashed var(--ink-mute)", display: "inline-block" }} />
          Last year
        </div>
      </div>
    </div>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────

export default function Reports() {
  const [tabs, setTabs] = useState<ReportTab[]>(() => {
    try {
      const saved = localStorage.getItem("report_tabs");
      if (saved) {
        const parsed = JSON.parse(saved) as ReportTab[];
        if (parsed.length > 0) return parsed;
      }
    } catch {}
    return [DEFAULT_TAB];
  });
  const [activeTabId, setActiveTabId] = useState<string>("default");
  const [customize, setCustomize] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);

  const activeTab = tabs.find(t => t.id === activeTabId) ?? tabs[0]!;
  const scope = activeTab.scope;
  const barScope: "6" | "12" = scope === "month" || scope === "quarter" ? "6" : "12";

  useEffect(() => {
    localStorage.setItem("report_tabs", JSON.stringify(tabs));
  }, [tabs]);

  const updateActiveTab = (patch: Partial<ReportTab>) => {
    setTabs(prev => prev.map(t => t.id === activeTabId ? { ...t, ...patch } : t));
  };

  const { data, isLoading, error } = useReportData(scope);

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

  // Scope-aware label prefix for KPI stats
  const kpiPrefix =
    scope === "month" ? "This month's" :
    scope === "quarter" ? "Quarter" :
    scope === "year" ? "12-month" :
    "All-time";

  // Aggregate KPIs from selected scope
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
          <div className="screen-eyebrow">{
            scope === "month" ? "Reports · this month" :
            scope === "quarter" ? "Reports · last quarter" :
            scope === "year" ? "Reports · this year" :
            "Reports · all time"
          }</div>
          <h1>See the shape of your money over time.</h1>
        </div>
      </div>

      {/* Tab strip */}
      <div style={{ display: "flex", alignItems: "center", gap: 4, marginBottom: 12, borderBottom: "1px solid var(--line)", paddingBottom: 8 }}>
        {tabs.map(tab => (
          <div
            key={tab.id}
            style={{
              position: "relative",
              display: "flex",
              alignItems: "center",
              gap: 4,
            }}
          >
            {renamingId === tab.id ? (
              <input
                autoFocus
                defaultValue={tab.name}
                style={{
                  padding: "2px 8px",
                  background: "var(--surface-2)",
                  border: "1px solid var(--accent)",
                  borderRadius: 4,
                  color: "var(--ink)",
                  fontSize: 13,
                  width: 120,
                }}
                onBlur={e => {
                  const name = e.target.value.trim() || tab.name;
                  setTabs(prev => prev.map(t => t.id === tab.id ? { ...t, name } : t));
                  setRenamingId(null);
                }}
                onKeyDown={e => {
                  if (e.key === "Enter") e.currentTarget.blur();
                  if (e.key === "Escape") setRenamingId(null);
                }}
              />
            ) : (
              <button
                className={`btn ghost sm${activeTabId === tab.id ? " active" : ""}`}
                style={{ fontWeight: activeTabId === tab.id ? 600 : undefined }}
                onClick={() => setActiveTabId(tab.id)}
                onDoubleClick={() => setRenamingId(tab.id)}
                title="Double-click to rename"
              >
                {tab.name}
              </button>
            )}
            {/* Delete button — only show on non-default tabs when active */}
            {tab.id !== "default" && activeTabId === tab.id && (
              <button
                className="btn ghost sm"
                style={{ padding: "0 4px", color: "var(--ink-mute)", fontSize: 11 }}
                onClick={() => {
                  setTabs(prev => prev.filter(t => t.id !== tab.id));
                  setActiveTabId("default");
                }}
                title="Delete tab"
              >
                ✕
              </button>
            )}
          </div>
        ))}

        {/* Add new tab */}
        <button
          className="btn ghost sm"
          style={{ color: "var(--ink-mute)" }}
          onClick={() => {
            const newTab: ReportTab = {
              id: crypto.randomUUID(),
              name: "New tab",
              scope: activeTab.scope,
              widgets: { ...activeTab.widgets },
            };
            setTabs(prev => [...prev, newTab]);
            setActiveTabId(newTab.id);
            setRenamingId(newTab.id);
          }}
        >
          + New tab
        </button>

        {/* Customize button — right side */}
        <div style={{ marginLeft: "auto" }}>
          <button
            className={`btn ghost sm${customize ? " active" : ""}`}
            style={customize ? { color: "var(--accent)" } : undefined}
            onClick={() => setCustomize(c => !c)}
            title="Show/hide widgets"
          >
            ✎ Customize
          </button>
        </div>
      </div>

      {/* Customize panel */}
      {customize && (
        <div className="card" style={{ padding: "12px 16px", marginBottom: 16, display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
          <span className="eyebrow" style={{ marginRight: 4 }}>Widgets</span>
          {(
            [
              ["barChart", "Bar chart"],
              ["netLine", "Net line"],
              ["donut", "Donut"],
              ["yoy", "Year-over-year"],
              ["categories", "Categories"],
              ["merchants", "Merchants"],
            ] as [keyof WidgetVisibility, string][]
          ).map(([key, label]) => (
            <button
              key={key}
              className={`btn ghost sm${activeTab.widgets[key] ? " active" : ""}`}
              style={activeTab.widgets[key] ? { color: "var(--accent)" } : { color: "var(--ink-mute)" }}
              onClick={() =>
                updateActiveTab({
                  widgets: { ...activeTab.widgets, [key]: !activeTab.widgets[key] },
                })
              }
            >
              {activeTab.widgets[key] ? "✓ " : ""}{label}
            </button>
          ))}
        </div>
      )}

      {/* Scope toolbar */}
      <div className="toolbar" style={{ marginBottom: 20 }}>
        {(["month", "quarter", "year", "all"] as const).map(s => (
          <button
            key={s}
            className={`btn ghost sm${activeTab.scope === s ? " active" : ""}`}
            onClick={() => updateActiveTab({ scope: s })}
          >
            {s === "month" ? "Month" : s === "quarter" ? "Quarter" : s === "year" ? "Year" : "All time"}
          </button>
        ))}
      </div>

      {/* KPI row */}
      <div className="stat-row">
        <div className="stat">
          <div className="label">{kpiPrefix} income</div>
          <div className="value figure money">{money(totalIncome)}</div>
          <div className="sub muted">{money(Math.round(totalIncome / activeMonths))}/mo avg</div>
        </div>
        <div className="stat">
          <div className="label">{kpiPrefix} expenses</div>
          <div className="value figure money">{money(totalExpense)}</div>
          <div className="sub muted">{money(avgMonthlySpend)}/mo avg</div>
        </div>
        <div className={`stat ${netTotal >= 0 ? "accent" : ""}`}>
          <div className="label">Net ({kpiPrefix.toLowerCase()})</div>
          <div className={`value figure money ${netTotal >= 0 ? "" : "neg"}`}>{money(netTotal)}</div>
          <div className="sub muted">{netTotal >= 0 ? "saved" : "deficit"}</div>
        </div>
        <div className={`stat ${savingsRate > 0 ? "accent" : ""}`}>
          <div className="label">Savings rate</div>
          <div className="value figure">{savingsRate}%</div>
          <div className="sub muted">of income kept</div>
        </div>
      </div>

      {/* Chart grid 1 */}
      {(activeTab.widgets.barChart || activeTab.widgets.netLine) && (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 24 }}>
          {activeTab.widgets.barChart && <BarChart months={data.monthly} scope={barScope} />}
          {activeTab.widgets.netLine && <NetLine months={data.monthly} />}
        </div>
      )}

      {/* Chart grid 2 */}
      {(activeTab.widgets.donut || activeTab.widgets.yoy) && (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 16 }}>
          {activeTab.widgets.donut && (
            <DonutChart
              categories={data.topCategories}
              totalCents={data.topCategories.reduce((s, c) => s + c.totalCents, 0)}
            />
          )}
          {activeTab.widgets.yoy && (
            <YoYChart thisYear={data.monthly} lastYear={data.monthlyLastYear} />
          )}
        </div>
      )}

      {/* Tables grid */}
      {(activeTab.widgets.categories || activeTab.widgets.merchants) && (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 16 }}>
          {/* Top categories */}
          {activeTab.widgets.categories && (
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
          )}

          {/* Top merchants */}
          {activeTab.widgets.merchants && (
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
          )}
        </div>
      )}
    </div>
  );
}

import { useState, useEffect, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { ResponsiveBar } from "@nivo/bar";
import { ResponsiveLine } from "@nivo/line";
import { ResponsivePie } from "@nivo/pie";
import { commands, type ReportData, type MonthSummary } from "../api/client";
import { money } from "../utils/format";
import { CopilotNudge } from "../components/CopilotNudge";
import Button from "../components/Button";
import Card from "../components/Card";
import Table, { TableHead, TableBody, TableRow, TableHeader, TableCell } from "../components/Table";

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

const NIVO_THEME = {
  axis: {
    ticks: { text: { fill: "var(--ink-faint)", fontSize: 11, fontFamily: "var(--mono)" } },
    domain: { line: { stroke: "var(--hairline)" } },
  },
  grid: { line: { stroke: "var(--hairline)", strokeDasharray: "4 4" } },
  legends: { text: { fill: "var(--ink-mute)", fontSize: 12, fontFamily: "var(--sans)" } },
  labels: { text: { fill: "var(--ink)", fontSize: 11, fontFamily: "var(--sans)" } },
  tooltip: {
    container: {
      background: "var(--elevated)",
      color: "var(--ink)",
      border: "1px solid var(--line-2)",
      borderRadius: "var(--radius-sm)",
      fontSize: 12,
      fontFamily: "var(--sans)",
    },
  },
};

// ── Helpers ─────────────────────────────────────────────────────────────────

function fmtK(cents: number) {
  if (cents >= 100_000) return `$${(cents / 100_000).toFixed(1)}k`;
  return `$${(cents / 100).toFixed(0)}`;
}

const srOnlyStyle: React.CSSProperties = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0, 0, 0, 0)",
  whiteSpace: "nowrap",
  border: 0,
};

// ── Nivo bar chart ─────────────────────────────────────────────────────────

function BarChart({ months, scope }: { months: MonthSummary[]; scope: "6" | "12" }) {
  const data = useMemo(() => {
    const slice = scope === "6" ? months.slice(-6) : months;
    return slice.map((m) => ({
      month: m.label,
      Income: Math.round(m.incomeCents / 100),
      Expenses: Math.round(m.expenseCents / 100),
    }));
  }, [months, scope]);

  return (
    <Card className="stack stack-sm" style={{ padding: "22px 4px 12px" }}>
      <div className="row-md" style={{ justifyContent: "space-between", alignItems: "baseline", padding: "0 20px 4px" }}>
        <div className="h3">Income vs expenses</div>
      </div>
      <div style={{ height: 180 }}>
        <ResponsiveBar
          data={data}
          keys={["Income", "Expenses"]}
          indexBy="month"
          colors={["var(--positive)", "var(--negative)"]}
          theme={NIVO_THEME}
          margin={{ top: 10, right: 20, bottom: 30, left: 50 }}
          padding={0.25}
          groupMode="grouped"
          axisBottom={{ tickSize: 0, tickPadding: 8 }}
          axisLeft={{ tickSize: 0, tickPadding: 8, format: (v) => `$${Number(v).toLocaleString()}` }}
          gridYValues={5}
          enableGridY
          enableLabel={false}
          legends={[
            {
              dataFrom: "keys",
              anchor: "top-right",
              direction: "row",
              translateY: -10,
              itemWidth: 70,
              itemHeight: 16,
              symbolSize: 10,
            },
          ]}
          role="img"
          ariaLabel="Income versus expenses by month"
        />
      </div>
      <table style={srOnlyStyle}>
        <caption>Income versus expenses by month</caption>
        <thead>
          <tr><th>Month</th><th>Income</th><th>Expenses</th></tr>
        </thead>
        <tbody>
          {data.map((d) => (
            <tr key={d.month}>
              <td>{d.month}</td>
              <td>{money(d.Income * 100)}</td>
              <td>{money(d.Expenses * 100)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

// ── Nivo net worth line ───────────────────────────────────────────────────

function NetLine({ months }: { months: MonthSummary[] }) {
  const { data, final } = useMemo(() => {
    let running = 0;
    const points = months.map((m) => {
      running += m.netCents;
      return { x: m.label, y: Math.round(running / 100) };
    });
    return { data: [{ id: "Cumulative net", data: points }], final: running };
  }, [months]);

  const isPositive = final >= 0;

  return (
    <Card className="stack stack-sm" style={{ padding: "22px 4px 12px" }}>
      <div style={{ padding: "0 20px 4px" }}>
        <div className="h3">Cumulative net (12-month running total)</div>
        <div className={`figure num ${isPositive ? "pos" : "neg"}`} style={{ fontSize: 24, marginTop: 4 }}>
          {money(final)}
        </div>
      </div>
      <div style={{ height: 100 }}>
        <ResponsiveLine
          data={data}
          colors={[isPositive ? "var(--positive)" : "var(--negative)"]}
          theme={NIVO_THEME}
          margin={{ top: 10, right: 20, bottom: 24, left: 50 }}
          axisBottom={{ tickSize: 0, tickPadding: 8 }}
          axisLeft={{ tickSize: 0, tickPadding: 8, format: (v) => `$${Number(v).toLocaleString()}` }}
          enableGridY={false}
          enableGridX={false}
          enablePoints
          pointSize={4}
          pointBorderWidth={2}
          pointBorderColor={{ from: "serieColor" }}
          enableArea
          areaOpacity={0.15}
          curve="monotoneX"
          role="img"
          ariaLabel="Cumulative net worth over the last 12 months"
        />
      </div>
      <table style={srOnlyStyle}>
        <caption>Cumulative net worth by month</caption>
        <thead>
          <tr><th>Month</th><th>Cumulative net</th></tr>
        </thead>
        <tbody>
          {data[0]!.data.map((d) => (
            <tr key={d.x}>
              <td>{d.x}</td>
              <td>{money(Number(d.y) * 100)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

// ── Nivo donut chart ───────────────────────────────────────────────────────

function DonutChart({ categories, totalCents }: {
  categories: { label: string; color: string; totalCents: number }[];
  totalCents: number;
}) {
  const data = useMemo(() =>
    categories
      .filter((c) => c.totalCents > 0)
      .map((c) => ({
        id: c.label,
        label: c.label,
        value: Math.round(c.totalCents / 100),
        color: c.color || "var(--accent)",
      })),
    [categories]
  );

  return (
    <Card className="stack stack-sm" style={{ padding: 20 }}>
      <div className="eyebrow">Spending by category</div>
      <div style={{ display: "grid", gridTemplateColumns: "120px 1fr", gap: 24, alignItems: "flex-start" }}>
        <div style={{ height: 120 }}>
          <ResponsivePie
            data={data}
            colors={{ datum: "data.color" }}
            theme={NIVO_THEME}
            innerRadius={0.65}
            padAngle={0.7}
            cornerRadius={3}
            enableArcLabels={false}
            enableArcLinkLabels={false}
            tooltip={({ datum }) => (
              <div>
                <strong>{datum.label}</strong>: {money(Number(datum.value) * 100)} ({Math.round((datum.value / (totalCents / 100)) * 100)}%)
              </div>
            )}
            role="img"
          />
        </div>
        <div className="stack stack-xs" style={{ flex: 1, minWidth: 0 }}>
          {data.slice(0, 8).map((s) => (
            <div key={s.id} className="row-sm" style={{ alignItems: "center", fontSize: 12 }}>
              <span
                className="swatch"
                style={{ background: s.color, flexShrink: 0 }}
                aria-hidden="true"
              />
              <span className="grow" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{s.label}</span>
              <span className="num muted" style={{ fontSize: 11 }}>
                {Math.round((s.value / (totalCents / 100)) * 100)}%
              </span>
            </div>
          ))}
        </div>
      </div>
      <table style={srOnlyStyle}>
        <caption>Spending by category</caption>
        <thead>
          <tr><th>Category</th><th>Amount</th><th>Share</th></tr>
        </thead>
        <tbody>
          {data.map((d) => (
            <tr key={d.id}>
              <td>{d.label}</td>
              <td>{money(d.value * 100)}</td>
              <td>{Math.round((d.value / (totalCents / 100)) * 100)}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

// ── Nivo year-over-year line chart ─────────────────────────────────────────

function YoYChart({ thisYear, lastYear }: {
  thisYear: { label: string; expenseCents: number }[];
  lastYear: { expenseCents: number }[];
}) {
  if (thisYear.length === 0) return null;

  const data = useMemo(() => [
    {
      id: "This year",
      data: thisYear.map((m) => ({ x: m.label, y: Math.round(m.expenseCents / 100) })),
    },
    ...(lastYear.length > 0 ? [{
      id: "Last year",
      data: lastYear.map((m, i) => ({ x: thisYear[i]?.label ?? i, y: Math.round(m.expenseCents / 100) })),
    }] : []),
  ], [thisYear, lastYear]);

  const labelIndices = thisYear.length <= 6
    ? thisYear.map((_, i) => i)
    : thisYear.map((_, i) => i).filter((i) => i % Math.ceil(thisYear.length / 6) === 0);

  return (
    <Card className="stack stack-sm" style={{ padding: 20 }}>
      <div className="eyebrow">Year-over-year expenses</div>
      <div style={{ height: 100 }}>
        <ResponsiveLine
          data={data}
          colors={["var(--accent)", "var(--ink-mute)"]}
          theme={NIVO_THEME}
          margin={{ top: 10, right: 20, bottom: 24, left: 50 }}
          axisBottom={{
            tickSize: 0,
            tickPadding: 8,
            tickValues: labelIndices.map((i) => thisYear[i]!.label),
          }}
          axisLeft={{ tickSize: 0, tickPadding: 8, format: (v) => `$${Number(v).toLocaleString()}` }}
          enableGridY={false}
          enableGridX={false}
          enablePoints={false}
          curve="monotoneX"
          lineWidth={2}
          role="img"
          ariaLabel="Year-over-year expenses comparison"
        />
      </div>
      <div className="row-sm" style={{ fontSize: 12, color: "var(--ink-mute)", padding: "8px 12px 0" }}>
        <span className="row-xs">
          <span style={{ width: 16, height: 2, background: "var(--accent)", display: "inline-block" }} />This year
        </span>
        <span className="row-xs">
          <span style={{ width: 16, height: 2, background: "var(--ink-mute)", display: "inline-block" }} />Last year
        </span>
      </div>
      <table style={srOnlyStyle}>
        <caption>Year-over-year expenses</caption>
        <thead>
          <tr><th>Month</th><th>This year</th><th>Last year</th></tr>
        </thead>
        <tbody>
          {thisYear.map((m, i) => (
            <tr key={m.label}>
              <td>{m.label}</td>
              <td>{money(m.expenseCents)}</td>
              <td>{money(lastYear[i]?.expenseCents ?? 0)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────

export default function Reports() {
  const [tabs, setTabs] = useState<ReportTab[]>(() => {
    try {
      const saved = localStorage.getItem("report_tabs");
      if (saved) {
        const parsed = JSON.parse(saved) as ReportTab[];
        const valid = parsed.filter(t =>
          t && typeof t.id === "string" && typeof t.name === "string" &&
          t.widgets && typeof t.widgets.barChart === "boolean"
        );
        if (valid.length > 0) return valid;
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

  if (!isLoading && !error && (!data || data.monthly.every((m) => m.incomeCents === 0 && m.expenseCents === 0))) {
    return (
      <div className="screen screen-reports">
        <div className="screen-header">
          <div className="screen-header-text">
            <div className="screen-eyebrow">Reports</div>
            <h1>See the shape of your money over time.</h1>
          </div>
        </div>
        <Card className="stack stack-md" style={{ textAlign: "center", padding: "64px 32px" }}>
          <div style={{ fontSize: 18, fontWeight: 600 }}>No data yet</div>
          <p className="muted" style={{ margin: 0, fontSize: 14 }}>Import transactions to see reports here.</p>
        </Card>
      </div>
    );
  }

  const kpiPrefix =
    scope === "month" ? "This month's" :
    scope === "quarter" ? "Quarter" :
    scope === "year" ? "12-month" :
    "All-time";

  const totalIncome  = data ? data.monthly.reduce((s, m) => s + m.incomeCents, 0) : 0;
  const totalExpense = data ? data.monthly.reduce((s, m) => s + m.expenseCents, 0) : 0;
  const netTotal     = totalIncome - totalExpense;
  const savingsRate  = totalIncome > 0 ? Math.round((netTotal / totalIncome) * 100) : 0;
  const activeMonths = data ? (data.monthly.filter((m) => m.incomeCents + m.expenseCents > 0).length || 1) : 1;
  const avgMonthlySpend = Math.round(totalExpense / activeMonths);

  const allWidgetsHidden =
    !activeTab.widgets.barChart && !activeTab.widgets.netLine &&
    !activeTab.widgets.donut && !activeTab.widgets.yoy &&
    !activeTab.widgets.categories && !activeTab.widgets.merchants;

  return (
    <div className="screen screen-reports">
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
      <div className="row-sm" style={{ borderBottom: "1px solid var(--line)", paddingBottom: 8, marginBottom: 12 }}>
        {tabs.map(tab => (
          <div key={tab.id} className="row-xs" style={{ position: "relative" }}>
            {renamingId === tab.id ? (
              <input
                autoFocus
                defaultValue={tab.name}
                className="screen-reports"
                style={{ padding: "2px 8px", width: 120, fontSize: 13 }}
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
              <Button
                variant="ghost"
                size="sm"
                className={activeTabId === tab.id ? "active" : ""}
                onClick={() => setActiveTabId(tab.id)}
                onDoubleClick={() => setRenamingId(tab.id)}
                title="Double-click to rename"
              >
                {tab.name}
              </Button>
            )}
            {tab.id !== "default" && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setTabs(prev => prev.filter(t => t.id !== tab.id));
                  setActiveTabId("default");
                }}
                title="Delete tab"
                aria-label={`Delete tab ${tab.name}`}
                style={{ padding: "0 4px", fontSize: 11 }}
              >
                ✕
              </Button>
            )}
          </div>
        ))}

        <Button
          variant="ghost"
          size="sm"
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
        </Button>

        <div style={{ marginLeft: "auto" }}>
          <Button
            variant="ghost"
            size="sm"
            className={customize ? "active" : ""}
            onClick={() => setCustomize(c => !c)}
            title="Show/hide widgets"
          >
            ✎ Customize
          </Button>
        </div>
      </div>

      {/* Customize panel */}
      {customize && (
        <Card className="row-sm wrap" style={{ padding: "12px 16px", marginBottom: 16, alignItems: "center" }}>
          <span className="eyebrow">Widgets</span>
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
            <Button
              key={key}
              variant={activeTab.widgets[key] ? "primary" : "ghost"}
              size="sm"
              onClick={() =>
                updateActiveTab({
                  widgets: { ...activeTab.widgets, [key]: !activeTab.widgets[key] },
                })
              }
            >
              {activeTab.widgets[key] ? "✓ " : ""}{label}
            </Button>
          ))}
        </Card>
      )}

      {/* Scope toolbar */}
      <div className="toolbar" style={{ marginBottom: 20 }}>
        {(["month", "quarter", "year", "all"] as const).map(s => (
          <button
            key={s}
            className={activeTab.scope === s ? "on" : ""}
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

      {/* Content area: loading / error / charts */}
      {isLoading ? (
        <div className="stub" style={{ minHeight: 200, marginTop: 16 }}>Loading…</div>
      ) : error ? (
        <div className="stub" style={{ minHeight: 200, marginTop: 16 }}>Error computing reports.</div>
      ) : data ? (
        <>
          {/* Chart grid 1 */}
          {(activeTab.widgets.barChart || activeTab.widgets.netLine) && (
            <div className="responsive-grid" style={{ gridTemplateColumns: "1fr 1fr", marginTop: 24 }}>
              {activeTab.widgets.barChart && <BarChart months={data.monthly} scope={barScope} />}
              {activeTab.widgets.netLine && <NetLine months={data.monthly} />}
            </div>
          )}

          {/* Chart grid 2 */}
          {(activeTab.widgets.donut || activeTab.widgets.yoy) && (
            <div className="responsive-grid" style={{ gridTemplateColumns: "1fr 1fr", marginTop: 16 }}>
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
            <div className="responsive-grid" style={{ gridTemplateColumns: "1fr 1fr", marginTop: 16 }}>
              {activeTab.widgets.categories && (
                <Card flush>
                  <div className="card-head">
                    <div className="h3">Top categories</div>
                    <div className="muted" style={{ fontSize: 12 }}>12-month spend</div>
                  </div>
                  <Table>
                    <TableHead>
                      <TableRow>
                        <TableHeader>Category</TableHeader>
                        <TableHeader right>Total</TableHeader>
                        <TableHeader right>Txns</TableHeader>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {data.topCategories.map((c) => (
                        <TableRow key={c.categoryId}>
                          <TableCell>
                            <div className="row-sm">
                              <span className="swatch" style={{ background: c.color || "var(--ink-faint)" }} aria-hidden="true" />
                              <span>{c.label}</span>
                            </div>
                          </TableCell>
                          <TableCell right><span className="num tabular money">{money(c.totalCents)}</span></TableCell>
                          <TableCell right><span className="num muted">{c.txnCount}</span></TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </Card>
              )}

              {activeTab.widgets.merchants && (
                <Card flush>
                  <div className="card-head">
                    <div className="h3">Top merchants</div>
                    <div className="muted" style={{ fontSize: 12 }}>12-month spend</div>
                  </div>
                  <Table>
                    <TableHead>
                      <TableRow>
                        <TableHeader>Merchant</TableHeader>
                        <TableHeader right>Total</TableHeader>
                        <TableHeader right>Txns</TableHeader>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {data.topMerchants.map((m, i) => (
                        <TableRow key={`${m.merchantRaw}-${i}`}>
                          <TableCell>
                            <div className="stack stack-xs">
                              <div>{m.merchantRaw}</div>
                              <div className="muted" style={{ fontSize: 12 }}>{m.categoryLabel || "Uncategorized"}</div>
                            </div>
                          </TableCell>
                          <TableCell right><span className="num tabular money">{money(m.totalCents)}</span></TableCell>
                          <TableCell right><span className="num muted">{m.txnCount}</span></TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </Card>
              )}
            </div>
          )}

          {/* All-widgets-hidden empty state */}
          {allWidgetsHidden && (
            <div className="stub" style={{ marginTop: 16, textAlign: "center", padding: "40px 20px" }}>
              All widgets hidden — use{" "}
              <Button variant="ghost" size="sm" onClick={() => setCustomize(true)} style={{ display: "inline" }}>
                ✎ Customize
              </Button>{" "}
              to re-enable some.
            </div>
          )}

          {/* Copilot nudge */}
          <div style={{ marginTop: 24 }}>
            <CopilotNudge
              prompt={`Analyze my ${scope === "month" ? "this month's" : scope === "quarter" ? "last quarter's" : scope === "year" ? "this year's" : "all-time"} spending trends, identify where I'm overspending, and suggest a concrete plan to improve my savings rate.`}
              label="Turn these trends into a plan"
              description="Let Copilot summarize your trends and suggest next steps"
              variant="info"
            />
          </div>
        </>
      ) : null}
    </div>
  );
}

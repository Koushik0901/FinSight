import { useMemo, useState } from "react";
import { useCustomReport } from "../../api/hooks/reports";
import type { ReportWidget } from "../../api/hooks/reportWidgets";
import { money } from "../../utils/format";
import { usePlotAnim } from "../../utils/plotMotion";
import Reveal from "../Reveal";
import type { CustomReportParams } from "../../api/openapiClient";
import { PieChart, Pie, Cell, ResponsiveContainer, LineChart, Line, XAxis, YAxis, Tooltip, AreaChart, Area } from "recharts";
import "./reportWidgets.css";

type Props = {
  widget: ReportWidget;
  memberId?: string | null;
};

function parseFilters(filtersJson: string): {
  includeTransfers: boolean;
  includeArchived: boolean;
  accountIds: string[];
  categoryIds: string[];
  payee: string | null;
  spendingType: string | null;
  minAmount: number | null;
  maxAmount: number | null;
  interval: string | null;
  metric: string | null;
} {
  try {
    const j = JSON.parse(filtersJson || "{}");
    return {
      includeTransfers: Boolean(j.includeTransfers ?? j.include_transfers ?? false),
      includeArchived: Boolean(j.includeArchived ?? j.include_archived ?? false),
      accountIds: Array.isArray(j.accountIds) ? j.accountIds as string[] : Array.isArray(j.account_ids) ? j.account_ids as string[] : [],
      categoryIds: Array.isArray(j.categoryIds) ? j.categoryIds as string[] : Array.isArray(j.category_ids) ? j.category_ids as string[] : [],
      payee: (j.payee as string | undefined) ?? null,
      spendingType: (j.spendingType as string | undefined) ?? (j.spending_type as string | undefined) ?? null,
      minAmount: j.minAmount != null ? Number(j.minAmount) : j.min_amount_cents != null ? Number(j.min_amount_cents) / 100 : j.minAmountCents != null ? Number(j.minAmountCents) / 100 : null,
      maxAmount: j.maxAmount != null ? Number(j.maxAmount) : j.max_amount_cents != null ? Number(j.max_amount_cents) / 100 : j.maxAmountCents != null ? Number(j.maxAmountCents) / 100 : null,
      interval: (j.interval as string | undefined) ?? null,
      metric: (j.metric as string | undefined) ?? null,
    };
  } catch {
    return { includeTransfers: false, includeArchived: false, accountIds: [], categoryIds: [], payee: null, spendingType: null, minAmount: null, maxAmount: null, interval: null, metric: null };
  }
}

function toParams(widget: ReportWidget, memberId?: string | null): CustomReportParams {
  const f = parseFilters(widget.filtersJson);
  const splitBy = widget.splitBy as CustomReportParams["splitBy"];
  const period = widget.period as CustomReportParams["period"];
  return {
    splitBy,
    period,
    includeTransfers: f.includeTransfers,
    includeArchived: f.includeArchived,
    memberId: memberId ?? null,
    accountIds: f.accountIds,
    categoryIds: f.categoryIds,
    groupIds: [],
    payee: f.payee,
    spendingType: f.spendingType,
    minAmountCents: f.minAmount != null ? Math.round(f.minAmount * 100) : null,
    maxAmountCents: f.maxAmount != null ? Math.round(f.maxAmount * 100) : null,
    interval: f.interval,
    metric: f.metric,
  } as unknown as CustomReportParams;
}

const COLORS = ["#84cc16", "#22c55e", "#06b6d4", "#8b5cf6", "#f59e0b", "#ef4444", "#ec4899", "#6366f1", "#14b8a6", "#f97316"];

export default function WidgetRenderer({ widget, memberId }: Props) {
  const params = useMemo(() => toParams(widget, memberId), [widget, memberId]);
  const { data, isLoading, error, refetch } = useCustomReport(params);
  const anim = usePlotAnim();
  // Donut hover focus: index of the emphasized slice, null when unfocused.
  const [activeSlice, setActiveSlice] = useState<number | null>(null);

  const rows = data?.rows ?? [];
  const maxTotal = Math.max(1, ...rows.map((r) => r.totalCents), 0);

  if (isLoading) {
    return <div className="stub" style={{ padding: 18 }}>Loading {widget.title}…</div>;
  }
  if (error) {
    return (
      <div role="alert" className="muted" style={{ padding: 16, fontSize: 13 }}>
        Could not load {widget.title}. <button className="btn outline sm" onClick={() => void refetch()}>Try again</button>
      </div>
    );
  }
  if (rows.length === 0) {
    return (
      <div className="muted" style={{ padding: "22px 0", textAlign: "center", fontSize: 13 }}>
        No transactions match these filters in {widget.period}. Try a wider period or clear filters.
      </div>
    );
  }

  // Special handling for month split: treat as time-series if chart is line/area
  const isTimeSeries = widget.splitBy === "month";
  if (widget.chartType === "table") {
    return (
      <Reveal>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div className="eyebrow" style={{ fontSize: 12 }}>Total {money(data?.totalCents ?? 0)} · {rows.length} groups</div>
          {rows.slice(0, 20).map((r, idx) => (
            <div key={r.label} style={{ display: "grid", gridTemplateColumns: "140px 1fr auto", gap: 12, alignItems: "center" }}>
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 13 }}>{r.label}</span>
              <div style={{ height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                <div
                  className="plot-grow-x wg-fill"
                  style={{ width: `${(r.totalCents / maxTotal) * 100}%`, height: "100%", background: "var(--accent)", borderRadius: 999, animationDelay: `${Math.min(idx, 10) * 40}ms` }}
                />
              </div>
              <span className="money" style={{ fontSize: 13 }}>{money(r.totalCents)} · {r.txnCount}</span>
            </div>
          ))}
          {rows.length > 20 && <span className="muted" style={{ fontSize: 12 }}>+ {rows.length - 20} more</span>}
        </div>
      </Reveal>
    );
  }
  if (widget.chartType === "bar" || widget.chartType === "barStacked") {
    return (
      <Reveal>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div className="eyebrow" style={{ fontSize: 12 }}>Total {money(data?.totalCents ?? 0)} · {rows.length} groups</div>
          <div style={{ display: "grid", gridTemplateColumns: `repeat(${Math.min(rows.length, 12)}, minmax(0, 1fr))`, gap: rows.length > 12 ? 6 : 12, alignItems: "end", minHeight: 180, paddingTop: 8 }}>
            {rows.slice(0, 24).map((r, idx) => (
              <div key={r.label} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 8 }}>
                <div style={{ width: "100%", minHeight: 140, display: "flex", alignItems: "end", justifyContent: "center" }}>
                  <span
                    className="plot-grow-y wg-fill-v"
                    style={{ width: rows.length > 12 ? 14 : 28, height: `${(r.totalCents / maxTotal) * 140}px`, borderRadius: 8, background: "var(--accent)", minHeight: 4, animationDelay: `${Math.min(idx, 10) * 40}ms` }}
                    title={`${r.label} ${money(r.totalCents)}`}
                  />
                </div>
                <span className="mono muted" style={rows.length > 12 ? { fontSize: 9, writingMode: "vertical-rl", maxHeight: 60, overflow: "hidden" } : { fontSize: 10, textAlign: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 80 }}>{r.label.slice(0, 12)}</span>
              </div>
            ))}
          </div>
        </div>
      </Reveal>
    );
  }
  if (widget.chartType === "donut") {
    const pieData = rows.slice(0, 8).map((r, i) => ({ name: r.label, value: r.totalCents, fill: COLORS[i % COLORS.length] }));
    const total = rows.reduce((s, r) => s + r.totalCents, 0);
    const focus = (i: number | null) => setActiveSlice(i);
    return (
      <Reveal>
        <div className="widget-donut-grid">
          <div style={{ width: "100%", height: 180 }}>
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie
                  data={pieData}
                  dataKey="value"
                  nameKey="name"
                  innerRadius={52}
                  outerRadius={80}
                  paddingAngle={2}
                  {...anim}
                  activeIndex={activeSlice ?? undefined}
                  onMouseEnter={(_: unknown, i: number) => focus(i)}
                  onMouseLeave={() => focus(null)}
                >
                  {pieData.map((entry, idx) => (
                    <Cell
                      key={idx}
                      fill={entry.fill}
                      style={{
                        fillOpacity: activeSlice != null && idx !== activeSlice ? 0.35 : 1,
                        transition: "fill-opacity 200ms cubic-bezier(0.25, 1, 0.5, 1)",
                        outline: "none",
                        cursor: "pointer",
                      }}
                    />
                  ))}
                </Pie>
                <Tooltip formatter={(v: number) => money(v)} />
              </PieChart>
            </ResponsiveContainer>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <div className="eyebrow" style={{ fontSize: 12 }}>{money(total)} total</div>
            {rows.slice(0, 8).map((r, i) => (
              <div
                key={r.label}
                className="row row-sm wg-legend-row"
                style={{ gap: 8, fontSize: 12, opacity: activeSlice != null && i !== activeSlice ? 0.55 : 1 }}
                onMouseEnter={() => focus(i)}
                onMouseLeave={() => focus(null)}
              >
                <span style={{ width: 10, height: 10, borderRadius: 3, background: COLORS[i % COLORS.length], flexShrink: 0 }} />
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}>{r.label}</span>
                <span className="money">{money(r.totalCents)}</span>
              </div>
            ))}
            {rows.length > 8 && <span className="muted" style={{ fontSize: 11 }}>+{rows.length - 8} more</span>}
          </div>
        </div>
      </Reveal>
    );
  }
  if (widget.chartType === "line" || widget.chartType === "area") {
    // For non-time-series, fallback to bar-like line over sorted groups
    const chartData = rows.slice(0, 12).map((r) => ({ label: r.label.slice(0, 8), total: r.totalCents / 100 }));
    if (isTimeSeries) {
      // rows already sorted by total desc, but for time series we want label asc (YYYY-MM)
      const sorted = [...rows].sort((a, b) => a.label.localeCompare(b.label)).slice(0, 24).map((r) => ({ label: r.label.slice(5), total: r.totalCents / 100 }));
      const ChartComp = widget.chartType === "area" ? AreaChart : LineChart;
      return (
        <Reveal>
          <div style={{ width: "100%", height: 200 }}>
            <ResponsiveContainer width="100%" height="100%">
              <ChartComp data={sorted}>
                <XAxis dataKey="label" tick={{ fontSize: 10 }} />
                <YAxis tick={{ fontSize: 10 }} tickFormatter={(v) => `$${v}`} width={45} />
                <Tooltip formatter={(v: number) => `$${v.toFixed(2)}`} />
                {widget.chartType === "area" ? <Area type="monotone" dataKey="total" stroke="#84cc16" fill="#84cc16" fillOpacity={0.2} strokeWidth={2} dot={false} {...anim} /> : <Line type="monotone" dataKey="total" stroke="#84cc16" strokeWidth={2} dot={false} {...anim} />}
              </ChartComp>
            </ResponsiveContainer>
          </div>
        </Reveal>
      );
    }
    return (
      <Reveal>
        <div style={{ width: "100%", height: 200 }}>
          <ResponsiveContainer width="100%" height="100%">
            {widget.chartType === "area" ? (
              <AreaChart data={chartData}>
                <XAxis dataKey="label" tick={{ fontSize: 10 }} />
                <YAxis tick={{ fontSize: 10 }} tickFormatter={(v) => `$${v}`} width={45} />
                <Tooltip formatter={(v: number) => `$${v.toFixed(2)}`} />
                <Area type="monotone" dataKey="total" stroke="#84cc16" fill="#84cc16" fillOpacity={0.2} strokeWidth={2} {...anim} />
              </AreaChart>
            ) : (
              <LineChart data={chartData}>
                <XAxis dataKey="label" tick={{ fontSize: 10 }} />
                <YAxis tick={{ fontSize: 10 }} tickFormatter={(v) => `$${v}`} width={45} />
                <Tooltip formatter={(v: number) => `$${v.toFixed(2)}`} />
                <Line type="monotone" dataKey="total" stroke="#84cc16" strokeWidth={2} dot={false} {...anim} />
              </LineChart>
            )}
          </ResponsiveContainer>
        </div>
      </Reveal>
    );
  }

  return (
    <div className="muted" style={{ padding: 16 }}>Unknown chart type {widget.chartType}</div>
  );
}

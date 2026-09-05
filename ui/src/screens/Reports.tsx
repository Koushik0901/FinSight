import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api, type ReportData } from "../api/openapiClient";
import { unwrap } from "../api/openapiClient";
import { money } from "../utils/format";
import { useNetWorth } from "../api/hooks/networth";
import { useFinancialMetrics } from "../api/hooks/metrics";
import { useAccounts } from "../api/hooks/accounts";
import MemberSwitcher from "../components/MemberSwitcher";
import { UnconvertedCurrencies } from "../components/UnconvertedCurrencies";
import PageHeader from "../components/PageHeader";
import EmptyState from "../components/EmptyState";
import CountUp from "../components/CountUp";
import Reveal from "../components/Reveal";
import { getReportReadiness } from "../utils/dataReadiness";
import ReportCanvas from "../components/reportWidgets/ReportCanvas";
import { usePlotAnim } from "../utils/plotMotion";
import { ComposedChart, Bar, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, Legend, CartesianGrid, ReferenceLine } from "recharts";
import SignalTrace from "../components/SignalTrace";
type Scope = "month" | "quarter" | "year" | "all";

function getBudgetCents(m: ReportData["monthly"][number]): number {
  if (m !== null && typeof m === "object" && "budgetCents" in m) {
    const v = m.budgetCents;
    if (typeof v === "number" && Number.isFinite(v)) return v;
  }
  return 0;
}

export function buildReportCsv(data: ReportData): string {
  const rows: string[] = [];
  rows.push("Section,Label,Income,Expense,Budget,Net");
  for (const month of data.monthly) {
    const budget = getBudgetCents(month);
    rows.push(`Monthly,${month.label},${(month.incomeCents / 100).toFixed(2)},${(month.expenseCents / 100).toFixed(2)},${(budget / 100).toFixed(2)},${(month.netCents / 100).toFixed(2)}`);
  }
  rows.push("");
  rows.push("Section,Category,Amount,Txns");
  for (const category of data.topCategories) {
    rows.push(`Top category,"${category.label.replace(/"/g, '""')}",${(category.totalCents / 100).toFixed(2)},${category.txnCount}`);
  }
  rows.push("");
  rows.push("Section,Merchant,Amount,Txns");
  for (const merchant of data.topMerchants) {
    rows.push(`Top merchant,"${merchant.merchantRaw.replace(/"/g, '""')}",${(merchant.totalCents / 100).toFixed(2)},${merchant.txnCount}`);
  }
  return rows.join("\n");
}

type BudgetVsActualRow = { month: string; label: string; budget: number; expense: number; variance: number };

function BudgetVsActualChart({ data, onNavigateBudget }: { data: ReportData; onNavigateBudget: () => void }) {
  const barAnim = usePlotAnim();
  const lineAnim = usePlotAnim({ begin: 140 });
  const monthly = data.monthly;
  const hasBudget = monthly.some((m) => getBudgetCents(m) > 0);
  const chartData: BudgetVsActualRow[] = monthly.map((m) => {
    const budget = getBudgetCents(m) / 100;
    const expense = m.expenseCents / 100;
    return { month: m.month, label: m.label, budget, expense, variance: budget - expense };
  });
  const totalBudget = chartData.reduce((s, r) => s + r.budget, 0);
  const totalExpense = chartData.reduce((s, r) => s + r.expense, 0);
  const totalVariance = totalBudget - totalExpense;
  const maxVal = Math.max(1, ...chartData.flatMap((r) => [r.budget, r.expense]));
  const yDomain: [number, number] = [0, Math.ceil(maxVal * 1.15)];

  if (!hasBudget) {
    return (
      <div className="card" style={{ padding: 16 }} data-testid="budget-vs-actual-empty">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
          <div>
            <h3 className="h3" style={{ margin: 0 }}>Budget vs Actual</h3>
            <p className="muted" style={{ margin: "4px 0 0", fontSize: 12 }}>Compare what you planned to spend against what you spent — month by month.</p>
          </div>
          <button className="btn primary sm" type="button" onClick={onNavigateBudget}>Set budgets</button>
        </div>
        <div className="muted" style={{ textAlign: "center", padding: "28px 12px", fontSize: 13, lineHeight: 1.5 }}>
          No budgets yet for this period. Set monthly budgets on the Budget screen to see the overlay.
        </div>
      </div>
    );
  }
  return (
    <Reveal>
      <div className="card" style={{ padding: 16 }} data-testid="budget-vs-actual">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12, flexWrap: "wrap", marginBottom: 12 }}>
        <div>
          <h3 className="h3" style={{ margin: 0 }}>Budget vs Actual</h3>
          <p className="muted" style={{ margin: "4px 0 0", fontSize: 12 }}>Budgeted (line) vs actual spending (bars) — positive variance means under budget.</p>
        </div>
        <div style={{ display: "flex", gap: 12, flexWrap: "wrap", textAlign: "right" }}>
          <div>
            <div className="eyebrow" style={{ fontSize: 10 }}>Total budgeted</div>
            <div className="money" style={{ fontWeight: 700, fontSize: 14 }}>
              <CountUp value={totalBudget} format={(v) => money(Math.round(v * 100))} />
            </div>
          </div>
          <div>
            <div className="eyebrow" style={{ fontSize: 10 }}>Total spent</div>
            <div className="money" style={{ fontWeight: 700, fontSize: 14 }}>
              <CountUp value={totalExpense} format={(v) => money(Math.round(v * 100))} />
            </div>
          </div>
          <div>
            <div className="eyebrow" style={{ fontSize: 10 }}>Variance</div>
            <div style={{ fontWeight: 700, fontSize: 14, color: totalVariance >= 0 ? "var(--positive, #16a34a)" : "var(--negative)" }}>
              <CountUp value={totalVariance} format={(v) => (v > 0 ? "+" : "") + money(Math.round(v * 100))} />
            </div>
            <div className="muted" style={{ fontSize: 10 }}>{totalVariance >= 0 ? "Under budget" : "Over budget"}</div>
          </div>
        </div>
      </div>

      <div style={{ width: "100%", height: 240 }}>
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart data={chartData} margin={{ top: 8, right: 12, left: 8, bottom: 0 }} barCategoryGap="28%">
            <CartesianGrid strokeDasharray="3 3" stroke="var(--line, #e5e7eb)" vertical={false} />
            <XAxis dataKey="label" tick={{ fontSize: 11 }} axisLine={false} tickLine={false} />
            <YAxis domain={yDomain} tick={{ fontSize: 11 }} tickFormatter={(v: number) => `$${Math.round(v).toLocaleString()}`} width={64} axisLine={false} tickLine={false} />
            <Tooltip
              formatter={(value: number, name: string) => [`$${Number(value).toFixed(2)}`, name === "budget" ? "Budgeted" : name === "expense" ? "Actual" : name]}
              labelFormatter={(l) => `Month: ${l}`}
              cursor={{ fill: "var(--surface-2)" }}
              contentStyle={{ borderRadius: 10, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 12 }}
              itemStyle={{ color: "var(--ink)" }}
              labelStyle={{ color: "var(--ink-mute)" }}
            />
            <Legend verticalAlign="top" height={24} iconType="plainline" formatter={(value) => <span style={{ fontSize: 12 }}>{value === "budget" ? "Budgeted" : value === "expense" ? "Actual spend" : value}</span>} />
            <Bar
              dataKey="expense"
              name="expense"
              fill="var(--accent, #84cc16)"
              radius={[8, 8, 0, 0]}
              barSize={22}
              {...barAnim}
              activeBar={{ fill: "color-mix(in srgb, var(--accent) 70%, #fff)" }}
            />
            <Line type="monotone" dataKey="budget" name="budget" stroke="#0ea5e9" strokeWidth={2.5} dot={{ r: 3, strokeWidth: 2 }} activeDot={{ r: 5 }} {...lineAnim} />
            <ReferenceLine y={0} stroke="var(--line)" />
          </ComposedChart>
        </ResponsiveContainer>
      </div>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 10 }}>
        {chartData.map((r) => {
          const over = r.expense > r.budget && r.budget > 0;
          return (
            <span
              key={r.month}
              className="chip"
              style={{
                fontSize: 11,
                padding: "4px 8px",
                borderColor: over ? "var(--negative)" : r.budget === 0 ? "var(--line)" : "var(--positive, #16a34a)",
                background: over ? "color-mix(in srgb, var(--negative) 10%, transparent)" : "var(--surface-2)",
              }}
              title={`${r.label}: budget $${r.budget.toFixed(2)} vs $${r.expense.toFixed(2)} (${r.variance >= 0 ? "+" : ""}$${r.variance.toFixed(2)})`}
            >
              {r.label}: {over ? "over" : r.budget === 0 ? "no budget" : "under"} {r.budget > 0 ? money(Math.abs(Math.round(r.variance * 100))) : ""}
            </span>
          );
        })}
      </div>
      </div>
    </Reveal>
  );
}

function useReportData(scope: Scope, memberId: string | null) {
  return useQuery<ReportData>({
    queryKey: ["report-data", scope, memberId],
    queryFn: async () => {
      return unwrap(api.getReportData(scope, memberId));
    },
    staleTime: 60_000,
  });
}


export default function Reports() {
  const navigate = useNavigate();
  const [scope, setScope] = useState<Scope>("year");
  const [memberId, setMemberId] = useState<string | null>(null);
  const { data, isLoading, error, refetch } = useReportData(scope, memberId);
  const netWorth = useNetWorth();
  const { data: metrics } = useFinancialMetrics();
  const { data: accounts = [] } = useAccounts();

  const monthly = useMemo(() => data?.monthly ?? [], [data?.monthly]);
  const totalIncome = monthly.reduce((sum, month) => sum + month.incomeCents, 0);
  const totalExpense = monthly.reduce((sum, month) => sum + month.expenseCents, 0);
  const monthlyLastYear = useMemo(() => data?.monthlyLastYear ?? [], [data?.monthlyLastYear]);
  const totalExpenseLastYear = monthlyLastYear.reduce((sum, month) => sum + month.expenseCents, 0);
  const yoyDeltaPct = scope !== "all" && totalExpenseLastYear > 0 ? Math.round(((totalExpense - totalExpenseLastYear) / totalExpenseLastYear) * 100) : null;
  const savingsRate = totalIncome > 0 ? Math.round(((totalIncome - totalExpense) / totalIncome) * 100) : 0;
  const activeExpenseMonths = monthly.filter((m) => m.expenseCents > 0).length;
  const avgMonthlyExpense = activeExpenseMonths > 0 ? Math.round(totalExpense / activeExpenseMonths) : 0;
  const runwayMonths = metrics?.runwayDays != null ? Math.round(metrics.runwayDays / 30) : null;
  const readiness = getReportReadiness(monthly, accounts, metrics?.runwayDays);
  const hasActivity = readiness.hasActivity;

  const scopeLabel = useMemo(() => {
    if (scope === "quarter") return "Quarter";
    if (scope === "year") return "Year";
    if (scope === "all") return "All-time";
    const anchor = monthly[monthly.length - 1]?.month;
    if (anchor) {
      const [y, m] = anchor.split("-");
      return new Date(Number(y), Number(m) - 1, 1).toLocaleDateString("en-US", { month: "long", year: "numeric" });
    }
    return "Month";
  }, [scope, monthly]);

  const handleExport = () => {
    if (!data) return;
    const csv = buildReportCsv(data);
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `finsight-report-${scope}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  if (isLoading) return <div className="stub">Loading reports…</div>;
  if (error) return <div className="stub" role="alert"><p>Reports could not load.</p><button className="btn outline sm" type="button" onClick={() => void refetch()}>Try again</button></div>;

  if (!hasActivity) {
    return (
      <div className="screen screen-reports">
        <PageHeader
          eyebrow={<>Reports · {scopeLabel}</>}
          title="Build a history before drawing conclusions."
          description="FinSight will not turn missing activity into zero-valued results."
          actions={scope !== "all" ? <button className="btn outline sm" type="button" onClick={() => setScope("all")}>Show all time</button> : undefined}
        />
        <UnconvertedCurrencies holdings={metrics?.unconvertedHoldings} primary={metrics?.currency} />
        <EmptyState
          visual={<SignalTrace variant="reports" label="Evidence first · conclusions second" />}
          title={`No financial history in ${scopeLabel.toLowerCase()}`}
          description="Import transactions to unlock savings rate, spending trends, category comparisons, and runway."
          details={
            <ul className="empty-unlocks">
              <li>{readiness.netWorth === "reliable" ? `Known account balances currently total ${money(netWorth)}.` : "Net worth appears after at least one confirmed balance is available."}</li>
              <li>Average spend becomes an estimate after one active month and reliable after two.</li>
              <li>Savings rate needs a period with both income and spending.</li>
            </ul>
          }
          actions={<button className="btn primary" type="button" onClick={() => navigate("/accounts")}>Import transactions</button>}
        />
      </div>
    );
  }

  return (
    <div className="screen screen-reports">
      <PageHeader
        eyebrow={<>Reports · {scopeLabel}</>}
        title="How money is moving."
        description={<>See the shape of your money over time. Every widget below is yours — reorder with the grip, edit, or make your own.{scope === "all" && monthly.length >= 24 ? " Showing the most recent 24 months." : ""}</>}
        actions={
          <div className="row row-sm wrap" style={{ justifyContent: "flex-end", gap: 8 }}>
            <div className="toolbar" role="group" aria-label="Time scope">
              <button className={scope === "month" ? "on" : ""} type="button" onClick={() => setScope("month")}>Month</button>
              <button className={scope === "quarter" ? "on" : ""} type="button" onClick={() => setScope("quarter")}>Quarter</button>
              <button className={scope === "year" ? "on" : ""} type="button" onClick={() => setScope("year")}>Year</button>
              <button className={scope === "all" ? "on" : ""} type="button" onClick={() => setScope("all")}>All time</button>
            </div>
            <button className="btn outline sm" type="button" onClick={handleExport}>Export</button>
          </div>
        }
      />

      <UnconvertedCurrencies holdings={metrics?.unconvertedHoldings} primary={metrics?.currency} />

      <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap", marginBottom: 16 }}>
        <MemberSwitcher value={memberId} onChange={setMemberId} />
        {memberId && (
          <span className="muted" style={{ fontSize: 12 }}>
            Stats and widgets below are this person&apos;s share (joint accounts split by ownership share). Net worth stays household.
          </span>
        )}
      </div>

      {/* Pinned header stats — never draggable, always first */}
      <div className="stat-row" style={{ marginBottom: 18 }}>
        <div className="stat"><div className="label">Savings rate</div><div className="value">{readiness.savingsRate === "reliable" ? <CountUp value={savingsRate} format={(v) => `${Math.round(v)}%`} /> : "—"}</div><div className="sub">{readiness.savingsRate === "reliable" ? "Income kept after spending" : "Needs income and spending"}</div></div>
        <div className="stat"><div className="label">Net worth</div><div className="value money">{readiness.netWorth === "reliable" ? <CountUp value={netWorth} format={(v) => money(Math.round(v))} /> : "—"}</div><div className="sub">{readiness.netWorth === "reliable" ? "Confirmed balances" : "Needs a confirmed balance"}</div></div>
        <div className="stat"><div className="label">Average monthly spend</div><div className="value money">{readiness.averageSpend !== "unavailable" ? <CountUp value={avgMonthlyExpense} format={(v) => money(Math.round(v))} /> : "—"}</div><div className="sub">{readiness.averageSpend === "reliable" ? "Across active months" : readiness.averageSpend === "estimated" ? "Early estimate · one active month" : "Needs spending history"}</div></div>
        <div className="stat accent"><div className="label">Runway</div><div className="value">{runwayMonths !== null ? <CountUp value={runwayMonths} format={(v) => `${Math.round(v)}`} /> : "—"}</div><div className="sub">{runwayMonths !== null ? "Months of typical spending covered" : "Needs about a month of history"}</div>{yoyDeltaPct !== null && <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{yoyDeltaPct >= 0 ? "↑" : "↓"} {Math.abs(yoyDeltaPct)}% vs same months last year</div>}</div>
      </div>

      {/* P0: Budget vs Actual overlay — trending budgeted vs spent */}
      {data && <BudgetVsActualChart data={data} onNavigateBudget={() => navigate("/budget")} />}

      {/* Customizable canvas — vertical stack, drag-handle reorder, pretty on mobile & desktop */}
      <Reveal>
        <ReportCanvas memberId={memberId} />
      </Reveal>
      {/* Tail hint for mobile */}
      <div className="muted" style={{ textAlign: "center", fontSize: 11, marginTop: 18, padding: "0 12px", lineHeight: 1.5 }}>
        Tip: on mobile, use the grip to drag and the ↑↓ buttons to nudge. Each widget&apos;s <span style={{ color: "var(--ink)", fontWeight: 600 }}>⋯ → Edit</span> lets you change data slice and chart type.
      </div>
    </div>
  );
}

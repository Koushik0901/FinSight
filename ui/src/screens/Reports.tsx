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
import { getReportReadiness } from "../utils/dataReadiness";
import ReportCanvas from "../components/reportWidgets/ReportCanvas";
type Scope = "month" | "quarter" | "year" | "all";

export function buildReportCsv(data: ReportData): string {
  const rows: string[] = [];
  rows.push("Section,Label,Income,Expense,Net");
  for (const month of data.monthly) {
    rows.push(`Monthly,${month.label},${(month.incomeCents / 100).toFixed(2)},${(month.expenseCents / 100).toFixed(2)},${(month.netCents / 100).toFixed(2)}`);
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
        <div className="stat"><div className="label">Savings rate</div><div className="value">{readiness.savingsRate === "reliable" ? `${savingsRate}%` : "—"}</div><div className="sub">{readiness.savingsRate === "reliable" ? "Income kept after spending" : "Needs income and spending"}</div></div>
        <div className="stat"><div className="label">Net worth</div><div className="value money">{readiness.netWorth === "reliable" ? money(netWorth) : "—"}</div><div className="sub">{readiness.netWorth === "reliable" ? "Confirmed balances" : "Needs a confirmed balance"}</div></div>
        <div className="stat"><div className="label">Average monthly spend</div><div className="value money">{readiness.averageSpend !== "unavailable" ? money(avgMonthlyExpense) : "—"}</div><div className="sub">{readiness.averageSpend === "reliable" ? "Across active months" : readiness.averageSpend === "estimated" ? "Early estimate · one active month" : "Needs spending history"}</div></div>
        <div className="stat accent"><div className="label">Runway</div><div className="value">{runwayMonths ?? "—"}</div><div className="sub">{runwayMonths !== null ? "Months of typical spending covered" : "Needs about a month of history"}</div>{yoyDeltaPct !== null && <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{yoyDeltaPct >= 0 ? "↑" : "↓"} {Math.abs(yoyDeltaPct)}% vs same months last year</div>}</div>
      </div>

      {/* Customizable canvas — vertical stack, drag-handle reorder, pretty on mobile & desktop */}
      <ReportCanvas memberId={memberId} />

      {/* Tail hint for mobile */}
      <div className="muted" style={{ textAlign: "center", fontSize: 11, marginTop: 18, padding: "0 12px", lineHeight: 1.5 }}>
        Tip: on mobile, use the grip to drag and the ↑↓ buttons to nudge. Each widget&apos;s <span style={{ color: "var(--ink)", fontWeight: 600 }}>⋯ → Edit</span> lets you change data slice and chart type.
      </div>
    </div>
  );
}

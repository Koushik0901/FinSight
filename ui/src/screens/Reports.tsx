import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type ReportData } from "../api/client";
import { money } from "../utils/format";
import { useMonthTotals } from "../api/hooks/reports";
import { useNetWorth, useNetWorthHistory } from "../api/hooks/networth";
import NetWorthChart from "../components/NetWorthChart";

type Scope = "month" | "quarter" | "year" | "all";
type Tab = "overview" | "networth" | "spending";

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

function useReportData(scope: Scope) {
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

const SCOPE_DAYS: Record<Scope, number> = { month: 30, quarter: 90, year: 365, all: 36500 };

export default function Reports() {
  const [scope, setScope] = useState<Scope>("month");
  const [tab, setTab] = useState<Tab>("overview");
  const { data, isLoading, error } = useReportData(scope);
  const { data: totals } = useMonthTotals();
  const netWorth = useNetWorth();
  const { data: nwHistory = [] } = useNetWorthHistory(SCOPE_DAYS[scope]);

  const monthly = data?.monthly ?? [];
  const monthlyLastYear = data?.monthlyLastYear ?? [];
  const totalIncome = monthly.reduce((sum, month) => sum + month.incomeCents, 0);
  const totalExpense = monthly.reduce((sum, month) => sum + month.expenseCents, 0);
  const totalExpenseLastYear = monthlyLastYear.reduce((sum, month) => sum + month.expenseCents, 0);
  const yoyDeltaPct = totalExpenseLastYear > 0 ? Math.round(((totalExpense - totalExpenseLastYear) / totalExpenseLastYear) * 100) : null;
  const savingsRate = totalIncome > 0 ? Math.round(((totalIncome - totalExpense) / totalIncome) * 100) : 0;
  const runwayMonths = totals?.expenseCents ? Math.max(0, Math.round(netWorth / Math.max(totals.expenseCents, 1))) : 0;
  const chartValues = monthly.slice(-6);
  const maxExpense = Math.max(1, ...chartValues.map((month) => month.expenseCents));
  const topCategoriesByAmount = useMemo(() => [...(data?.topCategories ?? [])].sort((a, b) => b.totalCents - a.totalCents), [data]);
  const maxCategoryAmount = Math.max(1, ...topCategoriesByAmount.map((category) => category.totalCents));

  const scopeLabel = useMemo(() => {
    if (scope === "quarter") return "Quarter";
    if (scope === "year") return "Year";
    if (scope === "all") return "All-time";
    return new Date().toLocaleDateString("en-US", { month: "long", year: "numeric" });
  }, [scope]);

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
  if (error) return <div className="stub" role="alert">Error loading reports.</div>;

  return (
    <div className="screen screen-reports">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Reports · {scopeLabel}</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>How money is moving.</h1>
          <div className="muted" style={{ marginTop: 6 }}>See the shape of your money over time.</div>
        </div>
        <div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}>
          <div className="toolbar">
            <button className={scope === "month" ? "on" : ""} type="button" onClick={() => setScope("month")}>Month</button>
            <button className={scope === "quarter" ? "on" : ""} type="button" onClick={() => setScope("quarter")}>Quarter</button>
            <button className={scope === "year" ? "on" : ""} type="button" onClick={() => setScope("year")}>Year</button>
            <button className={scope === "all" ? "on" : ""} type="button" onClick={() => setScope("all")}>All time</button>
          </div>
          <button className="btn outline sm" type="button" onClick={handleExport}>Export</button>
        </div>
      </div>

      <div className="toolbar" style={{ marginBottom: 16 }}>
        <button className={tab === "overview" ? "on" : ""} type="button" onClick={() => setTab("overview")}>Monthly overview</button>
        <button className={tab === "networth" ? "on" : ""} type="button" onClick={() => setTab("networth")}>Net worth</button>
        <button className={tab === "spending" ? "on" : ""} type="button" onClick={() => setTab("spending")}>Spending deep dive</button>
      </div>

      <div className="stat-row">
        <div className="stat"><div className="label">Savings rate</div><div className="value">{savingsRate}%</div><div className="sub">Income vs. spend</div></div>
        <div className="stat"><div className="label">Net worth</div><div className="value money">{money(netWorth, { currency: "USD" })}</div><div className="sub">Tracked balances</div></div>
        <div className="stat"><div className="label">Spent this month</div><div className="value money">{money(totals?.expenseCents ?? totalExpense, { currency: "USD" })}</div><div className="sub">Cash outflow</div></div>
        <div className="stat accent"><div className="label">Runway</div><div className="value">{runwayMonths}</div><div className="sub">Months at current burn</div></div>
      </div>

      {tab === "overview" && (
        <div className="bigchart">
          <div className="bigchart-head">
            <div>
              <div className="eyebrow">Monthly overview</div>
              <div className="h3" style={{ marginTop: 6 }}>Income and expenses over time</div>
              {yoyDeltaPct !== null && (
                <div className="muted" style={{ fontSize: 12.5, marginTop: 4 }}>
                  {money(totalExpense, { currency: "USD" })} spent this period · {yoyDeltaPct >= 0 ? "up" : "down"} {Math.abs(yoyDeltaPct)}% vs the same months last year ({money(totalExpenseLastYear, { currency: "USD" })})
                </div>
              )}
            </div>
          </div>
          <div style={{ padding: "0 22px 22px" }}>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(6, minmax(0, 1fr))", gap: 12, alignItems: "end", minHeight: 220 }}>
              {chartValues.map((month) => (
                <div key={month.month} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10 }}>
                  <div style={{ width: "100%", minHeight: 160, display: "flex", alignItems: "end", justifyContent: "center", gap: 8 }}>
                    <span style={{ width: 28, height: `${(month.incomeCents / Math.max(maxExpense, month.incomeCents, 1)) * 160}px`, borderRadius: 10, background: "var(--positive)" }} />
                    <span style={{ width: 28, height: `${(month.expenseCents / maxExpense) * 160}px`, borderRadius: 10, background: "var(--negative)" }} />
                  </div>
                  <span className="mono muted">{month.label}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {tab === "networth" && (
        <NetWorthChart points={nwHistory} rangeLabel={scope === "month" ? "month" : scope === "quarter" ? "3 months" : scope === "year" ? "year" : "all time"} />
      )}

      {tab === "spending" && (
        <div className="bigchart">
          <div className="bigchart-head">
            <div>
              <div className="eyebrow">Spending deep dive</div>
              <div className="h3" style={{ marginTop: 6 }}>Where it concentrates, this period</div>
            </div>
          </div>
          <div style={{ padding: "0 22px 22px", display: "flex", flexDirection: "column", gap: 10 }}>
            {topCategoriesByAmount.length === 0 ? (
              <div className="muted" style={{ padding: "18px 0" }}>No categorized spending in this period yet.</div>
            ) : topCategoriesByAmount.map((category) => (
              <div key={category.categoryId} style={{ display: "grid", gridTemplateColumns: "140px 1fr auto", gap: 12, alignItems: "center" }}>
                <span className="row row-sm" style={{ minWidth: 0 }}><span className="cswatch" style={{ background: category.color || "var(--accent)" }} /><span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{category.label}</span></span>
                <div style={{ height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                  <div style={{ width: `${(category.totalCents / maxCategoryAmount) * 100}%`, height: "100%", background: category.color || "var(--accent)", borderRadius: 999 }} />
                </div>
                <span className="money" style={{ fontSize: 13 }}>{money(category.totalCents, { currency: "USD" })}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="section" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18 }}>
        <div className="card flush">
          <div className="card-head"><div className="h3">Top categories</div></div>
          <table className="tbl">
            <thead><tr><th>Category</th><th className="right">Amount</th><th className="right">Txns</th></tr></thead>
            <tbody>
              {(data?.topCategories ?? []).map((category) => (
                <tr key={category.categoryId}><td><div className="row row-sm"><span className="cswatch" style={{ background: category.color || "var(--accent)" }} /><span>{category.label}</span></div></td><td className="right"><span className="money">{money(category.totalCents, { currency: "USD" })}</span></td><td className="right">{category.txnCount}</td></tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="card flush">
          <div className="card-head"><div className="h3">Top merchants</div></div>
          <table className="tbl">
            <thead><tr><th>Merchant</th><th>Category</th><th className="right">Amount</th><th className="right">Txns</th></tr></thead>
            <tbody>
              {(data?.topMerchants ?? []).map((merchant) => (
                <tr key={merchant.merchantRaw}>
                  <td>{merchant.merchantRaw}</td>
                  <td><span className="row row-sm"><span className="cswatch" style={{ background: merchant.categoryColor || "var(--ink-faint)" }} />{merchant.categoryLabel || "Uncategorized"}</span></td>
                  <td className="right"><span className="money">{money(merchant.totalCents, { currency: "USD" })}</span></td>
                  <td className="right">{merchant.txnCount}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

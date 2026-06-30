import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type ReportData } from "../api/client";
import { money } from "../utils/format";
import { useMonthTotals } from "../api/hooks/reports";
import { useNetWorth } from "../api/hooks/networth";

type Scope = "month" | "quarter" | "year" | "all";
type Tab = "overview" | "networth" | "spending";

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

export default function Reports() {
  const [scope, setScope] = useState<Scope>("month");
  const [tab, setTab] = useState<Tab>("overview");
  const { data, isLoading, error } = useReportData(scope);
  const { data: totals } = useMonthTotals();
  const netWorth = useNetWorth();

  const monthly = data?.monthly ?? [];
  const totalIncome = monthly.reduce((sum, month) => sum + month.incomeCents, 0);
  const totalExpense = monthly.reduce((sum, month) => sum + month.expenseCents, 0);
  const savingsRate = totalIncome > 0 ? Math.round(((totalIncome - totalExpense) / totalIncome) * 100) : 0;
  const runwayMonths = totals?.expenseCents ? Math.max(0, Math.round(netWorth / Math.max(totals.expenseCents, 1))) : 0;
  const chartValues = monthly.slice(-6);
  const maxExpense = Math.max(1, ...chartValues.map((month) => month.expenseCents));

  const scopeLabel = useMemo(() => {
    if (scope === "quarter") return "QUARTER";
    if (scope === "year") return "YEAR";
    if (scope === "all") return "ALL-TIME";
    return new Date().toLocaleDateString("en-US", { month: "long", year: "numeric" }).toUpperCase();
  }, [scope]);

  if (isLoading) return <div className="stub">Loading reports…</div>;
  if (error) return <div className="stub" role="alert">Error loading reports.</div>;

  return (
    <div className="screen screen-reports">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />REPORTS · {scopeLabel}</div>
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
          <button className="btn outline sm" type="button">Export</button>
          <button className="btn sm" type="button">Customize</button>
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

      <div className="bigchart">
        <div className="bigchart-head">
          <div>
            <div className="eyebrow">{tab === "overview" ? "MONTHLY OVERVIEW" : tab === "networth" ? "NET WORTH" : "SPENDING DEEP DIVE"}</div>
            <div className="h3" style={{ marginTop: 6 }}>{tab === "overview" ? "Income and expenses over time" : tab === "networth" ? "Balance momentum" : "Expense concentration"}</div>
          </div>
        </div>
        <div style={{ padding: "0 22px 22px" }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(6, minmax(0, 1fr))", gap: 12, alignItems: "end", minHeight: 220 }}>
            {chartValues.map((month) => (
              <div key={month.month} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10 }}>
                <div style={{ width: "100%", minHeight: 160, display: "flex", alignItems: "end", justifyContent: "center", gap: 8 }}>
                  {tab !== "networth" && <span style={{ width: 28, height: `${(month.incomeCents / Math.max(maxExpense, month.incomeCents, 1)) * 160}px`, borderRadius: 10, background: "var(--positive)" }} />}
                  <span style={{ width: 28, height: `${(month.expenseCents / maxExpense) * 160}px`, borderRadius: 10, background: tab === "spending" ? "var(--accent)" : "var(--negative)" }} />
                </div>
                <span className="mono muted">{month.label}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

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
            <thead><tr><th>Merchant</th><th className="right">Amount</th><th className="right">Txns</th></tr></thead>
            <tbody>
              {(data?.topMerchants ?? []).map((merchant) => (
                <tr key={merchant.merchantRaw}><td>{merchant.merchantRaw}</td><td className="right"><span className="money">{money(merchant.totalCents, { currency: "USD" })}</span></td><td className="right">{merchant.txnCount}</td></tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

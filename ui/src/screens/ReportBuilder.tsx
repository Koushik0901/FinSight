import { useState } from "react";
import { useCustomReport } from "../api/hooks/reports";
import type { CustomReportParams } from "../api/openapiClient";
import { money } from "../utils/format";

const SPLIT_OPTIONS: { value: CustomReportParams["splitBy"]; label: string }[] = [
  { value: "category", label: "Category" },
  { value: "group", label: "Group" },
  { value: "payee", label: "Payee" },
  { value: "account", label: "Account" },
  { value: "month", label: "Month" },
];

const PERIOD_OPTIONS: { value: CustomReportParams["period"]; label: string }[] = [
  { value: "Last1Month", label: "Last 1 month" },
  { value: "Last3Months", label: "Last 3 months" },
  { value: "Last6Months", label: "Last 6 months" },
  { value: "YTD", label: "YTD" },
  { value: "All", label: "All time" },
];

export default function ReportBuilder() {
  const [params, setParams] = useState<CustomReportParams>({
    splitBy: "category",
    period: "Last6Months",
    includeTransfers: false,
    includeArchived: false,
  });
  const { data, isLoading, error, refetch } = useCustomReport(params);

  const maxTotal = Math.max(1, ...(data?.rows.map((r) => r.totalCents) ?? [0]));

  return (
    <div className="screen screen-report-builder">
      <div className="card" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 12 }}>
        <h2 className="h3" style={{ margin: 0 }}>Custom Report Builder</h2>
        <p className="muted" style={{ margin: 0, fontSize: 13 }}>
          Slice spending by any dimension — the same transfer-aware totals the fixed reports use.
        </p>
        <div className="toolbar" style={{ flexWrap: "wrap" }}>
          <label className="muted" style={{ fontSize: 12 }}>
            Split by{" "}
            <select
              value={params.splitBy}
              onChange={(e) => setParams({ ...params, splitBy: e.target.value as CustomReportParams["splitBy"] })}
              aria-label="Split by"
            >
              {SPLIT_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>
          <label className="muted" style={{ fontSize: 12 }}>
            Period{" "}
            <select
              value={params.period}
              onChange={(e) => setParams({ ...params, period: e.target.value as CustomReportParams["period"] })}
              aria-label="Period"
            >
              {PERIOD_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>
          <label className="row row-sm" style={{ fontSize: 12 }}>
            <input
              type="checkbox"
              checked={params.includeTransfers}
              onChange={(e) => setParams({ ...params, includeTransfers: e.target.checked })}
            />{" "}
            Include transfers
          </label>
          <label className="row row-sm" style={{ fontSize: 12 }}>
            <input
              type="checkbox"
              checked={params.includeArchived}
              onChange={(e) => setParams({ ...params, includeArchived: e.target.checked })}
            />{" "}
            Include archived
          </label>
        </div>
        {error && (
          <div role="alert" className="muted">
            Could not load report. <button className="btn outline sm" onClick={() => void refetch()}>Try again</button>
          </div>
        )}
        {isLoading ? (
          <div className="stub">Loading custom report…</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div className="eyebrow">Total {money(data?.totalCents ?? 0)} across {data?.rows.length ?? 0} groups</div>
            {(data?.rows.length ?? 0) === 0 ? (
              <div className="muted" style={{ padding: "18px 0" }}>No transactions match this slice.</div>
            ) : (
              data?.rows.map((row, idx) => (
                <div key={row.label} style={{ display: "grid", gridTemplateColumns: "140px 1fr auto", gap: 12, alignItems: "center" }}>
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{row.label}</span>
                  <div style={{ height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                    <div
                      className="plot-grow-x pb-fill"
                      style={{
                        width: `${(row.totalCents / maxTotal) * 100}%`,
                        height: "100%",
                        background: "var(--accent)",
                        borderRadius: 999,
                        animationDelay: `${Math.min(idx * 30, 200)}ms`,
                      }}
                    />
                  </div>
                  <span className="money" style={{ fontSize: 13 }}>{money(row.totalCents)} · {row.txnCount} txns</span>
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}

export function ReportBuilderCard() {
  // Thin wrapper for embedding inside Reports.tsx without a full screen navigation
  return <ReportBuilder />;
}

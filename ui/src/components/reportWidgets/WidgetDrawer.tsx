import { useEffect, useState } from "react";
import Drawer from "../Drawer";
import { useCreateReportWidget, useUpdateReportWidget } from "../../api/hooks/reportWidgets";
import type { ReportWidget } from "../../api/hooks/reportWidgets";
import { useAccounts } from "../../api/hooks/accounts";
import { useCategories, useCategoryGroups } from "../../api/hooks/transactions";
import { toast } from "sonner";

function AccountsFilter({ accountIds, onChange }: { accountIds: string[]; onChange: (ids: string[]) => void }) {
  const { data: accounts = [] } = useAccounts();
  if (accounts.length === 0) return <span className="muted" style={{ fontSize: 12 }}>No accounts</span>;
  return (
    <>
      {accounts.map((a) => (
        <label key={a.id} className="row row-sm" style={{ gap: 8, cursor: "pointer", fontSize: 12 }}>
          <input
            type="checkbox"
            checked={accountIds.includes(a.id)}
            onChange={(e) => {
              if (e.target.checked) onChange([...accountIds, a.id]);
              else onChange(accountIds.filter((id) => id !== a.id));
            }}
          />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{a.name}</span>
        </label>
      ))}
    </>
  );
}

function CategoriesFilter({ categoryIds, onChange }: { categoryIds: string[]; onChange: (ids: string[]) => void }) {
  const { data: categories = [] } = useCategories();
  if (categories.length === 0) return <span className="muted" style={{ fontSize: 12 }}>No categories</span>;
  return (
    <>
      {categories.map((c) => (
        <label key={c.id} className="row row-sm" style={{ gap: 8, cursor: "pointer", fontSize: 12 }}>
          <input
            type="checkbox"
            checked={categoryIds.includes(c.id)}
            onChange={(e) => {
              if (e.target.checked) onChange([...categoryIds, c.id]);
              else onChange(categoryIds.filter((id) => id !== c.id));
            }}
          />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.label}</span>
        </label>
      ))}
    </>
  );
}

type Props = {
  open: boolean;
  onClose: () => void;
  editing: ReportWidget | null;
};

const CHART_OPTIONS: { value: string; label: string }[] = [
  { value: "table", label: "Table" },
  { value: "bar", label: "Bar" },
  { value: "barStacked", label: "Stacked bar" },
  { value: "line", label: "Line" },
  { value: "area", label: "Area" },
  { value: "donut", label: "Donut" },
];

const SPLIT_OPTIONS: { value: string; label: string }[] = [
  { value: "category", label: "Category" },
  { value: "group", label: "Group" },
  { value: "payee", label: "Payee" },
  { value: "account", label: "Account" },
  { value: "month", label: "Month" },
  { value: "spendingType", label: "Spending type" },
];

const PERIOD_OPTIONS: { value: string; label: string }[] = [
  { value: "Last1Month", label: "Last 1 month" },
  { value: "Last3Months", label: "Last 3 months" },
  { value: "Last6Months", label: "Last 6 months" },
  { value: "YTD", label: "YTD" },
  { value: "All", label: "All time" },
];

const INTERVAL_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "Default" },
  { value: "day", label: "Day" },
  { value: "week", label: "Week" },
  { value: "month", label: "Month" },
  { value: "year", label: "Year" },
];

const METRIC_OPTIONS: { value: string; label: string }[] = [
  { value: "sum", label: "Sum" },
  { value: "count", label: "Count" },
  { value: "average", label: "Average" },
];

function parseFilters(json: string) {
  try {
    const j = JSON.parse(json || "{}");
    return {
      includeTransfers: Boolean(j.includeTransfers ?? j.include_transfers ?? false),
      includeArchived: Boolean(j.includeArchived ?? j.include_archived ?? false),
      accountIds: Array.isArray(j.accountIds) ? j.accountIds as string[] : Array.isArray(j.account_ids) ? j.account_ids as string[] : [],
      categoryIds: Array.isArray(j.categoryIds) ? j.categoryIds as string[] : Array.isArray(j.category_ids) ? j.category_ids as string[] : [],
      payee: (j.payee as string | undefined) ?? null,
      spendingType: (j.spendingType as string | undefined) ?? (j.spending_type as string | undefined) ?? null,
      minAmount: j.minAmount ?? j.min_amount_cents ?? j.minAmountCents ?? null,
      maxAmount: j.maxAmount ?? j.max_amount_cents ?? j.maxAmountCents ?? null,
    };
  } catch {
    return { includeTransfers: false, includeArchived: false, accountIds: [], categoryIds: [], payee: null, spendingType: null, minAmount: null, maxAmount: null };
  }
}

export default function WidgetDrawer({ open, onClose, editing }: Props) {
  const create = useCreateReportWidget();
  const update = useUpdateReportWidget();

  const [title, setTitle] = useState("");
  const [chartType, setChartType] = useState("bar");
  const [splitBy, setSplitBy] = useState("category");
  const [period, setPeriod] = useState("All");
  const [interval, setInterval] = useState("");
  const [metric, setMetric] = useState("sum");
  const [includeTransfers, setIncludeTransfers] = useState(false);
  const [includeArchived, setIncludeArchived] = useState(false);
  const [accountIds, setAccountIds] = useState<string[]>([]);
  const [categoryIds, setCategoryIds] = useState<string[]>([]);
  const [payee, setPayee] = useState("");
  const [spendingType, setSpendingType] = useState("");
  const [minAmount, setMinAmount] = useState("");
  const [maxAmount, setMaxAmount] = useState("");
  useEffect(() => {
    if (editing) {
      setTitle(editing.title);
      setChartType(editing.chartType);
      setSplitBy(editing.splitBy);
      setPeriod(editing.period);
      const f = parseFilters(editing.filtersJson);
      setIncludeTransfers(f.includeTransfers);
      setIncludeArchived(f.includeArchived);
      setAccountIds(f.accountIds ?? []);
      setCategoryIds(f.categoryIds ?? []);
      setPayee((f.payee as string) ?? "");
      setSpendingType((f.spendingType as string) ?? "");
      setMinAmount(f.minAmount != null ? String(f.minAmount) : "");
      setMaxAmount(f.maxAmount != null ? String(f.maxAmount) : "");
      // Interval and metric are stored as top-level widget fields, not in filtersJson
      // For now, we store them in filtersJson as well for simplicity
      const jf = JSON.parse(editing.filtersJson || "{}");
      setInterval(jf.interval ?? "");
      setMetric(jf.metric ?? "sum");
    } else {
      setTitle("");
      setChartType("bar");
      setSplitBy("category");
      setPeriod("All");
      setInterval("");
      setMetric("sum");
      setIncludeTransfers(false);
      setIncludeArchived(false);
      setAccountIds([]);
      setCategoryIds([]);
      setPayee("");
      setSpendingType("");
      setMinAmount("");
      setMaxAmount("");
    }
  }, [editing, open]);

  const handleSave = async () => {
    const t = title.trim();
    if (!t) {
      toast.error("Title is required");
      return;
    }
    const filtersJson = JSON.stringify({
      includeTransfers,
      includeArchived,
      accountIds: accountIds.length ? accountIds : undefined,
      categoryIds: categoryIds.length ? categoryIds : undefined,
      payee: payee.trim() || undefined,
      spendingType: spendingType || undefined,
      minAmount: minAmount ? Number(minAmount) : undefined,
      maxAmount: maxAmount ? Number(maxAmount) : undefined,
      interval: interval || undefined,
      metric: metric !== "sum" ? metric : undefined,
    });
    try {
      if (editing) {
        await update.mutateAsync({
          id: editing.id,
          title: t,
          chartType,
          splitBy,
          period,
          filtersJson,
        });
        toast.success("Widget updated");
      } else {
        await create.mutateAsync({
          title: t,
          chartType,
          splitBy,
          period,
          filtersJson,
          position: null,
        });
        toast.success("Widget added");
      }
      onClose();
    } catch (e) {
      toast.error("Could not save", { description: String(e) });
    }
  };

  const isSaving = create.isPending || update.isPending;

  return (
    <Drawer open={open} onClose={onClose} title={editing ? "Edit widget" : "Add widget"} width={420}>
      <div style={{ display: "flex", flexDirection: "column", gap: 18, padding: "4px 0 12px" }}>
        <p className="muted" style={{ margin: 0, fontSize: 13, lineHeight: 1.5 }}>
          {editing ? "Change how this widget slices your ledger." : "Pick any slice of your ledger and how it should plot. Drag to reorder later."}
        </p>

        <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span className="eyebrow" style={{ fontSize: 11 }}>Title</span>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="e.g. Spending by category"
            maxLength={120}
            style={{
              padding: "10px 12px",
              borderRadius: 10,
              border: "1px solid var(--line)",
              background: "var(--elevated)",
              fontSize: 14,
            }}
          />
        </label>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
          <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Chart</span>
            <select
              value={chartType}
              onChange={(e) => setChartType(e.target.value)}
              aria-label="Chart"
              style={{ padding: "10px 12px", borderRadius: 10, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 14 }}
            >
              {CHART_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </label>

          <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Group by</span>
            <select
              value={splitBy}
              onChange={(e) => setSplitBy(e.target.value)}
              aria-label="Group by"
              style={{ padding: "10px 12px", borderRadius: 10, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 14 }}
            >
              {SPLIT_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </label>
        </div>

        <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span className="eyebrow" style={{ fontSize: 11 }}>Period</span>
          <select
            value={period}
            onChange={(e) => setPeriod(e.target.value)}
            aria-label="Period"
            style={{ padding: "10px 12px", borderRadius: 10, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 14 }}
          >
            {PERIOD_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
          </select>
        </label>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
          <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Interval</span>
            <select
              value={interval}
              onChange={(e) => setInterval(e.target.value)}
              aria-label="Interval"
              style={{ padding: "10px 12px", borderRadius: 10, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 14 }}
            >
              {INTERVAL_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </label>
          <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Metric</span>
            <select
              value={metric}
              onChange={(e) => setMetric(e.target.value)}
              aria-label="Metric"
              style={{ padding: "10px 12px", borderRadius: 10, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 14 }}
            >
              {METRIC_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </label>
        </div>
        <fieldset style={{ border: "1px solid var(--line)", borderRadius: 12, padding: 12, display: "flex", flexDirection: "column", gap: 12 }}>
          <legend className="eyebrow" style={{ fontSize: 11, padding: "0 6px" }}>Filters</legend>
          <label className="row row-sm" style={{ gap: 8, cursor: "pointer", fontSize: 13 }}>
            <input type="checkbox" checked={includeTransfers} onChange={(e) => setIncludeTransfers(e.target.checked)} />
            Include transfers
          </label>
          <label className="row row-sm" style={{ gap: 8, cursor: "pointer", fontSize: 13 }}>
            <input type="checkbox" checked={includeArchived} onChange={(e) => setIncludeArchived(e.target.checked)} />
            Include archived
          </label>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Accounts</span>
            <div style={{ maxHeight: 100, overflowY: "auto", border: "1px solid var(--line)", borderRadius: 8, padding: 8, display: "flex", flexDirection: "column", gap: 6, background: "var(--elevated)" }}>
              <AccountsFilter accountIds={accountIds} onChange={setAccountIds} />
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Categories</span>
            <div style={{ maxHeight: 100, overflowY: "auto", border: "1px solid var(--line)", borderRadius: 8, padding: 8, display: "flex", flexDirection: "column", gap: 6, background: "var(--elevated)" }}>
              <CategoriesFilter categoryIds={categoryIds} onChange={setCategoryIds} />
            </div>
          </div>
          <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Payee contains</span>
            <input value={payee} onChange={(e) => setPayee(e.target.value)} placeholder="e.g. Whole Foods" style={{ padding: "8px 10px", borderRadius: 8, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 13 }} />
          </label>
          <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span className="eyebrow" style={{ fontSize: 11 }}>Spending type</span>
            <select value={spendingType} onChange={(e) => setSpendingType(e.target.value)} style={{ padding: "8px 10px", borderRadius: 8, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 13 }}>
              <option value="">Any</option>
              <option value="Need">Need</option>
              <option value="Want">Want</option>
              <option value="Saving">Saving</option>
              <option value="Investment">Investment</option>
            </select>
          </label>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
            <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <span className="eyebrow" style={{ fontSize: 11 }}>Min amount ($)</span>
              <input type="number" value={minAmount} onChange={(e) => setMinAmount(e.target.value)} placeholder="0.00" style={{ padding: "8px 10px", borderRadius: 8, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 13 }} />
            </label>
            <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <span className="eyebrow" style={{ fontSize: 11 }}>Max amount ($)</span>
              <input type="number" value={maxAmount} onChange={(e) => setMaxAmount(e.target.value)} placeholder="1000.00" style={{ padding: "8px 10px", borderRadius: 8, border: "1px solid var(--line)", background: "var(--elevated)", fontSize: 13 }} />
            </label>
          </div>
        </fieldset>

        {/* Live preview hint */}
        <div style={{ background: "var(--surface-2)", borderRadius: 12, padding: 12, display: "flex", gap: 8, alignItems: "center" }}>
          <span style={{ width: 28, height: 28, borderRadius: 8, background: "var(--accent)", display: "inline-flex", alignItems: "center", justifyContent: "center", fontSize: 14, flexShrink: 0 }}>◈</span>
          <span className="muted" style={{ fontSize: 12, lineHeight: 1.4 }}>
            {chartType === "table" ? "Rows as a ranked list with bars." : chartType === "donut" ? "Share of total per group." : `Grouped by ${splitBy} over ${period}.`}
          </span>
        </div>

        <div style={{ display: "flex", gap: 10, justifyContent: "flex-end", marginTop: 4 }}>
          <button className="btn outline" type="button" onClick={onClose} disabled={isSaving}>Cancel</button>
          <button className="btn primary" type="button" onClick={handleSave} disabled={isSaving || !title.trim()}>
            {isSaving ? "Saving…" : editing ? "Save changes" : "Add widget"}
          </button>
        </div>
      </div>
    </Drawer>
  );
}

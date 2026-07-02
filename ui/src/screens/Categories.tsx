import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useCategoriesWithSpending, useSetCategorySpendingType, useUpdateCategoryColor } from "../api/hooks/transactions";
import type { CategoryWithSpending } from "../api/client";
import { money } from "../utils/format";
import { DEFAULT_CATEGORY_COLOR, iconFor } from "../utils/categoryColor";
import Swatch from "../components/Swatch";

type Scope = "month" | "avg" | "year";

const COLOR_CHOICES = [
  "#A78BFA", "#34D399", "#FB923C", "#60A5FA", "#FACC15",
  "#F472B6", "#2DD4BF", "#FCA5A5", "#818CF8", "#FDE68A",
  DEFAULT_CATEGORY_COLOR,
];

const SPENDING_TYPE_OPTIONS = [
  { value: "fixed", label: "Fixed" },
  { value: "investments", label: "Investments" },
  { value: "savings", label: "Savings" },
  { value: "guilt_free", label: "Guilt-free" },
  { value: "", label: "Untagged" },
] as const;

function valueFor(category: CategoryWithSpending, scope: Scope) {
  if (scope === "avg") return Math.round((category.thisMonthCents + category.lastMonthCents) / 2);
  if (scope === "year") return category.yearTotalCents;
  return category.thisMonthCents;
}

function compareFor(category: CategoryWithSpending, scope: Scope) {
  if (scope === "avg") return category.thisMonthCents;
  return category.lastMonthCents;
}

function txnCountFor(category: CategoryWithSpending, scope: Scope) {
  return scope === "year" ? category.yearTxnCount : category.txnCount;
}

export default function Categories() {
  const [scope, setScope] = useState<Scope>("month");
  const { data: categories = [], isLoading, error } = useCategoriesWithSpending();
  const setSpendingType = useSetCategorySpendingType();
  const updateColor = useUpdateCategoryColor();
  const [savingId, setSavingId] = useState<string | null>(null);
  const [openColorId, setOpenColorId] = useState<string | null>(null);

  const monthLabel = new Date().toLocaleDateString("en-US", { month: "long", year: "numeric" });
  const prevMonthLabel = new Date(new Date().getFullYear(), new Date().getMonth() - 1, 1).toLocaleDateString("en-US", { month: "long" });
  const valueLabel = scope === "year" ? "Year total" : scope === "avg" ? "2-mo average" : "This month";
  const compareLabel = scope === "year" ? null : scope === "avg" ? "This month" : prevMonthLabel;

  const sorted = useMemo(() => [...categories].sort((a, b) => valueFor(b, scope) - valueFor(a, scope) || a.label.localeCompare(b.label)), [categories, scope]);
  const active = sorted.filter((category) => valueFor(category, scope) > 0 || compareFor(category, scope) > 0 || category.budgetCents > 0);
  const totalThis = active.reduce((sum, category) => sum + valueFor(category, scope), 0);
  const totalCompare = active.reduce((sum, category) => sum + compareFor(category, scope), 0);
  const biggestDrop = active.map((category) => ({ category, delta: valueFor(category, scope) - compareFor(category, scope) })).sort((a, b) => a.delta - b.delta)[0];
  const biggestRise = active.map((category) => ({ category, delta: valueFor(category, scope) - compareFor(category, scope) })).sort((a, b) => b.delta - a.delta)[0];

  const saveSpendingType = async (id: string, spendingType: string) => {
    setSavingId(id);
    try {
      await setSpendingType.mutateAsync({ id, spendingType: spendingType || null });
      toast.success("Saved");
    } catch {
      toast.error("Could not save spending type");
    } finally {
      setSavingId(null);
    }
  };

  const saveColor = async (id: string, color: string) => {
    try {
      await updateColor.mutateAsync({ id, color });
      toast.success("Color updated");
    } catch {
      toast.error("Could not save color");
    }
  };

  if (isLoading) return <div className="stub">Loading categories…</div>;
  if (error) return <div className="stub" role="alert">Error loading categories.</div>;

  return (
    <div className="screen screen-categories">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Categories · {scope === "year" ? "Year" : monthLabel}</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Where the money is going.</h1>
        </div>
        <div className="toolbar" role="tablist" aria-label="Category time scope">
          <button className={scope === "month" ? "on" : ""} type="button" onClick={() => setScope("month")}>This month</button>
          <button className={scope === "avg" ? "on" : ""} type="button" onClick={() => setScope("avg")}>vs. average</button>
          <button className={scope === "year" ? "on" : ""} type="button" onClick={() => setScope("year")}>Year</button>
        </div>
      </div>

      <div className="card" style={{ padding: 28 }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-end", gap: 16, marginBottom: 16 }}>
          <div>
            <div className="eyebrow">{valueLabel}</div>
            <div className="figure money" style={{ fontSize: 48, lineHeight: 1, marginTop: 8 }}>{money(totalThis, { currency: "USD" })}</div>
          </div>
          {compareLabel && (
            <div style={{ textAlign: "right" }}>
              <div className="muted" style={{ fontSize: 13 }}>vs. {compareLabel}</div>
              <div className={`figure money ${totalThis <= totalCompare ? "pos" : "neg"}`} style={{ fontSize: 22, marginTop: 4 }}>
                {totalCompare > 0 ? money(Math.abs(totalThis - totalCompare), { currency: "USD" }) : money(0, { currency: "USD" })}
                {totalCompare > 0 && ` · ${Math.round((Math.abs(totalThis - totalCompare) / totalCompare) * 100)}%`}
              </div>
            </div>
          )}
        </div>

        <div className="stream" style={{ height: 18, borderRadius: 6 }}>
          {active.map((category) => <span key={category.id} title={`${category.label} · ${money(valueFor(category, scope), { currency: "USD" })}`} style={{ width: `${totalThis > 0 ? (valueFor(category, scope) / totalThis) * 100 : 0}%`, background: category.color || "var(--accent)" }} />)}
        </div>

        {biggestDrop && biggestRise && biggestDrop.delta < 0 && biggestRise.delta > 0 && (
          <p className="muted" style={{ marginTop: 18, marginBottom: 0 }}>
            ✦ <strong>{biggestDrop.category.label}</strong> dropped <span className="money">{money(Math.abs(biggestDrop.delta), { currency: "USD" })}</span> — biggest move. <strong>{biggestRise.category.label}</strong> rose by <span className="money">{money(biggestRise.delta, { currency: "USD" })}</span>.
          </p>
        )}
      </div>

      <section className="section">
        <div className="card flush">
          <div className="card-head">
            <div>
              <div className="h3">All categories</div>
              <div className="muted" style={{ fontSize: 13, marginTop: 4 }}>Budget set by you · agent suggests adjustments quarterly</div>
            </div>
          </div>
          <table className="tbl">
            <thead>
              <tr>
                <th>Category</th>
                <th>Pace</th>
                <th className="right">{valueLabel}</th>
                {compareLabel && <th className="right">{compareLabel}</th>}
                <th className="right">Budget</th>
                <th className="right">Transactions</th>
                <th>Type</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((category) => {
                const current = valueFor(category, scope);
                const compare = compareFor(category, scope);
                const budget = category.budgetCents;
                const pct = budget > 0 ? Math.min(100, (current / budget) * 100) : 0;
                const over = budget > 0 && current > budget;
                const colorPickerOpen = openColorId === category.id;
                const CategoryIcon = iconFor(category.id);
                return (
                  <tr key={category.id}>
                    <td>
                      <div className="row row-sm">
                        <button
                          type="button"
                          className="cat-icon-tile"
                          style={{ color: category.color || DEFAULT_CATEGORY_COLOR, cursor: "pointer" }}
                          onClick={() => setOpenColorId(colorPickerOpen ? null : category.id)}
                          aria-label={`Change color for ${category.label}`}
                          aria-expanded={colorPickerOpen}
                        >
                          <CategoryIcon data-testid={`cat-icon-${category.id}`} />
                        </button>
                        <span>{category.label}</span>
                      </div>
                      {colorPickerOpen && (
                        <div className="swatch-row" role="radiogroup" aria-label={`Color for ${category.label}`} style={{ marginTop: 8 }}>
                          {COLOR_CHOICES.map((c) => (
                            <Swatch
                              key={c}
                              color={c}
                              selected={c === (category.color || DEFAULT_CATEGORY_COLOR)}
                              onClick={() => {
                                void saveColor(category.id, c);
                                setOpenColorId(null);
                              }}
                              label={`Choose ${c}`}
                            />
                          ))}
                        </div>
                      )}
                    </td>
                    <td><div className="row row-sm" style={{ alignItems: "center" }}><div className={`goal-bar ${over ? "negative" : ""}`} style={{ width: 180, height: 6 }}><span style={{ width: `${pct}%`, background: over ? "var(--negative)" : category.color || "var(--accent)" }} /></div><span className={`num ${over ? "neg" : "muted"}`} style={{ fontSize: 12 }}>{Math.round(pct)}%</span></div></td>
                    <td className="right"><span className="money">{money(current, { currency: "USD" })}</span></td>
                    {compareLabel && <td className="right"><span className="money muted">{compare > 0 ? money(compare, { currency: "USD" }) : "—"}</span></td>}
                    <td className="right"><span className={`money ${over ? "neg" : "muted"}`}>{budget > 0 ? money(budget, { currency: "USD" }) : "—"}</span></td>
                    <td className="right"><span className="num muted">{txnCountFor(category, scope)}</span></td>
                    <td><select className="control" value={category.spendingType ?? ""} disabled={savingId === category.id} onChange={(e) => void saveSpendingType(category.id, e.target.value)} aria-label={`Spending type for ${category.label}`} style={{ minWidth: 130 }}>{SPENDING_TYPE_OPTIONS.map((option) => <option key={option.value || "untagged"} value={option.value}>{option.label}</option>)}</select></td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

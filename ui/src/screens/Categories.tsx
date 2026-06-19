import { useState } from "react";
import type { ReactNode } from "react";
import { toast } from "sonner";
import { useCategoriesWithSpending, useSetCategorySpendingType } from "../api/hooks/transactions";
import type { CategoryWithSpending } from "../api/client";
import { money } from "../utils/format";
import Card from "../components/Card";
import Table from "../components/Table";
import { TableHead, TableBody, TableRow, TableHeader, TableCell } from "../components/Table";
import Badge from "../components/Badge";
import ProgressBar from "../components/ProgressBar";

function PaceBar({ value, compare, color, label }: { value: number; compare: number; color: string; label: string }) {
  const max = Math.max(value, compare, 1);
  const pct = Math.min(120, (value / max) * 100);
  const over = value > compare && compare > 0;
  return (
    <div className="row row-sm">
      <div style={{ flex: 1, maxWidth: 180 }}>
        <div style={{ "--accent": color } as React.CSSProperties}>
          <ProgressBar
            value={Math.round(pct)}
            max={100}
            size="sm"
            tone={over ? "negative" : "default"}
            aria-label={`Pace for ${label}`}
          />
        </div>
      </div>
      <span className={`num tabular ${over ? "neg" : "muted"}`} style={{ fontSize: 12, minWidth: 32 }}>
        {Math.round(pct)}%
      </span>
    </div>
  );
}

const SPENDING_TYPE_OPTIONS = [
  { value: "fixed", label: "Fixed", background: "var(--ink-mute)", color: "var(--bg)", border: "var(--ink-mute)" },
  { value: "investments", label: "Investments", background: "var(--accent)", color: "var(--bg)", border: "var(--accent)" },
  { value: "savings", label: "Savings", background: "#34D399", color: "var(--bg)", border: "#34D399" },
  { value: "guilt_free", label: "Guilt-free", background: "#FB923C", color: "var(--bg)", border: "#FB923C" },
  { value: "", label: "Untagged", background: "transparent", color: "var(--ink-mute)", border: "var(--line)" },
] as const;

function spendingTypeStyle(spendingType: string | null | undefined) {
  const selected = SPENDING_TYPE_OPTIONS.find((option) => option.value === (spendingType ?? "")) ?? SPENDING_TYPE_OPTIONS[4];
  return {
    background: selected.background,
    color: selected.color,
    border: selected.value
      ? `1px solid ${selected.border}`
      : `1px dashed ${selected.border}`,
  };
}

export default function Categories() {
  const [scope, setScope] = useState<"month" | "avg" | "year">("month");
  const { data: cats = [], isLoading, error } = useCategoriesWithSpending();
  const setSpendingType = useSetCategorySpendingType();
  const [savingId, setSavingId] = useState<string | null>(null);

  // Filter to non-zero categories and sort by spend desc
  const active = cats
    .filter((c) =>
      c.thisMonthCents > 0 ||
      c.lastMonthCents > 0 ||
      (scope === "year" && c.yearTotalCents > 0)
    )
    .sort((a, b) => b.thisMonthCents - a.thisMonthCents);

  const valueFor = (c: CategoryWithSpending) => {
    if (scope === "avg") return Math.round((c.thisMonthCents + c.lastMonthCents) / 2);
    if (scope === "year") return c.yearTotalCents;
    return c.thisMonthCents;
  };
  const compareFor = (c: CategoryWithSpending) =>
    scope === "avg" ? c.thisMonthCents : c.lastMonthCents;
  const sorted = [...cats].sort((a, b) => {
    const delta = valueFor(b) - valueFor(a);
    if (delta !== 0) return delta;
    return a.groupLabel.localeCompare(b.groupLabel) || a.label.localeCompare(b.label);
  });

  const totalThis = active.reduce((s, c) => s + valueFor(c), 0);
  const totalLast = active.reduce((s, c) => s + compareFor(c), 0);

  const delta = totalLast > 0 ? ((totalThis - totalLast) / totalLast) * 100 : 0;

  const now = new Date();
  const monthLabel = now.toLocaleString("default", { month: "long", year: "numeric" });
  const lastMonthLabel = new Date(now.getFullYear(), now.getMonth() - 1, 1)
    .toLocaleString("default", { month: "long" });

  // §6c: AI insight sentence
  const hasLastMonthData = active.some((c) => c.lastMonthCents > 0);
  let insightJSX: ReactNode = null;
  if (scope === "month" && hasLastMonthData && active.length >= 2) {
    const withDelta = active.map((c) => ({ ...c, delta: c.thisMonthCents - c.lastMonthCents }));
    const topGainer = withDelta.reduce((best, c) => c.delta < best.delta ? c : best);
    const topRiser  = withDelta.reduce((best, c) => c.delta > best.delta ? c : best);
    if (topGainer.delta < 0 && topRiser.delta > 0) {
      insightJSX = (
        <div className="muted" style={{ fontSize: 13, fontStyle: "italic", marginBottom: 12 }}>
          ✦ <strong>{topGainer.label}</strong> dropped {money(Math.abs(topGainer.delta))} — biggest improvement.{" "}
          <strong>{topRiser.label}</strong> rose by {money(topRiser.delta)}.
        </div>
      );
    }
  }

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        Loading categories…
      </div>
    );
  }
  if (error) {
    return (
      <div className="stub" role="alert" aria-live="assertive">
        Error loading categories.
      </div>
    );
  }

  const saveSpendingType = async (id: string, next: string) => {
    setSavingId(id);
    try {
      await setSpendingType.mutateAsync({ id, spendingType: next || null });
      toast.success("Saved");
    } catch {
      toast.error("Could not save spending type");
    } finally {
      setSavingId(null);
    }
  };

  return (
    <div className="screen screen-categories">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            Categories · {scope === "avg" ? "trailing average" : monthLabel}
          </div>
          <h1>Where the money is going.</h1>
        </div>
        <div className="toolbar" role="tablist" aria-label="Spending scope">
          <button className={scope === "month" ? "on" : ""} onClick={() => setScope("month")} role="tab" aria-selected={scope === "month"}>
            This month
          </button>
          <button className={scope === "avg" ? "on" : ""} onClick={() => setScope("avg")} role="tab" aria-selected={scope === "avg"}>
            vs. last month
          </button>
          <button className={scope === "year" ? "on" : ""} onClick={() => setScope("year")} role="tab" aria-selected={scope === "year"}>
            Year to date
          </button>
        </div>
      </div>

      {insightJSX}

      {/* Summary card */}
      <Card>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", marginBottom: 14 }}>
          <div>
            <div className="eyebrow" style={{ marginBottom: 6 }}>
              {scope === "avg" ? "Average" : "Total spent"}
            </div>
            <div className="figure money" style={{ fontSize: 44, lineHeight: 1 }}>
              {money(totalThis)}
            </div>
            {active.length === 0 && (
              <div className="muted" style={{ fontSize: 12.5, marginTop: 8 }}>
                No spending yet — tag categories below to prepare your conscious spending split.
              </div>
            )}
          </div>
          {scope !== "year" && totalLast > 0 && (
            <div className="right">
              <div className="muted" style={{ fontSize: 13 }}>vs. {lastMonthLabel}</div>
              <div className={`num ${totalThis < totalLast ? "pos" : "neg"}`} style={{ fontSize: 18 }}>
                {totalThis < totalLast ? "↓" : "↑"}{" "}
                {money(Math.abs(totalLast - totalThis))} · {Math.abs(Math.round(delta))}%
              </div>
            </div>
          )}
        </div>

        {/* Category stream bar */}
        <div className="stream" style={{ height: 18, borderRadius: 6 }}>
          {active.map((c) => (
            <span
              key={c.id}
              title={`${c.label} · ${money(valueFor(c))}`}
              style={{
                width: totalThis > 0 ? `${(valueFor(c) / totalThis) * 100}%` : "0%",
                background: c.color || "var(--ink-faint)",
              }}
            />
          ))}
        </div>
      </Card>

      {/* Full table */}
      <div className="section">
        <Card flush>
          <div className="card-head">
            <div className="h3">All categories</div>
            <div className="row row-sm" style={{ alignItems: "center" }}>
              <span className="muted" style={{ fontSize: 13 }}>
                Sorted by spend
              </span>
              <Badge>{active.length} active</Badge>
            </div>
          </div>
          <Table>
            <TableHead>
              <tr>
                <TableHeader>Category</TableHeader>
                <TableHeader>Spending type</TableHeader>
                <TableHeader>Pace vs. {lastMonthLabel}</TableHeader>
                <TableHeader right>{scope === "avg" ? "Average" : scope === "year" ? "Year to date" : "This month"}</TableHeader>
                <TableHeader right>{lastMonthLabel}</TableHeader>
                <TableHeader right>Transactions</TableHeader>
                <TableHeader right>Budget</TableHeader>
              </tr>
            </TableHead>
            <TableBody>
              {sorted.map((c) => {
                const v = valueFor(c);
                const cmp = scope === "year" ? 0 : compareFor(c);
                const color = c.color || "var(--ink-mute)";
                const style = spendingTypeStyle(c.spendingType);
                return (
                  <TableRow key={c.id}>
                    <TableCell>
                      <div className="row row-sm">
                        <span
                          className="cswatch"
                          style={{
                            background: color + "22",
                            border: `1px solid ${color}44`,
                            width: 18,
                            height: 18,
                            borderRadius: 6,
                          }}
                        />
                        <span>{c.label}</span>
                        <span className="muted" style={{ fontSize: 12 }}>{c.groupLabel}</span>
                      </div>
                    </TableCell>
                    <TableCell>
                      <select
                        className="spending-type-select"
                        value={c.spendingType ?? ""}
                        disabled={savingId === c.id}
                        onChange={(e) => void saveSpendingType(c.id, e.target.value)}
                        aria-label={`Spending type for ${c.label}`}
                        style={{
                          width: "100%",
                          minWidth: 120,
                          borderRadius: 999,
                          padding: "6px 10px",
                          fontSize: 12,
                          fontWeight: 600,
                          outline: "none",
                          cursor: "pointer",
                          ...style,
                        }}
                      >
                        {SPENDING_TYPE_OPTIONS.map((option) => (
                          <option key={option.value || "untagged"} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </TableCell>
                    <TableCell>
                      <PaceBar value={v} compare={cmp} color={color} label={c.label} />
                    </TableCell>
                    <TableCell right>
                      <span className="num tabular money">{money(v)}</span>
                    </TableCell>
                    <TableCell right>
                      <span className="num tabular muted">{cmp > 0 ? money(cmp) : "—"}</span>
                    </TableCell>
                    <TableCell right>
                      <span className="num tabular muted">{c.txnCount}</span>
                    </TableCell>
                    <TableCell right>
                      <span className={`num tabular ${c.budgetCents > 0 && c.thisMonthCents > c.budgetCents ? "neg" : "muted"}`}>
                        {c.budgetCents > 0 ? money(c.budgetCents) : "—"}
                      </span>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Card>
      </div>
    </div>
  );
}

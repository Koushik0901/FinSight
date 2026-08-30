import { useMemo } from "react";
import { useBudgetHistory } from "../../api/hooks/budget";
import { money } from "../../utils/format";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { MobileStat, MobileStatRow } from "../../components/mobile/MobileStat";
import * as I from "../../components/Icons";

export default function MobileReports() {
  const { data: history = [] } = useBudgetHistory(6);
  const monthlyTotals = useMemo(() => {
    if (history.length === 0) return [];
    const months = history[0]?.monthly ?? [];
    return months.map((_, idx) => {
      const label = months[idx]?.label ?? months[idx]?.month.slice(0, 7) ?? `M${idx + 1}`;
      const spent = history.reduce((sum, cat) => sum + (cat.monthly[idx]?.spentCents ?? 0), 0);
      const budgeted = history.reduce((sum, cat) => sum + (cat.monthly[idx]?.budgetedCents ?? 0), 0);
      return { label, spent, budgeted };
    });
  }, [history]);
  const maxSpent = Math.max(...monthlyTotals.map((m) => m.spent), 1);
  const avgSpent = monthlyTotals.length > 0 ? Math.round(monthlyTotals.reduce((s, m) => s + m.spent, 0) / monthlyTotals.length) : 0;
  const last = monthlyTotals[monthlyTotals.length - 1];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      <MobileSection title="History" description="One insight at a time — 6 months, full width">
        <div style={{ padding: 16, border: "1px solid var(--line)", borderRadius: 16, background: "var(--surface)" }}>
          <div style={{ fontSize: 12, color: "var(--ink-faint)", fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>6-month spent</div>
          {monthlyTotals.length > 0 ? (
            <div style={{ display: "flex", alignItems: "end", gap: 6, height: 120, marginTop: 12 }}>
              {monthlyTotals.map((m) => (
                <div key={m.label} style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", gap: 6 }}>
                  <div
                    title={`${m.label}: ${money(m.spent)}`}
                    style={{
                      width: "100%",
                      height: `${Math.max(8, (m.spent / maxSpent) * 100)}%`,
                      minHeight: 8,
                      background: m === last ? "var(--accent)" : "var(--line-2)",
                      borderRadius: 6,
                      transition: "height 300ms ease",
                    }}
                  />
                  <span style={{ fontSize: 10, color: "var(--ink-faint)", fontWeight: 600, whiteSpace: "nowrap" }}>{m.label.slice(0, 3)}</span>
                </div>
              ))}
            </div>
          ) : (
            <div style={{ height: 120, display: "grid", placeItems: "center", color: "var(--ink-faint)", fontSize: 13, marginTop: 8, background: "var(--surface-2)", borderRadius: 12 }}>
              No history yet — import more months
            </div>
          )}
          {monthlyTotals.length > 0 ? (
            <MobileStatRow>
              <MobileStat label="Avg spent" value={money(avgSpent)} sub={`${monthlyTotals.length} months`} />
              <MobileStat label="Last month" value={last ? money(last.spent) : "—"} sub={last && avgSpent ? `${Math.round(((last.spent - avgSpent) / Math.max(1, avgSpent)) * 100)}% vs avg` : ""} />
            </MobileStatRow>
          ) : null}
        </div>
      </MobileSection>

      <MobileSection title="Quick insights">
        <MobileList ariaLabel="Reports insights">
          <MobileListItem icon={<I.Spark width={14} height={14} />} title="Spending by category" subtitle="Tap to drill" value="" onPress={() => {}} />
          <MobileListItem icon={<I.Horizon width={14} height={14} />} title="Cash flow calendar" subtitle="Upcoming · safe to spend" value="" onPress={() => {}} />
          <MobileListItem icon={<I.Bolt width={14} height={14} />} title="Budget vs actual" subtitle="This month comparison" value="" onPress={() => {}} />
        </MobileList>
      </MobileSection>
    </div>
  );
}

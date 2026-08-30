import { useMemo } from "react";
import { useBudgetHistory } from "../../api/hooks/budget";
import { money } from "../../utils/format";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { MobileStat, MobileStatRow } from "../../components/mobile/MobileStat";
import * as I from "../../components/Icons";

export default function MobileReports() {
  const { data: history = [] } = useBudgetHistory(6);
  const totals = useMemo(() => {
    const last = history[history.length - 1];
    if (!last) return null;
    return last;
  }, [history]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      <MobileSection title="History" description="Full-width chart would render here — simplified for mobile">
        <div style={{ padding: 16, border: "1px solid var(--line)", borderRadius: 16, background: "var(--surface)" }}>
          <div style={{ fontSize: 12, color: "var(--ink-faint)", fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>6-month trend</div>
          <div style={{ height: 120, display: "grid", placeItems: "center", color: "var(--ink-faint)", fontSize: 13, marginTop: 8, background: "var(--surface-2)", borderRadius: 12 }}>
            Chart uses available width — no tiny legends
          </div>
          {history.length > 0 ? (
            <MobileStatRow>
              <MobileStat label="Categories" value={String(history.length)} sub="Tracked" />
              <MobileStat label="Months" value="6" sub="History window" />
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

import { useRecurring } from "../../api/hooks/recurring";
import { money } from "../../utils/format";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import * as I from "../../components/Icons";

function daysUntil(dateStr: string): string | null {
  const next = new Date(dateStr);
  const now = new Date();
  const diff = Math.round((next.getTime() - now.getTime()) / 86400000);
  if (diff < 0) return null;
  if (diff === 0) return "Today";
  if (diff === 1) return "Tomorrow";
  if (diff <= 14) return `In ${diff} days`;
  return null;
}

export default function MobileRecurring() {
  const { data: recurring = [] } = useRecurring();

  if (recurring.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <MobileEmptyState icon={<I.Repeat width={28} height={28} />} title="No recurring yet" description="Subscriptions and bills will appear here once detected from your transactions." />
      </div>
    );
  }

  const soon = recurring.filter((r) => daysUntil(r.nextExpected) !== null);
  const other = recurring.filter((r) => daysUntil(r.nextExpected) === null);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      {soon.length > 0 ? (
        <MobileSection title="Due soon" description="Next two weeks">
          <MobileList ariaLabel="Due soon">
            {soon.map((item) => (
              <MobileListItem
                key={`${item.merchantRaw}-${item.nextExpected}`}
                icon={<span style={{ width: 10, height: 10, borderRadius: "50%", background: item.categoryColor ?? "var(--accent)", display: "inline-block" }} />}
                title={item.merchantRaw}
                subtitle={`${daysUntil(item.nextExpected)} · ${item.cadence ?? "monthly"}`}
                value={money(item.lastAmountCents ?? item.monthlyEquivalentCents ?? 0)}
                chevron={false}
              />
            ))}
          </MobileList>
        </MobileSection>
      ) : null}

      {other.length > 0 ? (
        <MobileSection title="All recurring" description={`${other.length} commitments`}>
          <MobileList ariaLabel="All recurring">
            {other.map((item) => (
              <MobileListItem
                key={`${item.merchantRaw}-${item.nextExpected}-other`}
                icon={<span style={{ width: 10, height: 10, borderRadius: "50%", background: item.categoryColor ?? "var(--line-2)", display: "inline-block" }} />}
                title={item.merchantRaw}
                subtitle={item.cadence ?? "recurring"}
                value={item.lastAmountCents ? money(item.lastAmountCents) : item.monthlyEquivalentCents ? money(item.monthlyEquivalentCents) : ""}
                chevron={false}
              />
            ))}
          </MobileList>
        </MobileSection>
      ) : null}
    </div>
  );
}

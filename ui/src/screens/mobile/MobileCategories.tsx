import { useMemo, useState } from "react";
import { useCategoriesWithSpending } from "../../api/hooks/transactions";
import { money } from "../../utils/format";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";

type Sort = "spent" | "name" | "budget";

export default function MobileCategories() {
  const { data: cats = [] } = useCategoriesWithSpending();
  const [sort, setSort] = useState<Sort>("spent");

  const sorted = useMemo(() => {
    const copy = [...cats];
    if (sort === "spent") copy.sort((a, b) => b.thisMonthCents - a.thisMonthCents);
    if (sort === "name") copy.sort((a, b) => a.label.localeCompare(b.label));
    if (sort === "budget") copy.sort((a, b) => (b as unknown as { budgetCents?: number }).budgetCents ?? 0 - ((a as unknown as { budgetCents?: number }).budgetCents ?? 0));
    return copy;
  }, [cats, sort]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      <SegmentedControl
        options={[
          { value: "spent", label: "Spent" },
          { value: "budget", label: "Budget" },
          { value: "name", label: "A–Z" },
        ]}
        value={sort}
        onChange={(v) => setSort(v as Sort)}
        ariaLabel="Sort categories"
      />

      <MobileSection title="Categories" description="One insight at a time — full width, mobile-readable">
        <MobileList ariaLabel="Categories">
          {sorted.map((c) => {
            const spendingType = (c as unknown as { spendingType?: string }).spendingType;
            return (
              <MobileListItem
                key={c.id}
                icon={<span style={{ width: 10, height: 10, borderRadius: "50%", background: c.color ?? "var(--accent)", display: "inline-block" }} />}
                title={c.label}
                subtitle={`${spendingType ?? ""}${spendingType ? " · " : ""}${c.txnCount} transactions`}
                value={money(c.thisMonthCents)}
                meta={c.thisMonthCents > 0 ? `${Math.round((c.thisMonthCents / Math.max(1, sorted.reduce((s, x) => s + x.thisMonthCents, 0))) * 100)}%` : undefined}
                chevron={false}
              />
            );
          })}
        </MobileList>
      </MobileSection>

      <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface-2)", color: "var(--ink-mute)", fontSize: 12 }}>
        Charts use available width with mobile-readable labels — no tiny legends. Tap a category to see transactions.
      </div>
    </div>
  );
}

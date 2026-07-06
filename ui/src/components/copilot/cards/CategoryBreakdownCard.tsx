import type { CopilotResponseBlock } from "../../../api/client";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { SegmentBar } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "categoryBreakdown" }>;

export function CategoryBreakdownCard({ block }: { block: Block }) {
  const max = Math.max(...block.rows.map((r) => r.amountCents));
  return (
    <div className="cp-card">
      <div className="cp-card-title">Spending by category · {block.periodLabel}</div>
      <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)", marginTop: 4, marginBottom: 12 }}>
        "fixed" cost · "lever" you control
      </div>
      <div className="cp-bars">
        {block.rows.map((r, i) => (
          <SegmentBar
            key={`${r.categoryKey}-${i}`}
            label={r.categoryKey}
            amountCents={r.amountCents}
            maxCents={max}
            color={colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)"}
            tag={r.isLever ? { text: "lever" } : r.isFixed ? { text: "fixed", muted: true } : undefined}
            dimmed={r.isFixed}
          />
        ))}
      </div>
    </div>
  );
}

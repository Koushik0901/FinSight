import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";

type Block = Extract<CopilotResponseBlock, { kind: "allocationSplit" }>;

const FALLBACK_COLORS = ["var(--accent)", "var(--c-travel)", "var(--c-dining)", "var(--c-shopping)"];

export function AllocationSplitCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-title">
        Recommended split of <span className="mono money">{money(block.totalCents)}</span>
      </div>
      <div className="muted" style={{ fontSize: 11.5, marginTop: 4, marginBottom: 16 }}>
        every dollar, accounted for
      </div>
      <div className="cp-alloc-bar">
        {block.segments.map((s, i) => (
          <div
            key={`${s.label}-${i}`}
            className="cp-alloc-seg"
            style={{
              width: `${(s.amountCents / block.totalCents) * 100}%`,
              background: colorForCategoryLabel(s.categoryKey) ?? FALLBACK_COLORS[i % FALLBACK_COLORS.length],
            }}
            title={`${s.label} · ${money(s.amountCents)}`}
          />
        ))}
      </div>
      <div className="cp-alloc-legend">
        {block.segments.map((s, i) => {
          const color = colorForCategoryLabel(s.categoryKey) ?? FALLBACK_COLORS[i % FALLBACK_COLORS.length];
          return (
            <div key={`${s.label}-${i}`} className="cp-alloc-row">
              <span className="cp-dot" style={{ background: color }} />
              <div className="cp-alloc-meta">
                <span className="cp-alloc-label">{s.label}</span>
                <span className="cp-alloc-why">{s.rationale}</span>
              </div>
              <span className="cp-alloc-amt mono money" style={{ color }}>{money(s.amountCents)}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

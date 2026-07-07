import type { CopilotResponseBlock } from "../../../api/client";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { ActionApprovalToolCard } from "../renderers";
import { ConfidenceBadge } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "recategorizationPreview" }>;

export function RecategorizationPreviewCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-title">
        {block.count} categorization{block.count === 1 ? "" : "s"} proposed
      </div>
      <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)", marginTop: 4, marginBottom: 12 }}>
        all proposed changes await your approval below
      </div>
      <div className="cp-recat">
        {block.rows.map((r, i) => {
          const color = colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)";
          return (
            <div key={i} className="cp-recat-row">
              <span className="cp-recat-merchant">{r.merchant}</span>
              <span className="cp-recat-cat" style={{ color, borderColor: color }}>
                <span className="cp-dot" style={{ background: color }} />
                {r.categoryKey}
              </span>
              <ConfidenceBadge confidence={r.confidence} color={color} />
            </div>
          );
        })}
        {block.more > 0 && <div className="cp-tx-more">+ {block.more} more matched the same way</div>}
      </div>
      {/* Never a standalone mutation — approve/reject/execute is the existing real flow. */}
      <div style={{ marginTop: 14 }}>
        <ActionApprovalToolCard bundleId={block.bundleId} />
      </div>
    </div>
  );
}

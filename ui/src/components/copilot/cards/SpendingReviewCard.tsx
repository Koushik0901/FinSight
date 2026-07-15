import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { SegmentBar, StatLine, ActionChecklist } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "spendingReview" }>;

/**
 * Composite spending-review surface: one bordered card per month, each with a
 * stat header, category bars, an optional summary box, and an action-plan
 * checklist. Cap-safe (one block carries N months), so a 3-month review never
 * blows the 8-block response cap. Presentational only — the checklist toggles
 * local state and mutations still route through the bundle-approval flow.
 */
export function SpendingReviewCard({ block }: { block: Block }) {
  return (
    <div className="cp-review">
      {block.months.map((m, mi) => {
        const max = Math.max(...m.categories.map((c) => c.amountCents), 1);
        return (
          <div key={`${m.label}-${mi}`} className="cp-card cp-review-month">
            <div className="cp-review-hd">
              <div className="cp-card-title">{m.label}</div>
              <StatLine parts={[`${money(m.spentCents)} spent`, m.subtitle ?? ""]} />
            </div>
            <div className="cp-bars">
              {m.categories.map((c, ci) => (
                <SegmentBar
                  key={`${c.label}-${ci}`}
                  label={c.label}
                  amountCents={c.amountCents}
                  maxCents={max}
                  color={colorForCategoryLabel(c.label) ?? "var(--ink-faint)"}
                  tag={
                    c.tag === "over"
                      ? { text: "over" }
                      : c.tag
                        ? { text: c.tag, muted: true }
                        : undefined
                  }
                  dimmed={c.tag === "fixed"}
                />
              ))}
            </div>
            {m.summary && <div className="cp-review-summary">{m.summary}</div>}
            {m.actions.length > 0 && <ActionChecklist title="Action plan" items={m.actions} />}
          </div>
        );
      })}
    </div>
  );
}

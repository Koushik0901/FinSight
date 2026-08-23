import type { CopilotResponseBlock } from "../../../api/openapiClient";
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
 *
 * Every number here is server-computed: the model emits only a period, a
 * summary, and actions per month, and the server fills label/spentCents/
 * categories from the ledger. Those fields are therefore optional on the wire,
 * and a month the server did not fill is skipped rather than shown as an
 * unlabeled "$0 spent" card. The server drops such months before sending; this
 * mirrors that so the two sides agree.
 */
export function SpendingReviewCard({ block }: { block: Block }) {
  const months = block.months.filter((m) => (m.label ?? "").trim().length > 0);
  if (months.length === 0) return null;
  return (
    <div className="cp-review">
      {months.map((m, mi) => {
        const categories = m.categories ?? [];
        const max = Math.max(...categories.map((c) => c.amountCents), 1);
        return (
          <div key={`${m.label}-${mi}`} className="cp-card cp-review-month">
            <div className="cp-review-hd">
              <div className="cp-card-title">{m.label}</div>
              <StatLine parts={[`${money(m.spentCents ?? 0)} spent`, m.subtitle ?? ""]} />
            </div>
            <div className="cp-bars">
              {categories.map((c, ci) => (
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
            {(m.actions ?? []).length > 0 && (
              <ActionChecklist title="Action plan" items={m.actions ?? []} />
            )}
          </div>
        );
      })}
    </div>
  );
}

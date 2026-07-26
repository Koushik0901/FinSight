import { useNavigate } from "react-router-dom";
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import Button from "../../Button";
import { ConfidenceBadge } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "categoryReviewQueue" }>;

/**
 * The categorization review queue, as a Copilot card.
 *
 * READ-ONLY by design, following `RecategorizationPreviewCard`'s rule that a
 * card never becomes a standalone mutation surface: accept / correct / dismiss
 * each have real side effects (a `source='user'` audit row, agent-memory
 * learning) and belong on `/review`, where the user can see what they are
 * deciding about. This card shows what is waiting and hands off.
 *
 * Every number here is server-synthesized from `category_proposals` — the model
 * emits the block bare — so the count can never disagree with the sidebar badge.
 */
export function CategoryReviewQueueCard({ block }: { block: Block }) {
  const navigate = useNavigate();
  // Both fields are optional in the generated type because the Rust struct
  // defaults them — that is the thin-emission contract, surfaced in the type
  // system. A block that somehow reached the client un-hydrated reads as an
  // empty queue rather than crashing.
  const count = block.pendingCount ?? 0;
  const shown = block.items ?? [];
  const more = Math.max(0, count - shown.length);

  if (count === 0) {
    return (
      <div className="cp-card">
        <div className="cp-card-title">Nothing waiting on your review</div>
        <div className="muted" style={{ fontSize: 12.5, marginTop: 4 }}>
          Every automated categorization has either been confirmed or was
          confident enough not to ask.
        </div>
      </div>
    );
  }

  return (
    <div className="cp-card">
      <div className="cp-card-title">
        {count} categorization{count === 1 ? "" : "s"} waiting on you
      </div>
      <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)", marginTop: 4, marginBottom: 12 }}>
        low-confidence guesses the agent wants confirmed
      </div>

      <div className="cp-recat">
        {shown.map((item, i) => {
          const color = colorForCategoryLabel(item.proposedCategory) ?? "var(--ink-faint)";
          return (
            <div key={`${item.merchant}-${i}`} className="cp-recat-row">
              <span className="cp-recat-merchant">{item.merchant}</span>
              <span className="cp-recat-cat" style={{ color, borderColor: color }}>
                <span className="cp-dot" style={{ background: color }} />
                {item.proposedCategory}
              </span>
              {item.amountCents != null && (
                <span className="mono money" style={{ fontSize: 12 }}>
                  {money(item.amountCents, { decimals: 2 })}
                </span>
              )}
              <ConfidenceBadge confidence={item.confidence} color={color} />
            </div>
          );
        })}
        {more > 0 && <div className="cp-tx-more">+ {more} more in the queue</div>}
      </div>

      {/*
        `applied` and the pending review status are separate axes. Saying
        "waiting on you" without this line would imply nothing is counted yet —
        but an applied proposal is already in the user's budgets, and dismissing
        it leaves it there.
      */}
      <div className="muted" style={{ fontSize: 12, marginTop: 10 }}>
        {shown.every((i) => i.applied)
          ? "These categories are already applied — reviewing confirms them rather than turning them on."
          : "Some of these have not been written to their transactions yet."}
      </div>

      <div style={{ marginTop: 12 }}>
        <Button variant="outline" size="sm" onClick={() => navigate("/review")}>
          Open the review queue
        </Button>
      </div>
    </div>
  );
}

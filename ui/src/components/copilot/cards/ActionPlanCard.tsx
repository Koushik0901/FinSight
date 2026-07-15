import type { CopilotResponseBlock } from "../../../api/client";
import { ActionChecklist } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "actionPlan" }>;

/**
 * A standalone next-steps checklist (reuses the same ActionChecklist as the
 * SpendingReviewCard month cards). Presentational — checkboxes toggle local
 * state only; anything that mutates data stays on the bundle-approval flow.
 */
export function ActionPlanCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <ActionChecklist title={block.title ?? "Action plan"} items={block.items} />
    </div>
  );
}

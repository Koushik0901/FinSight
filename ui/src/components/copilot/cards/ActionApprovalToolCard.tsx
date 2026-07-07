import { toast } from "sonner";
import * as I from "../../Icons";
import Button from "../../Button";
import Badge from "../../Badge";
import type { ExecutionSummary } from "../../../api/client";
import {
  useActionBundle,
  useApproveActionItem,
  useRejectActionItem,
  useExecuteActionBundle,
} from "../../../api/hooks/copilot";
import { humanizeToolName } from "../toolNames";

/** The real approve / reject / execute UI for a backend-issued action bundle.
 *  Rendered both by the main copilot tool-call path (for `request_action_approval`)
 *  and by `RecategorizationPreviewCard`. Lives in its own module so the card
 *  layer never has to import back into `renderers.tsx`. */
export function ActionApprovalToolCard({ bundleId }: { bundleId: string }) {
  const { data: bundle, isLoading } = useActionBundle(bundleId);
  const approve = useApproveActionItem();
  const reject = useRejectActionItem();
  const execute = useExecuteActionBundle();

  if (isLoading) {
    return (
      <div className="copilot-tool-card">
        <div className="copilot-tool-head">
          <span>Review proposed actions</span>
          <span className="copilot-tool-status">loading</span>
        </div>
        <p>Verifying this approval request against FinSight’s local action store.</p>
      </div>
    );
  }

  if (!bundle) {
    return (
      <div className="copilot-tool-card" data-error="true">
        <div className="copilot-tool-head">
          <span>Approval unavailable</span>
          <span className="copilot-tool-status">rejected</span>
        </div>
        <p>This approval request does not match a backend-issued action bundle.</p>
      </div>
    );
  }

  const pendingItems = bundle.items.filter((item) => item.status === "pending");
  const approvedItems = bundle.items.filter((item) => item.status === "approved");
  const canExecute = approvedItems.length > 0 && !execute.isPending;

  const runExecute = async () => {
    try {
      const summary = await execute.mutateAsync(bundle.id) as ExecutionSummary;
      if (summary.failed > 0) {
        toast.error(`${summary.failed} action${summary.failed === 1 ? "" : "s"} failed`, {
          description: `${summary.succeeded} succeeded.`,
        });
      } else {
        toast.success(`${summary.succeeded} action${summary.succeeded === 1 ? "" : "s"} applied`);
      }
    } catch (error) {
      toast.error("Could not execute approved actions", { description: String(error) });
    }
  };

  return (
    <div className="copilot-tool-card copilot-approval-card">
      <div className="copilot-tool-head">
        <span>Review proposed actions</span>
        <span className="copilot-tool-status">
          {pendingItems.length > 0 ? "requires action" : approvedItems.length > 0 ? "approved" : bundle.status}
        </span>
      </div>
      <p>
        FinSight generated {bundle.items.length} draft action{bundle.items.length === 1 ? "" : "s"}.
        Nothing changes until you approve and execute them.
      </p>
      <div className="stack stack-sm">
        {bundle.items.map((item) => (
          <div key={item.id} className="copilot-approval-row">
            <div>
              <strong>{humanizeToolName(item.actionKind)}</strong>
              <p>{item.rationale}</p>
            </div>
            <Badge tone={item.status === "rejected" ? "negative" : item.status === "pending" ? "warning" : "positive"}>
              {item.status}
            </Badge>
            {item.status === "pending" && (
              <div className="row-sm">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={approve.isPending || reject.isPending}
                  onClick={() => approve.mutate(item.id)}
                >
                  <I.Check width={12} height={12} />
                  Approve
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={approve.isPending || reject.isPending}
                  onClick={() => reject.mutate(item.id)}
                >
                  <I.X width={12} height={12} />
                  Reject
                </Button>
              </div>
            )}
          </div>
        ))}
      </div>
      {canExecute && (
        <div className="copilot-approval-footer">
          <Button
            variant="primary"
            size="sm"
            loading={execute.isPending}
            disabled={execute.isPending}
            onClick={() => void runExecute()}
          >
            <I.Check width={13} height={13} />
            Execute approved actions
          </Button>
        </div>
      )}
    </div>
  );
}

import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import * as I from "../../Icons";
import Button from "../../Button";
import Badge from "../../Badge";
import type { AgentNavigationTarget, ExecutionSummary } from "../../../api/client";
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
  const navigate = useNavigate();
  // Where the user can go to see what just landed. Populated only after a
  // successful execution, and never navigated to automatically — being yanked
  // out of a conversation mid-thread is worse than clicking a link.
  const [navTargets, setNavTargets] = useState<AgentNavigationTarget[]>([]);

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
      // Backend-derived from the payloads that actually applied, so these
      // always point at a real screen. Older servers omit the field entirely.
      setNavTargets(summary.navigation ?? []);
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
      {navTargets.length > 0 && (
        <div className="copilot-approval-footer copilot-approval-nav">
          <span className="copilot-approval-nav-lbl">See the change</span>
          {navTargets.map((target) => (
            <Button
              key={target.path}
              variant="outline"
              size="sm"
              onClick={() => navigate(target.path)}
            >
              {target.label}
              <I.ArrowRight width={12} height={12} />
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}

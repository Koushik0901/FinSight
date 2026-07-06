import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  defineToolkit,
  type GenerativeUIComponentRegistry,
  type ToolCallMessagePartProps,
} from "@assistant-ui/react";
import { toast } from "sonner";
import * as I from "../Icons";
import Button from "../Button";
import Badge from "../Badge";
import type { CopilotResponseBlock, ExecutionSummary } from "../../api/client";
import {
  useActionBundle,
  useApproveActionItem,
  useRejectActionItem,
  useExecuteActionBundle,
} from "../../api/hooks/copilot";
import { parseFinanceArtifactEnvelope } from "./agUi/artifacts";
import { colorForCategoryLabel } from "../../utils/categoryColor";
import { TransactionTableCard } from "./cards/TransactionTableCard";
import { AffordabilityVerdictCard } from "./cards/AffordabilityVerdictCard";

const ALL_TOOL_NAMES = [
  "get_financial_snapshot",
  "analyze_cash_inflow",
  "calculate_goal_eta",
  "rank_debt_payoff",
  "compare_debt_vs_goal",
  "get_account_balances",
  "get_month_totals",
  "get_top_spending_categories",
  "get_budgets",
  "get_goals",
  "get_recurring_bills",
  "get_liabilities",
  "search_transactions",
  "run_cashflow_projection",
  "run_debt_payoff_scenarios",
  "run_goal_allocation_scenarios",
  "run_goal_conflict_scenario",
  "run_emergency_fund_scenarios",
  "run_cashflow_timeline",
  "run_purchase_affordability",
  "get_data_quality_report",
  "draft_set_budget",
  "draft_update_goal_monthly",
  "draft_create_planned_transaction",
  "draft_save_scenario",
  "draft_debt_payoff_plan",
  "request_action_approval",
  "render_finance_artifact",
] as const;

function humanizeToolName(name: string) {
  return name
    .replace(/^draft_/, "")
    .replace(/^get_/, "")
    .replace(/^run_/, "")
    .replaceAll("_", " ");
}

function formatValue(value: unknown) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (value == null) return "";
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

export function CopilotToolCard({
  toolName,
  args,
  result,
  isError,
  status,
}: ToolCallMessagePartProps<Record<string, unknown>, unknown>) {
  if (toolName === "render_finance_artifact" && typeof result === "string") {
    const artifact = parseFinanceArtifactEnvelope(result);
    const block = artifact?.component === "FinSightResponseBlock"
      ? (artifact.props.block as CopilotResponseBlock | undefined)
      : undefined;
    if (block) {
      return <FinSightResponseBlock block={block} />;
    }
  }

  if (toolName === "request_action_approval") {
    const approval = parseApprovalResult(result) ?? parseApprovalResult(args);
    if (approval) return <ActionApprovalToolCard bundleId={approval.bundleId} />;
  }

  const resultObj = result && typeof result === "object" ? (result as Record<string, unknown>) : null;
  const summary =
    typeof resultObj?.summary === "string"
      ? resultObj.summary
      : isError
        ? "The tool returned an error."
        : status.type === "running"
          ? "Working with your local finance data."
          : "Completed.";

  return (
    <div className="copilot-tool-card" data-error={isError ? "true" : "false"}>
      <div className="copilot-tool-head">
        <span>{humanizeToolName(toolName)}</span>
        <span className="copilot-tool-status">{status.type}</span>
      </div>
      <p>{summary}</p>
      {Object.keys(args ?? {}).length > 0 && (
        <div className="copilot-tool-args">
          {Object.entries(args).map(([key, value]) => (
            <span key={key}>
              {key}: {formatValue(value)}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function parseApprovalResult(value: unknown): { bundleId: string } | null {
  let parsed = value;
  if (typeof value === "string") {
    try {
      parsed = JSON.parse(value) as unknown;
    } catch {
      return null;
    }
  }
  if (!parsed || typeof parsed !== "object") return null;
  const record = parsed as Record<string, unknown>;
  if (record.kind !== "approval_request" && typeof record.bundleId !== "string") return null;
  const bundleId = record.bundleId;
  return typeof bundleId === "string" && bundleId.trim() ? { bundleId } : null;
}

function ActionApprovalToolCard({ bundleId }: { bundleId: string }) {
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

function MetricGrid({ metrics }: Extract<CopilotResponseBlock, { kind: "metricGrid" }>) {
  return (
    <div className="copilot-gen-grid">
      {metrics.map((metric) => (
        <div key={`${metric.label}-${metric.value}`} className="copilot-gen-metric" data-tone={metric.tone ?? "neutral"}>
          <span>{metric.label}</span>
          <strong>{metric.value}</strong>
          {metric.detail && <small>{metric.detail}</small>}
        </div>
      ))}
    </div>
  );
}

function TableBlock({ title, columns, rows }: Extract<CopilotResponseBlock, { kind: "table" }>) {
  return (
    <div className="copilot-gen-table-wrap">
      {title && <p className="copilot-gen-title">{title}</p>}
      <table className="copilot-gen-table">
        <thead>
          <tr>{columns.map((column) => <th key={column}>{column}</th>)}</tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={index}>
              {row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ChartBlock(block: Extract<CopilotResponseBlock, { kind: "barChart" | "lineChart" }>) {
  const max = Math.max(...block.data.map((point) => point.value), 1);
  return (
    <div className="copilot-gen-chart">
      {block.title && <p className="copilot-gen-title">{block.title}</p>}
      {block.data.map((point) => {
        // Bars labelled with a known category reuse its canonical color so
        // Copilot charts match Reports/Budget/Categories; others keep accent.
        const categoryColor = colorForCategoryLabel(point.label);
        return (
          <div key={point.label} className="copilot-gen-bar-row">
            <span>{point.label}</span>
            <div><i style={{ inlineSize: `${Math.max(4, (point.value / max) * 100)}%`, ...(categoryColor ? { background: categoryColor } : {}) }} /></div>
            <strong>{point.value.toLocaleString()}</strong>
          </div>
        );
      })}
    </div>
  );
}

function CalloutBlock({ tone, title, body }: Extract<CopilotResponseBlock, { kind: "callout" }>) {
  return (
    <div className="copilot-gen-callout" data-tone={tone}>
      {title && <strong>{title}</strong>}
      <p>{body}</p>
    </div>
  );
}

export function FinSightResponseBlock({ block }: { block: CopilotResponseBlock }) {
  switch (block.kind) {
    case "markdown":
      return (
        <div className="copilot-gen-markdown">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{block.markdown}</ReactMarkdown>
        </div>
      );
    case "metricGrid":
      return <MetricGrid {...block} />;
    case "table":
      return <TableBlock {...block} />;
    case "barChart":
    case "lineChart":
      return <ChartBlock {...block} />;
    case "callout":
      return <CalloutBlock {...block} />;
    case "transactionTable":
      return <TransactionTableCard block={block} />;
    case "affordabilityVerdict":
      return <AffordabilityVerdictCard block={block} />;
    default:
      return null;
  }
}

export const generativeUIComponents: GenerativeUIComponentRegistry = {
  FinSightResponseBlock,
};

const renderOnlyTool = (description: string) => ({
  description,
  parameters: {
    type: "object",
    properties: {},
    additionalProperties: true,
  } as const,
  render: CopilotToolCard,
});

export const copilotToolkit = defineToolkit(
  Object.fromEntries(
    ALL_TOOL_NAMES.map((name) => [
      name,
      renderOnlyTool(`Render the FinSight ${humanizeToolName(name)} tool call.`),
    ])
  )
);

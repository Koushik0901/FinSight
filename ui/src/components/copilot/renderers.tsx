import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  defineToolkit,
  useMessage,
  type GenerativeUIComponentRegistry,
  type ToolCallMessagePartProps,
} from "@assistant-ui/react";
import type { CopilotResponseBlock } from "../../api/client";
import { parseFinanceArtifactEnvelope } from "./agUi/artifacts";
import { colorForCategoryLabel } from "../../utils/categoryColor";
import { humanizeToolName } from "./toolNames";
import { ActionApprovalToolCard } from "./cards/ActionApprovalToolCard";
import { TransactionTableCard } from "./cards/TransactionTableCard";
import { AffordabilityVerdictCard } from "./cards/AffordabilityVerdictCard";
import { CategoryBreakdownCard } from "./cards/CategoryBreakdownCard";
import { AllocationSplitCard } from "./cards/AllocationSplitCard";
import { RankedOptionsCard } from "./cards/RankedOptionsCard";
import { ComparisonBarsCard } from "./cards/ComparisonBarsCard";
import { RecategorizationPreviewCard } from "./cards/RecategorizationPreviewCard";
import { SpendingReviewCard } from "./cards/SpendingReviewCard";
import { AccountsOverviewCard } from "./cards/AccountsOverviewCard";
import { SpendTimelineCard } from "./cards/SpendTimelineCard";
import { SpendingDriversCard } from "./cards/SpendingDriversCard";
import { WatchListCard } from "./cards/WatchListCard";
import { ActionPlanCard } from "./cards/ActionPlanCard";
import { ClarificationCard } from "./cards/ClarificationCard";

const ALL_TOOL_NAMES = [
  "get_financial_snapshot",
  "analyze_cash_inflow",
  "calculate_goal_eta",
  "rank_debt_payoff",
  "compare_payoff_strategies",
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
  // The response-block artifact's own tool-call part completes as soon as its
  // result arrives, which happens BEFORE prose streaming even starts (the
  // backend emits response blocks, then streams text word-by-word) — so
  // `status.type` here is "complete" for nearly the entire streaming window,
  // not a useful signal for "is the message still streaming." Read the real
  // message-level running state instead, since that's what ComparisonBars
  // actually needs to know before it's safe to mount a Recharts chart.
  const message = useMessage();
  const messageIsRunning = message.status?.type === "running";

  if (toolName === "render_finance_artifact" && typeof result === "string") {
    const artifact = parseFinanceArtifactEnvelope(result);
    const block = artifact?.component === "FinSightResponseBlock"
      ? (artifact.props.block as CopilotResponseBlock | undefined)
      : undefined;
    if (block) {
      return <FinSightResponseBlock block={block} isRunning={messageIsRunning} />;
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

function MetricGrid({ metrics }: Extract<CopilotResponseBlock, { kind: "metricGrid" }>) {
  return (
    <div className="cp-card">
      <div className="copilot-gen-grid">
        {metrics.map((metric) => (
          <div key={`${metric.label}-${metric.value}`} className="copilot-gen-metric" data-tone={metric.tone ?? "neutral"}>
            <span>{metric.label}</span>
            <strong>{metric.value}</strong>
            {metric.detail && <small>{metric.detail}</small>}
          </div>
        ))}
      </div>
    </div>
  );
}

function TableBlock({ title, columns, rows }: Extract<CopilotResponseBlock, { kind: "table" }>) {
  const categoryColIndex = columns.findIndex((c) => c.toLowerCase() === "category");
  return (
    <div className="cp-card">
      {title && <div className="cp-card-title" style={{ marginBottom: 12 }}>{title}</div>}
      <table className="tbl">
        <thead>
          <tr>{columns.map((column) => <th key={column}>{column}</th>)}</tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={index}>
              {row.map((cell, cellIndex) => (
                <td key={cellIndex}>
                  {cellIndex === categoryColIndex ? (
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                      <span className="cp-dot" style={{ background: colorForCategoryLabel(cell) ?? "var(--ink-faint)" }} />
                      {cell}
                    </span>
                  ) : cell}
                </td>
              ))}
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
    <div className="cp-card">
      {block.title && <p className="cp-card-title" style={{ marginBottom: 12 }}>{block.title}</p>}
      <div className="copilot-gen-chart">
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
    </div>
  );
}

function CalloutBlock({ tone, title, body }: Extract<CopilotResponseBlock, { kind: "callout" }>) {
  return (
    <div className="cp-card copilot-gen-callout" data-tone={tone}>
      {title && <strong>{title}</strong>}
      <p>{body}</p>
    </div>
  );
}

export function FinSightResponseBlock({
  block,
  isRunning,
}: {
  block: CopilotResponseBlock;
  isRunning: boolean;
}) {
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
    case "categoryBreakdown":
      return <CategoryBreakdownCard block={block} />;
    case "allocationSplit":
      return <AllocationSplitCard block={block} />;
    case "rankedOptions":
      return <RankedOptionsCard block={block} />;
    case "comparisonBars":
      return <ComparisonBarsCard block={block} isRunning={isRunning} />;
    case "recategorizationPreview":
      return <RecategorizationPreviewCard block={block} />;
    case "spendingReview":
      return <SpendingReviewCard block={block} />;
    case "accountsOverview":
      return <AccountsOverviewCard block={block} />;
    case "spendTimeline":
      return <SpendTimelineCard block={block} />;
    case "spendingDrivers":
      return <SpendingDriversCard block={block} />;
    case "watchList":
      return <WatchListCard block={block} />;
    case "actionPlan":
      return <ActionPlanCard block={block} />;
    case "clarification":
      return <ClarificationCard block={block} />;
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

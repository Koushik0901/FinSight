import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  defineToolkit,
  type GenerativeUIComponentRegistry,
  type ToolCallMessagePartProps,
} from "@assistant-ui/react";
import { z } from "zod";
import type { CopilotResponseBlock } from "../../api/client";

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
      {block.data.map((point) => (
        <div key={point.label} className="copilot-gen-bar-row">
          <span>{point.label}</span>
          <div><i style={{ inlineSize: `${Math.max(4, (point.value / max) * 100)}%` }} /></div>
          <strong>{point.value.toLocaleString()}</strong>
        </div>
      ))}
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
    default:
      return null;
  }
}

export const generativeUIComponents: GenerativeUIComponentRegistry = {
  FinSightResponseBlock,
};

const renderOnlyTool = (description: string) => ({
  description,
  parameters: z.object({}).passthrough(),
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

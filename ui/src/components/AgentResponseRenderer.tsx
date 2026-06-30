import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { ResponsiveBar } from "@nivo/bar";
import { ResponsiveLine } from "@nivo/line";
import type { AgentAnswer, AgentResponseBlock } from "../api/client";

type Props = {
  answer: Pick<AgentAnswer, "prose" | "responseBlocks">;
  compact?: boolean;
};

function MarkdownBlock({ markdown, compact }: { markdown: string; compact: boolean }) {
  return (
    <div className={`agent-rich-markdown${compact ? " compact" : ""}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeSanitize]}
        components={{
          table: ({ children }) => (
            <div className="agent-rich-table-wrap">
              <table className="tbl">{children}</table>
            </div>
          ),
          a: ({ children, href }) => (
            <a href={href} target="_blank" rel="noreferrer">
              {children}
            </a>
          ),
        }}
      >
        {markdown}
      </ReactMarkdown>
    </div>
  );
}

function TableBlock({ block }: { block: Extract<AgentResponseBlock, { kind: "table" }> }) {
  return (
    <div className="agent-rich-block stack stack-sm">
      {block.title && <p className="eyebrow">{block.title}</p>}
      <div className="agent-rich-table-wrap">
        <table className="tbl" aria-label={block.title ?? "Agent response table"}>
          <thead>
            <tr>
              {block.columns.map((column) => (
                <th key={column}>{column}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {block.rows.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {row.map((cell, cellIndex) => (
                  <td key={`${rowIndex}-${cellIndex}`}>{cell}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function ChartBlock({ block }: { block: Extract<AgentResponseBlock, { kind: "barChart" | "lineChart" }> }) {
  const seriesLabel = block.seriesLabel ?? "Value";
  const safeData = block.data.filter((point) => Number.isFinite(point.value));
  if (safeData.length === 0) return null;

  return (
    <div className="agent-rich-block stack stack-sm">
      {block.title && <p className="eyebrow">{block.title}</p>}
      <div className="agent-rich-chart" role="img" aria-label={block.title ?? "Agent response chart"}>
        {block.kind === "barChart" ? (
          <ResponsiveBar
            data={safeData.map((point) => ({ label: point.label, [seriesLabel]: point.value }))}
            keys={[seriesLabel]}
            indexBy="label"
            margin={{ top: 16, right: 16, bottom: 48, left: 54 }}
            padding={0.32}
            valueScale={{ type: "linear" }}
            colors={["var(--accent)"]}
            borderRadius={6}
            enableLabel={false}
            axisTop={null}
            axisRight={null}
            theme={{
              text: { fill: "var(--ink-mute)", fontSize: 11 },
              grid: { line: { stroke: "var(--line)" } },
              axis: {
                ticks: { line: { stroke: "var(--line)" }, text: { fill: "var(--ink-mute)" } },
                legend: { text: { fill: "var(--ink-mute)" } },
              },
              tooltip: { container: { background: "var(--elevated)", color: "var(--ink)" } },
            }}
          />
        ) : (
          <ResponsiveLine
            data={[{ id: seriesLabel, data: safeData.map((point) => ({ x: point.label, y: point.value })) }]}
            margin={{ top: 16, right: 20, bottom: 48, left: 54 }}
            xScale={{ type: "point" }}
            yScale={{ type: "linear", min: "auto", max: "auto", stacked: false, reverse: false }}
            curve="monotoneX"
            colors={["var(--accent)"]}
            pointSize={7}
            pointBorderWidth={2}
            pointBorderColor="var(--surface)"
            useMesh
            axisTop={null}
            axisRight={null}
            theme={{
              text: { fill: "var(--ink-mute)", fontSize: 11 },
              grid: { line: { stroke: "var(--line)" } },
              axis: {
                ticks: { line: { stroke: "var(--line)" }, text: { fill: "var(--ink-mute)" } },
                legend: { text: { fill: "var(--ink-mute)" } },
              },
              tooltip: { container: { background: "var(--elevated)", color: "var(--ink)" } },
            }}
          />
        )}
      </div>
    </div>
  );
}

function MetricGridBlock({ block }: { block: Extract<AgentResponseBlock, { kind: "metricGrid" }> }) {
  return (
    <div className="agent-rich-metrics">
      {block.metrics.map((metric) => (
        <div key={`${metric.label}-${metric.value}`} className={`agent-rich-metric ${metric.tone ?? "neutral"}`}>
          <span>{metric.label}</span>
          <strong className={metric.value.includes("$") ? "money" : undefined}>{metric.value}</strong>
          {metric.detail && <small>{metric.detail}</small>}
        </div>
      ))}
    </div>
  );
}

function CalloutBlock({ block }: { block: Extract<AgentResponseBlock, { kind: "callout" }> }) {
  return (
    <div className={`agent-rich-callout ${block.tone}`}>
      {block.title && <strong>{block.title}</strong>}
      <p>{block.body}</p>
    </div>
  );
}

function renderBlock(block: AgentResponseBlock, index: number, compact: boolean) {
  switch (block.kind) {
    case "markdown":
      return <MarkdownBlock key={index} markdown={block.markdown} compact={compact} />;
    case "table":
      return <TableBlock key={index} block={block} />;
    case "barChart":
    case "lineChart":
      return compact ? null : <ChartBlock key={index} block={block} />;
    case "metricGrid":
      return <MetricGridBlock key={index} block={block} />;
    case "callout":
      return <CalloutBlock key={index} block={block} />;
    default:
      return null;
  }
}

export function AgentResponseRenderer({ answer, compact = false }: Props) {
  const blocks =
    answer.responseBlocks && answer.responseBlocks.length > 0
      ? answer.responseBlocks
      : answer.prose.trim()
        ? [{ kind: "markdown" as const, markdown: answer.prose }]
        : [];

  return <div className={`agent-rich stack ${compact ? "stack-sm compact" : "stack-md"}`}>{blocks.map((block, index) => renderBlock(block, index, compact))}</div>;
}

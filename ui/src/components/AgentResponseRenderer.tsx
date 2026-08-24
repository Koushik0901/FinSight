import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { ResponsiveContainer, BarChart, Bar, LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip } from "recharts";
import type { AgentAnswer, AgentResponseBlock } from "../api/openapiClient";

const chartTooltipStyle = {
  background: "var(--elevated)",
  border: "1px solid var(--line)",
  borderRadius: 8,
  color: "var(--ink)",
  fontSize: 12,
};
const chartAxisTick = { fill: "var(--ink-mute)", fontSize: 11 };

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
        <ResponsiveContainer width="100%" height="100%">
          {block.kind === "barChart" ? (
            <BarChart data={safeData} margin={{ top: 16, right: 16, bottom: 8, left: 8 }}>
              <CartesianGrid vertical={false} stroke="var(--line)" />
              <XAxis dataKey="label" tick={chartAxisTick} axisLine={{ stroke: "var(--line)" }} tickLine={false} />
              <YAxis tick={chartAxisTick} axisLine={false} tickLine={false} width={48} />
              <Tooltip cursor={{ fill: "var(--surface-2)" }} contentStyle={chartTooltipStyle} labelStyle={{ color: "var(--ink-mute)" }} />
              <Bar dataKey="value" name={seriesLabel} fill="var(--accent)" radius={[6, 6, 0, 0]} maxBarSize={40} />
            </BarChart>
          ) : (
            <LineChart data={safeData} margin={{ top: 16, right: 20, bottom: 8, left: 8 }}>
              <CartesianGrid vertical={false} stroke="var(--line)" />
              <XAxis dataKey="label" tick={chartAxisTick} axisLine={{ stroke: "var(--line)" }} tickLine={false} />
              <YAxis tick={chartAxisTick} axisLine={false} tickLine={false} width={48} />
              <Tooltip contentStyle={chartTooltipStyle} labelStyle={{ color: "var(--ink-mute)" }} />
              <Line
                type="monotone"
                dataKey="value"
                name={seriesLabel}
                stroke="var(--accent)"
                strokeWidth={2}
                dot={{ r: 4, fill: "var(--accent)", strokeWidth: 0 }}
                activeDot={{ r: 6 }}
              />
            </LineChart>
          )}
        </ResponsiveContainer>
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

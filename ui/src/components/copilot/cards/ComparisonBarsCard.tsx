import type { CopilotResponseBlock } from "../../../api/client";
import { FinSightBarComparison } from "../charts/FinSightChart";

type Block = Extract<CopilotResponseBlock, { kind: "comparisonBars" }>;

/**
 * Recharts' ResponsiveContainer renders blank at width:0 and re-animates on
 * every reflow (see the FinSightChart.stream test in Phase B) — so this card
 * only mounts the chart once the assistant message has finished streaming,
 * matching the mockup's own reveal order where cards appear after the answer.
 */
export function ComparisonBarsCard({ block, isRunning }: { block: Block; isRunning: boolean }) {
  if (isRunning) {
    return (
      <div className="cp-card">
        <div className="cp-card-title">{block.title}</div>
        <p className="muted" style={{ fontSize: 12.5, marginTop: 8 }}>Preparing comparison…</p>
      </div>
    );
  }
  return (
    <FinSightBarComparison
      title={block.title}
      current={{ label: block.current.label, amountCents: block.current.amountCents }}
      prior={{ label: block.prior.label, amountCents: block.prior.amountCents }}
    />
  );
}

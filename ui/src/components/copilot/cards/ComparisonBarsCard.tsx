import { lazy, Suspense } from "react";
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";

const FinSightBarComparison = lazy(() =>
  import("../charts/FinSightChart").then((module) => ({ default: module.FinSightBarComparison }))
);

type Block = Extract<CopilotResponseBlock, { kind: "comparisonBars" }>;

function ComparisonFallback({ block }: { block: Block }) {
  return (
    <div className="cp-card" aria-busy="true">
      <p className="cp-card-title" style={{ marginBottom: 12 }}>{block.title}</p>
      <div className="row-sm" style={{ justifyContent: "space-between" }}>
        <span className="mono" style={{ fontSize: 12, color: "var(--ink-mute)" }}>
          <span>{block.prior.label}</span>: <span className="money">{money(block.prior.amountCents)}</span>
        </span>
        <span className="mono" style={{ fontSize: 12, color: "var(--ink)" }}>
          <span>{block.current.label}</span>: <span className="money">{money(block.current.amountCents)}</span>
        </span>
      </div>
    </div>
  );
}

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
    <Suspense fallback={<ComparisonFallback block={block} />}>
      <FinSightBarComparison
        title={block.title}
        current={{ label: block.current.label, amountCents: block.current.amountCents }}
        prior={{ label: block.prior.label, amountCents: block.prior.amountCents }}
      />
    </Suspense>
  );
}

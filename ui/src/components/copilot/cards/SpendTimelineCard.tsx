import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { StatLine } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "spendTimeline" }>;

/**
 * A month-by-month spend trend as vertical bars scaled to the max point.
 * `highlight` accents the months being focused on, `projected` marks an
 * incomplete current month (dashed), and `annotation` labels an outlier bar.
 */
export function SpendTimelineCard({ block }: { block: Block }) {
  const max = Math.max(...block.points.map((p) => p.amountCents), 1);
  return (
    <div className="cp-card cp-timeline">
      {block.title && <div className="cp-card-title">{block.title}</div>}
      {block.subtitle && <StatLine parts={[block.subtitle]} />}
      <div className="cp-timeline-bars">
        {block.points.map((p, i) => (
          <div
            key={`${p.label}-${i}`}
            className={`cp-tl-col ${p.highlight ? "is-hl" : ""} ${p.projected ? "is-proj" : ""}`}
          >
            {p.annotation && <span className="cp-tl-note">{p.annotation}</span>}
            <span className="cp-tl-val mono">{money(p.amountCents)}</span>
            <div className="cp-tl-bar" style={{ height: `${Math.max(4, (p.amountCents / max) * 100)}%` }} />
            <span className="cp-tl-label">{p.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

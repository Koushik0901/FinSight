import { money } from "../../../utils/format";

/**
 * A labeled, colored, proportional-width bar with a trailing amount — shared
 * by result cards that need to show "this amount as a fraction of a whole"
 * (e.g. category spending vs. the largest category, a debt-payoff segment)
 * so this visual isn't duplicated per card.
 */
export function SegmentBar({
  label,
  amountCents,
  maxCents,
  color,
  tag,
  dimmed,
}: {
  label: string;
  amountCents: number;
  maxCents: number;
  color: string;
  tag?: { text: string; muted?: boolean };
  dimmed?: boolean;
}) {
  const pct = maxCents > 0 ? (amountCents / maxCents) * 100 : 0;
  return (
    <div className="cp-bar-row">
      <div className="cp-bar-label">
        <span className="cp-dot" style={{ background: color }} />
        {label}
        {tag && <span className={`cp-bar-tag ${tag.muted ? "muted" : ""}`}>{tag.text}</span>}
      </div>
      <div className="cp-bar-track">
        <div
          data-testid="segment-bar-fill"
          className="cp-bar-fill"
          style={{ width: `${pct}%`, background: color, opacity: dimmed ? 0.4 : 1 }}
        />
      </div>
      <span className="cp-bar-amt mono money">{money(amountCents)}</span>
    </div>
  );
}

/** A confidence-percentage meter — shared wherever a proposed change carries a confidence score. */
export function ConfidenceBadge({ confidence, color }: { confidence: number; color: string }) {
  const pct = Math.round(confidence * 100);
  return (
    <div className="cp-conf">
      <div className="cp-conf-track">
        <div data-testid="confidence-fill" className="cp-conf-fill" style={{ width: `${pct}%`, background: color }} />
      </div>
      <span className="cp-conf-num mono">{pct}%</span>
    </div>
  );
}

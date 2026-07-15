import { useState } from "react";
import { money } from "../../../utils/format";
import * as I from "../../Icons";

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

/** A mono, middot-joined sub-header line (spent stats, account summary, timeline caption). */
export function StatLine({ parts }: { parts: string[] }) {
  return <div className="cp-statline mono">{parts.filter(Boolean).join(" · ")}</div>;
}

/** A small uppercase tag pill whose color comes from a tone token (drivers, category flags). */
export function TagPill({ label, tone }: { label: string; tone: string }) {
  return (
    <span className="cp-tag" data-tone={tone}>
      {label}
    </span>
  );
}

/**
 * A presentational next-steps checklist. Checkboxes toggle local-only state
 * (no persistence, no mutation) — mutating actions stay on the bundle-approval
 * flow. Shared by SpendingReviewCard month cards and the standalone ActionPlanCard.
 */
export function ActionChecklist({ title, items }: { title?: string; items: string[] }) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const toggle = (i: number) =>
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      return next;
    });
  return (
    <div className="cp-checklist">
      {title && <p className="cp-checklist-title eyebrow">{title}</p>}
      {items.map((text, i) => (
        <button
          type="button"
          key={i}
          className="cp-check-row"
          onClick={() => toggle(i)}
          aria-pressed={checked.has(i)}
        >
          <span className={`cp-check-box ${checked.has(i) ? "is-on" : ""}`}>
            {checked.has(i) && <I.Check width={11} height={11} />}
          </span>
          <span className="cp-check-txt">{text}</span>
        </button>
      ))}
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

import type { CopilotResponseBlock } from "../../../api/client";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { StatLine, TagPill } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "spendingDrivers" }>;

/**
 * "What changed vs your baseline" breakdown: one row per driver with a colored
 * dot, a tag pill (planned/trend/prices/anomaly/creep/mixed), a signed per-month
 * delta string, and a short note. Deltas are presentational strings copied from
 * tool output — the card never computes them.
 */
export function SpendingDriversCard({ block }: { block: Block }) {
  return (
    <div className="cp-card cp-drivers">
      <div className="cp-card-title">{block.title}</div>
      {block.subtitle && <StatLine parts={[block.subtitle]} />}
      <div className="cp-drivers-list">
        {block.drivers.map((d, i) => (
          <div key={`${d.label}-${i}`} className="cp-driver-row">
            <span className="cp-dot" style={{ background: colorForCategoryLabel(d.label) ?? "var(--ink-faint)" }} />
            <span className="cp-driver-label">{d.label}</span>
            <TagPill label={d.tag} tone={d.tag} />
            <span className="cp-driver-amt mono">{d.amountDisplay}</span>
            {d.note && <span className="cp-driver-note">{d.note}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

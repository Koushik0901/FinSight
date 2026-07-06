import type { CopilotResponseBlock } from "../../../api/client";

type Block = Extract<CopilotResponseBlock, { kind: "rankedOptions" }>;

const VERDICT_LABEL: Record<Block["options"][number]["rankTone"], string> = {
  primary: "Do this first",
  neutral: "With what's left",
  muted: "Not yet",
};

export function RankedOptionsCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-title">{block.title}</div>
      <div className="cp-options">
        {block.options.map((o, i) => (
          <div key={`${o.label}-${i}`} className={`cp-option ${o.rankTone === "primary" ? "is-primary" : ""}`}>
            <div className="cp-option-top">
              <span className={`cp-verdict cp-verdict-${o.rankTone}`}>{VERDICT_LABEL[o.rankTone]}</span>
              <span className="cp-option-detail mono">{o.detail}</span>
            </div>
            <div className="cp-option-label">{o.label}</div>
            <div className="cp-option-why">{o.rationale}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

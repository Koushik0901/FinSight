import type { CopilotResponseBlock } from "../../../api/client";
import * as I from "../../Icons";

type Block = Extract<CopilotResponseBlock, { kind: "affordabilityVerdict" }>;

export function AffordabilityVerdictCard({ block }: { block: Block }) {
  return (
    <div className="cp-card" style={{ overflow: "hidden" }}>
      <div className="cp-verdict-hero">
        <div className={`cp-verdict-big ${block.canAfford ? "pos" : "neg"}`}>{block.headline}</div>
        <div className="cp-verdict-sub">{block.sub}</div>
      </div>
      {block.caveat && (
        <div className="cp-caveat">
          <I.Bolt width={13} height={13} />
          <span>{block.caveat}</span>
        </div>
      )}
      {block.fundingSource && (
        <div className="cp-fund">
          <div className="cp-fund-label">{block.fundingSource.label}</div>
          <div className="cp-fund-detail mono">{block.fundingSource.detail}</div>
        </div>
      )}
    </div>
  );
}

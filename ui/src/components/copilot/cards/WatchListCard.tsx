import type { CopilotResponseBlock } from "../../../api/client";

type Block = Extract<CopilotResponseBlock, { kind: "watchList" }>;

/** A short numbered list of risks/things to watch, each a label + one-line detail
 *  and an optional cost string. The renderer supplies the ordinal. */
export function WatchListCard({ block }: { block: Block }) {
  return (
    <div className="cp-card cp-watch">
      <div className="cp-card-title">{block.title}</div>
      <div className="cp-watch-list">
        {block.items.map((it, i) => (
          <div key={`${it.label}-${i}`} className="cp-watch-row">
            <span className="cp-watch-n">{i + 1}</span>
            <div className="cp-watch-body">
              <span className="cp-watch-label">{it.label}</span>
              <span className="cp-watch-detail">{it.detail}</span>
            </div>
            {it.amountDisplay && <span className="cp-watch-amt mono">{it.amountDisplay}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

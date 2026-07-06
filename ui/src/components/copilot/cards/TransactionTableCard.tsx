import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";

type Block = Extract<CopilotResponseBlock, { kind: "transactionTable" }>;

export function TransactionTableCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-title">{block.count} transactions</div>
      <div className="cp-tx">
        {block.rows.map((r, i) => (
          <div key={i} className="cp-tx-row">
            <span className="cp-tx-date mono">{r.date}</span>
            <div className="cp-tx-merchant">
              <span className="cp-dot" style={{ background: colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)" }} />
              <span>{r.merchant}</span>
              {r.flag && <span className="cp-tx-flag">{r.flag}</span>}
            </div>
            <span className="cp-tx-cat">{r.categoryKey}</span>
            <span className="cp-tx-amt mono money">{money(r.amountCents)}</span>
          </div>
        ))}
        {block.more > 0 && (
          <div className="cp-tx-more">
            + {block.more} more · <span className="money">{money(block.totalCents)}</span> total
          </div>
        )}
      </div>
    </div>
  );
}

import { useState } from "react";
import { toast } from "sonner";
import type { CopilotResponseBlock } from "../../../api/client";
import { commands } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import Button from "../../Button";

type Block = Extract<CopilotResponseBlock, { kind: "transactionTable" }>;

export function TransactionTableCard({ block }: { block: Block }) {
  const [exporting, setExporting] = useState(false);
  // The originating search filters are carried on the block itself (attached
  // server-side from the search_transactions call), so the export re-runs the
  // exact same query. Absent when the table has no reliably-known query, in
  // which case we don't offer an export rather than dumping the whole table.
  const query = block.query;

  async function handleExport() {
    if (!query) return;
    setExporting(true);
    try {
      const result = await commands.exportSearchTransactionsCsv({
        merchant: query.merchant,
        account: query.account,
        startDate: query.startDate,
        endDate: query.endDate,
        minAmountCents: query.minAmountCents,
        direction: query.direction,
      });
      if (result.status === "ok") {
        if (result.data) {
          toast.success("Exported CSV", { description: result.data });
        }
      } else {
        toast.error("Export failed", { description: result.error.message });
      }
    } catch (e) {
      toast.error("Export failed", { description: String(e) });
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="cp-card">
      <div className="cp-card-title">{block.count} transaction{block.count === 1 ? "" : "s"}</div>
      <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)", marginTop: 4, marginBottom: 12 }}>
        <span className="mono money">{money(block.totalCents)}</span> total · top {block.rows.length} by size
      </div>
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
            + {block.more} more · <span className="mono money">{money(block.totalCents)}</span> total
          </div>
        )}
      </div>
      {query && (
        <div style={{ marginTop: 14 }}>
          <Button
            variant="primary"
            size="sm"
            loading={exporting}
            disabled={exporting}
            onClick={() => void handleExport()}
          >
            Export {block.count} as CSV
          </Button>
        </div>
      )}
    </div>
  );
}

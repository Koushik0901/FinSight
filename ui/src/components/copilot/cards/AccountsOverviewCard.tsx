import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { StatLine } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "accountsOverview" }>;

/**
 * Accounts overview table: a header count/summary plus one row per account with
 * a type chip and a right-aligned balance (negative in the negative token). An
 * account with no known balance carries a null amountCents and shows a badge
 * (e.g. "needs a balance set") instead — never a fabricated $0.
 *
 * Renders NOTHING when there are no rows. This kind is server-rendered: the
 * model emits a bare `{"kind":"accountsOverview"}` and the server fills the rows
 * from the ledger, so an empty block means the server declined to fill it —
 * which happens both when the user has no accounts and when the accounts read
 * failed. A titled empty table would assert "you have no accounts" in a case
 * where the truth may be "we could not read them". The server already drops
 * these; this mirrors that decision so the two sides agree.
 */
export function AccountsOverviewCard({ block }: { block: Block }) {
  const rows = block.rows ?? [];
  if (rows.length === 0) return null;
  return (
    <div className="cp-card cp-accounts">
      {(block.title || block.subtitle) && (
        <div className="cp-accounts-hd">
          {block.title && <div className="cp-card-title">{block.title}</div>}
          {block.subtitle && <StatLine parts={[block.subtitle]} />}
        </div>
      )}
      <div className="cp-accounts-rows">
        {rows.map((r, i) => (
          <div key={`${r.name}-${i}`} className="cp-account-row">
            <div className="cp-account-id">
              <span className="cp-account-name">{r.name}</span>
              {r.subtitle && <span className="cp-account-sub mono">{r.subtitle}</span>}
            </div>
            <span className="cp-account-type chip">{r.typeLabel}</span>
            {r.amountCents == null ? (
              <span className="cp-account-badge">{r.badge ?? "needs a balance set"}</span>
            ) : (
              <span className={`cp-account-bal mono money ${r.amountCents < 0 ? "is-neg" : ""}`}>
                {money(r.amountCents)}
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

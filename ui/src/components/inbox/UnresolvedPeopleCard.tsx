import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import Card from "../Card";
import Button from "../Button";
import { money } from "../../utils/format";
import { useApplyCounterpartyVerdict, useUnresolvedCounterparties } from "../../api/hooks/inbox";
import type { CounterpartyVerdict, UnresolvedCounterpartyDto } from "../../api/client";

const VERDICTS: { verdict: CounterpartyVerdict; label: string }[] = [
  { verdict: "transfer", label: "Transfer" },
  { verdict: "settleUp", label: "Settle-up" },
  { verdict: "real", label: "Real" },
];

const VERDICT_TOAST_LABEL: Record<CounterpartyVerdict, string> = {
  transfer: "transfer",
  settleUp: "settle-up",
  real: "real spending",
};

/** "$11,475 out · $3,000 in" — only the non-zero sides render, and each
 *  amount carries the `.money` privacy-blur class on its own. */
function NetFlow({ outflowCents, inflowCents }: { outflowCents: number; inflowCents: number }) {
  const sides: { key: string; cents: number; suffix: string }[] = [];
  if (outflowCents > 0) sides.push({ key: "out", cents: outflowCents, suffix: "out" });
  if (inflowCents > 0) sides.push({ key: "in", cents: inflowCents, suffix: "in" });
  if (sides.length === 0) return null;

  return (
    <>
      {sides.map((side, i) => (
        <span key={side.key}>
          {i > 0 ? " · " : ""}
          <span className="num money">{money(side.cents)}</span> {side.suffix}
        </span>
      ))}
    </>
  );
}

function CounterpartyRow({
  group,
  onVerdict,
}: {
  group: UnresolvedCounterpartyDto;
  onVerdict: (group: UnresolvedCounterpartyDto, verdict: CounterpartyVerdict) => void;
}) {
  const navigate = useNavigate();
  const isUnnamed = group.pattern === null;

  return (
    <div
      className="row-md wrap"
      data-testid={`counterparty-row-${group.pattern ?? "unnamed"}`}
      style={{ alignItems: "flex-start", justifyContent: "space-between", padding: "12px 0" }}
    >
      <div className="stack stack-xs grow">
        <div style={{ fontWeight: 600, fontSize: 14 }}>{group.label}</div>
        <div className="muted" style={{ fontSize: 13 }}>
          {group.txnCount} txn{group.txnCount === 1 ? "" : "s"} · <NetFlow outflowCents={group.outflowCents} inflowCents={group.inflowCents} />
        </div>
      </div>

      {isUnnamed ? (
        <Button variant="ghost" size="sm" onClick={() => navigate("/transactions?filter=transfer_review")}>
          Review individually →
        </Button>
      ) : (
        <div className="row-sm">
          {VERDICTS.map(({ verdict, label }) => (
            <Button key={verdict} variant="outline" size="sm" onClick={() => onVerdict(group, verdict)}>
              {label}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Grouped "People with unresolved money" review card. Lists every
 * undecided transfer-review counterparty, letting the user clear a whole
 * person's history with one click (Transfer / Settle-up / Real spending).
 * The "Unnamed internal transfers" bucket (bare `INTERNET TRANSFER <ref>`
 * rows, no shared pattern) can't be bulk-ruled — it links to the ledger for
 * one-by-one review instead.
 */
export default function UnresolvedPeopleCard() {
  const { data = [], isLoading } = useUnresolvedCounterparties();
  const applyVerdict = useApplyCounterpartyVerdict();
  const [removedPatterns, setRemovedPatterns] = useState<Set<string>>(new Set());

  const visible = data.filter((g) => g.pattern === null || !removedPatterns.has(g.pattern));

  if (isLoading || visible.length === 0) return null;

  const handleVerdict = (group: UnresolvedCounterpartyDto, verdict: CounterpartyVerdict) => {
    const pattern = group.pattern;
    if (!pattern) return;

    // Optimistically drop the row so the click reads as instant.
    setRemovedPatterns((prev) => new Set(prev).add(pattern));

    applyVerdict.mutate(
      { pattern, verdict },
      {
        onSuccess: (count) => {
          toast.success(
            `Ruled ${count} transaction${count === 1 ? "" : "s"} with ${group.label} as ${VERDICT_TOAST_LABEL[verdict]}`,
          );
        },
        onError: (err) => {
          toast.error(`Could not apply verdict for ${group.label}`, { description: String(err) });
          // Roll back the optimistic removal so the row reappears.
          setRemovedPatterns((prev) => {
            const next = new Set(prev);
            next.delete(pattern);
            return next;
          });
        },
      },
    );
  };

  return (
    <section className="stack stack-md" aria-labelledby="inbox-unresolved-people">
      <div id="inbox-unresolved-people" className="eyebrow">
        People with unresolved money ({visible.length})
      </div>
      <Card style={{ padding: "4px 20px" }}>
        <div className="stack">
          {visible.map((group, i) => (
            <div key={group.pattern ?? "unnamed"} style={i > 0 ? { borderTop: "1px solid var(--line)" } : undefined}>
              <CounterpartyRow group={group} onVerdict={handleVerdict} />
            </div>
          ))}
        </div>
      </Card>
    </section>
  );
}

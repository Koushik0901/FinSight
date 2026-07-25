import { useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import type { CategoryProposal, CategoryDto, Transaction } from "../api/client";
import {
  useCategoryProposals,
  useAcceptCategoryProposal,
  useCorrectCategoryProposal,
  useRejectCategoryProposal,
} from "../api/hooks/categoryProposals";
import { useCategories, useTransactions } from "../api/hooks/transactions";
import { useAccounts } from "../api/hooks/accounts";
import { money } from "../utils/format";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";
import CategoryPicker from "../components/CategoryPicker";
import * as I from "../components/Icons";

/** How many queue items render before the user asks for more. */
const PAGE_SIZE = 25;

/**
 * Size the enrichment fetch to cover the WHOLE queue.
 *
 * The two populations are identical by construction — `needs_review` is
 * literally `t.id IN (SELECT txn_id FROM category_proposals WHERE
 * status='pending')` — so `count` transactions is already full coverage; the
 * head-room absorbs a proposal recorded between the two queries.
 *
 * Quantized to whole hundreds so the value — and therefore the transactions
 * query key, which the filter is part of — does not move every time one item
 * is resolved.
 */
export function enrichmentLimit(count: number): number {
  return Math.max(1, Math.ceil((count + 25) / 100)) * 100;
}

/**
 * The enrichment limit for this session, which only ever GROWS.
 *
 * Quantizing alone is not enough: resolving one item can carry the count back
 * across a bucket boundary (76 -> 75 recomputes 200 -> 100), and a shrinking
 * limit is a brand-new query key with no cached data. The shared
 * `useTransactions` hook sets no `placeholderData`, so that key starts
 * `isLoading`, and the render gate below would blank the whole screen to a
 * loading stub right after an accept or dismiss.
 *
 * A high-water mark makes the key monotonic: it can only ever step up (to cover
 * a growing queue), never back down, so resolving items never re-keys.
 * Over-fetching a stale-high limit costs nothing — the population is bounded by
 * the pending proposals themselves, not by this number.
 */
function useEnrichmentLimit(count: number): number {
  const highWater = useRef(0);
  if (count > highWater.current) highWater.current = count;
  return enrichmentLimit(highWater.current);
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
}

/** A confidence percentage, rendered as a plain readable number. */
function confidenceLabel(confidence: number): string {
  return `${Math.round(confidence * 100)}% confident`;
}

interface RowProps {
  proposal: CategoryProposal;
  /** The proposal's transaction, when the enrichment fetch covered it. */
  txn: Transaction | undefined;
  /** The transaction's own account currency — never a hardcoded default, so a
   *  CAD row is not labelled in USD. */
  currency: string;
  /** The proposal's PROPOSED category — resolved from the proposal, never from
   *  the transaction's current canonical category (those are two different
   *  things whenever `applied` is false). */
  proposedCategory: CategoryDto | undefined;
}

export function ProposalRow({ proposal, txn, currency, proposedCategory }: RowProps) {
  const [picking, setPicking] = useState(false);
  const accept = useAcceptCategoryProposal();
  const correct = useCorrectCategoryProposal();
  const reject = useRejectCategoryProposal();

  const busy = accept.isPending || correct.isPending || reject.isPending;
  const categoryLabel = proposedCategory?.label ?? proposal.proposedCategoryId;
  const categoryColor = proposedCategory?.color ?? "var(--ink-faint)";
  const merchant = txn?.merchant_label ?? txn?.merchant_raw ?? "Unknown merchant";

  const onAccept = async () => {
    try {
      await accept.mutateAsync(proposal.id);
      toast.success("Categorization confirmed", { description: `${merchant} → ${categoryLabel}` });
    } catch (e) {
      toast.error("Could not confirm this categorization", {
        description: e instanceof Error ? e.message : undefined,
      });
    }
  };

  const onCorrect = async (categoryId: string) => {
    try {
      await correct.mutateAsync({ id: proposal.id, categoryId });
      setPicking(false);
      toast.success("Category changed", { description: merchant });
    } catch (e) {
      toast.error("Could not change the category", {
        description: e instanceof Error ? e.message : undefined,
      });
    }
  };

  const onReject = async () => {
    try {
      await reject.mutateAsync(proposal.id);
      toast("Dismissed from the review queue", {
        description: proposal.applied
          ? `${merchant} stays in ${categoryLabel}.`
          : `${merchant} stays uncategorized.`,
      });
    } catch (e) {
      toast.error("Could not dismiss this suggestion", {
        description: e instanceof Error ? e.message : undefined,
      });
    }
  };

  return (
    <Card className="stack stack-md" style={{ padding: "18px 20px" }}>
      <div className="row-md" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
        <div className="stack stack-xs grow">
          <div style={{ fontSize: 15, fontWeight: 600 }}>{merchant}</div>
          <div className="muted" style={{ fontSize: 12.5 }}>
            {txn ? (
              <>
                {formatDate(txn.posted_at)} ·{" "}
                <span className="num money">
                  {money(txn.amount_cents, { currency, decimals: 2 })}
                </span>
              </>
            ) : (
              // The enrichment fetch missed this row (it fell outside the page,
              // or the transaction changed underneath us). Degrade to what the
              // proposal itself knows rather than dropping the item — a queue
              // that silently hides entries can never be cleared.
              "Transaction details unavailable — the suggestion below still applies."
            )}
          </div>
        </div>
        <Badge tone={proposal.applied ? "default" : "warning"}>
          {proposal.applied ? "Already applied" : "Not applied"}
        </Badge>
      </div>

      <div className="row-sm" style={{ alignItems: "center", flexWrap: "wrap", gap: 8 }}>
        <span className="eyebrow" style={{ marginBottom: 0 }}>
          <I.Sparkle width={11} height={11} aria-hidden="true" />
          {proposal.source === "llm" ? "AI suggested" : `${proposal.source} suggested`}
        </span>
        <span
          className="chip"
          style={{ color: categoryColor, borderColor: categoryColor }}
          data-testid="proposed-category"
        >
          {categoryLabel}
        </span>
        <span className="muted num" style={{ fontSize: 12 }}>
          {confidenceLabel(proposal.confidence)}
        </span>
      </div>

      {proposal.rationale && (
        <p className="muted" style={{ fontSize: 13, lineHeight: 1.55, margin: 0 }}>
          “{proposal.rationale}”
        </p>
      )}

      {/*
        `status` and `applied` are two separate axes and the copy must not
        collapse them. Everything the LLM pass records today is applied=1 —
        the category is ALREADY counting toward budgets and reports, and
        reviewing it is a confirmation, not an activation. A future
        suggestion-only pass records applied=0, where the decision is what
        makes it real.
      */}
      <p className="muted" style={{ fontSize: 12.5, lineHeight: 1.55, margin: 0 }}>
        {proposal.applied
          ? `This category is already live — ${categoryLabel} counts toward your budgets and reports right now. Confirming records it as your own decision so the agent stops guessing.`
          : `Nothing has been written to this transaction yet. It stays uncategorized until you confirm or pick a category.`}
      </p>

      {picking ? (
        <div className="stack stack-sm">
          <div className="eyebrow" style={{ marginBottom: 0 }}>Pick the right category</div>
          <CategoryPicker value={txn?.category_id ?? null} onChange={(id) => void onCorrect(id)} />
          <div>
            <Button variant="ghost" size="sm" onClick={() => setPicking(false)} disabled={busy}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="stack stack-xs">
          <div className="row-sm" style={{ flexWrap: "wrap" }}>
            <Button
              variant="primary"
              size="sm"
              loading={accept.isPending}
              disabled={busy}
              aria-label={`Confirm ${categoryLabel} for ${merchant}`}
              onClick={() => void onAccept()}
            >
              Confirm {categoryLabel}
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={busy}
              aria-label={`Change category for ${merchant}`}
              onClick={() => setPicking(true)}
            >
              Change category…
            </Button>
            <Button
              variant="ghost"
              size="sm"
              loading={reject.isPending}
              disabled={busy}
              aria-label={`Dismiss the suggestion for ${merchant}`}
              onClick={() => void onReject()}
            >
              Dismiss
            </Button>
          </div>
          <div className="muted" style={{ fontSize: 12 }}>
            {proposal.applied
              ? `Dismiss keeps ${categoryLabel} on this transaction — it only clears the item from this queue.`
              : `Dismiss leaves this transaction uncategorized.`}
          </div>
        </div>
      )}
    </Card>
  );
}

export default function CategoryReview() {
  const navigate = useNavigate();
  const [visible, setVisible] = useState(PAGE_SIZE);
  const { data: proposals = [], isLoading, error } = useCategoryProposals();
  const { data: categories = [] } = useCategories();
  const { data: accounts = [] } = useAccounts();
  const limit = useEnrichmentLimit(proposals.length);

  // Enrichment only. Queue membership comes from `category_proposals`; this
  // fetch just supplies merchant/date/amount for the same rows.
  const { data: transactions = [], isLoading: enrichmentLoading } = useTransactions({
    accountId: null,
    limit,
    offset: null,
    search: null,
    filterPreset: "needs_review",
    startDate: null,
    endDate: null,
  });

  const txnById = useMemo(() => {
    const map = new Map<string, Transaction>();
    for (const t of transactions) map.set(t.id, t);
    return map;
  }, [transactions]);

  const categoryById = useMemo(() => {
    const map = new Map<string, CategoryDto>();
    for (const c of categories) map.set(c.id, c);
    return map;
  }, [categories]);

  const currencyByAccount = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of accounts) map.set(a.id, a.currency);
    return map;
  }, [accounts]);

  const shown = proposals.slice(0, visible);

  if (isLoading) return <div className="stub">Loading review queue…</div>;
  if (error) return <div className="stub">Error loading the review queue.</div>;
  // Hold the list back until the enrichment fetch has settled, so rows never
  // paint their "details unavailable" fallback on the way to having real data.
  if (proposals.length > 0 && enrichmentLoading) {
    return <div className="stub">Loading review queue…</div>;
  }

  return (
    <div className="screen screen-category-review">
      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Workshop · Review queue</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>
            {proposals.length === 0
              ? "Nothing to review."
              : `${proposals.length} categorization${proposals.length === 1 ? "" : "s"} to confirm.`}
          </h1>
        </div>
        <Button variant="outline" onClick={() => navigate("/transactions?filter=needs_review")}>
          Open in ledger
        </Button>
      </header>

      <p className="muted" style={{ maxWidth: 680, marginTop: -12, marginBottom: 28, fontSize: 14, lineHeight: 1.6 }}>
        When the agent categorizes a transaction it wasn’t sure about, it lands
        here. Confirming turns its guess into your decision — the agent learns
        from it and stops second-guessing similar merchants.
      </p>

      {proposals.length === 0 ? (
        <EmptyState
          icon={<I.Check style={{ color: "var(--ink-faint)", width: 24, height: 24 }} />}
          title="You're all caught up"
          description="Nothing is waiting on your review. New low-confidence suggestions show up here after the agent categorizes an import."
          compact
        />
      ) : (
        <div className="stack stack-md">
          {shown.map((p) => {
            const txn = txnById.get(p.txnId);
            return (
              <ProposalRow
                key={p.id}
                proposal={p}
                txn={txn}
                currency={(txn && currencyByAccount.get(txn.account_id)) || "USD"}
                proposedCategory={categoryById.get(p.proposedCategoryId)}
              />
            );
          })}
          {proposals.length > shown.length && (
            <div>
              <Button variant="outline" size="sm" onClick={() => setVisible((v) => v + PAGE_SIZE)}>
                Show more ({proposals.length - shown.length} left)
              </Button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

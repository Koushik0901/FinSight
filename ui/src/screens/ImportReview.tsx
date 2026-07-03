import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import {
  useAcceptImportCandidateMatch,
  useCreateImportCandidateTransaction,
  useDismissImportCandidate,
  useImportReviewCandidates,
} from "../api/hooks/simplefin";
import { useTransactions } from "../api/hooks/transactions";
import type { ImportCandidateMatch, ImportCandidateWithMatches, Transaction } from "../api/client";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";
import Button from "../components/Button";
import Card from "../components/Card";
import EmptyState from "../components/EmptyState";

function MatchCard({
  candidateId,
  match,
  matchTxn,
}: {
  candidateId: string;
  match: ImportCandidateMatch;
  matchTxn: Transaction | undefined;
}) {
  const accept = useAcceptImportCandidateMatch();

  return (
    <div
      className="card"
      style={{
        padding: 12,
        background: match.isRecommended ? "var(--surface-2)" : "transparent",
        borderColor: match.isRecommended ? "var(--accent)" : "var(--line)",
      }}
    >
      <div className="row" style={{ justifyContent: "space-between", gap: 12 }}>
        <div className="stack stack-xs">
          <div className="row row-sm wrap">
            <strong>{matchTxn?.merchant_raw ?? "Existing transaction"}</strong>
            {match.isRecommended && <span className="chip">Recommended</span>}
            <span className="muted">Match strength {Math.round(match.score)}</span>
          </div>
          <div className="muted" style={{ fontSize: 12.5 }}>
            {matchTxn ? `${new Date(matchTxn.posted_at).toLocaleDateString()} · ${money(matchTxn.amount_cents, { decimals: 2 })}` : "Transaction details unavailable"}
            {matchTxn?.category_label ? ` · ${matchTxn.category_label}` : ""}
          </div>
        </div>
        <Button
          variant="primary"
          size="sm"
          loading={accept.isPending}
          onClick={async () => {
            try {
              await accept.mutateAsync({ candidateId, transactionId: match.transactionId });
              toast.success("Matched imported transaction");
            } catch {
              toast.error("Could not accept match");
            }
          }}
        >
          Accept match
        </Button>
      </div>
    </div>
  );
}

function CandidateRow({
  item,
  accountName,
  transactionsById,
}: {
  item: ImportCandidateWithMatches;
  accountName: string;
  transactionsById: Record<string, Transaction>;
}) {
  const createTxn = useCreateImportCandidateTransaction();
  const dismiss = useDismissImportCandidate();

  const recommended = item.matches.find((match) => match.isRecommended) ?? item.matches[0];
  const otherMatches = item.matches.filter((match) => match.id !== recommended?.id);

  return (
    <Card style={{ marginBottom: 16 }}>
      <div className="row" style={{ justifyContent: "space-between", gap: 16, alignItems: "flex-start" }}>
        <div className="stack stack-sm" style={{ flex: 1, minWidth: 0 }}>
          <div className="row row-sm wrap">
            <strong>{item.candidate.merchantRaw}</strong>
            <span className="chip">{new Date(item.candidate.postedAt).toLocaleDateString()}</span>
            <span className="chip">{accountName}</span>
            <span className="chip">Match strength {Math.round(item.candidate.confidence)}</span>
          </div>
          <div className="row row-sm wrap">
            <span className={`money num${item.candidate.amountCents > 0 ? " pos" : ""}`}>
              {money(item.candidate.amountCents, { decimals: 2 })}
            </span>
            <span className="muted">{item.candidate.reason}</span>
          </div>

          {recommended ? (
            <div className="stack stack-sm">
              <div className="eyebrow">Proposed match</div>
              <MatchCard
                candidateId={item.candidate.id}
                match={recommended}
                matchTxn={transactionsById[recommended.transactionId]}
              />
              {otherMatches.length > 0 && (
                <div className="stack stack-xs">
                  <div className="muted" style={{ fontSize: 12 }}>Other possible matches</div>
                  {otherMatches.map((match) => (
                    <MatchCard
                      key={match.id}
                      candidateId={item.candidate.id}
                      match={match}
                      matchTxn={transactionsById[match.transactionId]}
                    />
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="muted" style={{ fontSize: 13 }}>No match found. Create a fresh transaction instead.</div>
          )}
        </div>

        <div className="stack stack-sm" style={{ minWidth: 160 }}>
          <Button
            variant="outline"
            loading={createTxn.isPending}
            onClick={async () => {
              try {
                await createTxn.mutateAsync(item.candidate.id);
                toast.success("Created transaction from import");
              } catch {
                toast.error("Could not create transaction");
              }
            }}
          >
            {recommended ? "Create new" : "Create new transaction"}
          </Button>
          <Button
            variant="ghost"
            loading={dismiss.isPending}
            onClick={async () => {
              try {
                await dismiss.mutateAsync(item.candidate.id);
                toast.success("Dismissed candidate");
              } catch {
                toast.error("Could not dismiss candidate");
              }
            }}
          >
            Dismiss
          </Button>
        </div>
      </div>
    </Card>
  );
}

export default function ImportReview() {
  const { data: candidates = [], isLoading, error } = useImportReviewCandidates();
  const { data: accounts = [] } = useAccounts();
  const { data: transactions = [] } = useTransactions({
    accountId: null,
    limit: 5000,
    offset: 0,
    search: null,
    filterPreset: null,
    startDate: null,
    endDate: null,
  });

  const accountById = Object.fromEntries(accounts.map((account) => [account.id, getAccountDisplayName(account)]));
  const transactionsById = Object.fromEntries(transactions.map((txn) => [txn.id, txn]));

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true">
        <span className="spinner" aria-hidden="true" />
        <span style={{ marginTop: 12 }}>Loading import review…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="stub" role="alert">
        {(error as Error).message}
      </div>
    );
  }

  return (
    <div className="screen">
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Import Review · {candidates.length} pending
          </div>
          <h1>Reconcile imported activity before it lands.</h1>
        </div>
      </header>

      {candidates.length === 0 ? (
        <EmptyState
          title="No pending import candidates"
          description="Everything imported cleanly. New matches and create-vs-merge decisions will appear here."
        />
      ) : (
        <div className="stack stack-md">
          {candidates.map((item) => (
            <CandidateRow
              key={item.candidate.id}
              item={item}
              accountName={accountById[item.candidate.accountId] ?? "Unknown account"}
              transactionsById={transactionsById}
            />
          ))}
        </div>
      )}
    </div>
  );
}

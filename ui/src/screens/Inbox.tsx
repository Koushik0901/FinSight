import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import type { ActionItem, ImportCandidateWithMatches, SimpleFinAlert, TransferSuggestionInfo } from "../api/client";
import { useActionItems, useUnresolvedCounterparties } from "../api/hooks/inbox";
import { useNotifications } from "../api/hooks/notifications";
import { useTriggerRecategorizeLowConfidence } from "../api/hooks/agent";
import {
  useSimpleFinAlerts,
  useAcknowledgeSimpleFinAlert,
  useSimpleFinTransferSuggestions,
  useConfirmSimpleFinTransfer,
  useRejectSimpleFinTransfer,
  useImportReviewCandidates,
  useAcceptImportCandidateMatch,
  useCreateImportCandidateTransaction,
  useDismissImportCandidate,
} from "../api/hooks/simplefin";
import { money } from "../utils/format";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";
import * as I from "../components/Icons";
import { CopilotNudge } from "../components/CopilotNudge";
import UnresolvedPeopleCard from "../components/inbox/UnresolvedPeopleCard";
import NotificationsInboxSection from "../components/inbox/NotificationsInboxSection";

const CATEGORY_ICONS: Record<string, React.FC<React.SVGProps<SVGSVGElement>>> = {
  review:  I.Flow,
  bills:   I.Repeat,
  budget:  I.Lego,
  goals:   I.Goal,
  savings: I.Wallet,
};

const CATEGORY_TONES: Record<string, "default" | "accent" | "positive" | "negative" | "warning"> = {
  review:  "accent",
  bills:   "warning",
  budget:  "accent",
  goals:   "positive",
  savings: "positive",
};

const PRIORITY_TONES: Record<string, "default" | "accent" | "positive" | "negative" | "warning"> = {
  high:   "negative",
  medium: "warning",
  low:    "default",
};

const PRIORITY_LABELS: Record<string, string> = {
  high:   "High",
  medium: "Medium",
  low:    "Low",
};

const CATEGORY_LABELS: Record<string, string> = {
  review:  "Needs review",
  bills:   "Bills & subscriptions",
  budget:  "Budget",
  goals:   "Goals",
  savings: "Savings & emergency fund",
};

function ActionItemCard({ item, onRerunAi }: { item: ActionItem; onRerunAi?: () => void }) {
  const navigate = useNavigate();
  const CategoryIcon = CATEGORY_ICONS[item.category] ?? I.Bell;
  const catTone = CATEGORY_TONES[item.category] ?? "default";

  const handleAction = () => {
    const [path, query] = item.actionRoute.split("?");
    navigate(path + (query ? `?${query}` : ""));
  };

  return (
    <Card
      className="stack stack-sm"
      style={{
        padding: "18px 20px",
        borderLeftWidth: 3,
        borderLeftColor:
          item.priority === "high" ? "var(--negative)" :
          item.priority === "medium" ? "var(--warning)" :
          "var(--line)",
      }}
    >
      <div className="row-md" style={{ alignItems: "flex-start" }}>
        <div
          className="row"
          style={{
            width: 34,
            height: 34,
            borderRadius: 8,
            background: "var(--surface-2)",
            justifyContent: "center",
            flexShrink: 0,
          }}
          aria-hidden="true"
        >
          <CategoryIcon width={16} height={16} style={{ color: "var(--accent)" }} />
        </div>

        <div className="grow stack stack-xs">
          <div className="row-sm wrap" style={{ marginBottom: 4 }}>
            <Badge tone={PRIORITY_TONES[item.priority] ?? "default"}>{PRIORITY_LABELS[item.priority] ?? item.priority}</Badge>
            <Badge tone={catTone}>{CATEGORY_LABELS[item.category] ?? item.category}</Badge>
            {typeof item.badgeCount === "number" && item.badgeCount > 0 && (
              <Badge>{item.badgeCount}</Badge>
            )}
          </div>

          <div style={{ fontSize: 14.5, fontWeight: 600, lineHeight: 1.4 }}>{item.title}</div>
        </div>

        {typeof item.amountCents === "number" && (
          <div className="num money" style={{ fontSize: 13, color: "var(--ink-mute)", flexShrink: 0 }}>
            {money(Math.abs(item.amountCents))}
          </div>
        )}
      </div>

      <p className="muted" style={{ fontSize: 13, lineHeight: 1.6, paddingLeft: 46, margin: 0 }}>
        {item.detail}
      </p>

      <div className="row-sm" style={{ paddingLeft: 46 }}>
        <Button variant="default" size="sm" onClick={handleAction}>
          {item.actionLabel} →
        </Button>
        {onRerunAi && (
          <Button variant="ghost" size="sm" onClick={onRerunAi} title="Re-run AI categorization on uncertain transactions">
            ↻ Re-run AI
          </Button>
        )}
      </div>
    </Card>
  );
}

function AlertCard({ alert }: { alert: SimpleFinAlert }) {
  const ack = useAcknowledgeSimpleFinAlert();
  const tone =
    alert.severity === "error" ? "negative" :
    alert.severity === "warning" ? "warning" :
    "default";

  return (
    <Card
      className="stack stack-sm"
      style={{
        padding: "18px 20px",
        borderLeftWidth: 3,
        borderLeftColor:
          alert.severity === "error" ? "var(--negative)" :
          alert.severity === "warning" ? "var(--warning)" :
          "var(--line)",
      }}
    >
      <div className="row-md" style={{ alignItems: "flex-start" }}>
        <div
          className="row"
          style={{
            width: 34,
            height: 34,
            borderRadius: 8,
            background: "var(--surface-2)",
            justifyContent: "center",
            flexShrink: 0,
          }}
          aria-hidden="true"
        >
          <I.Bell width={16} height={16} style={{ color: "var(--accent)" }} />
        </div>
        <div className="grow stack stack-xs">
          <div className="row-sm wrap" style={{ marginBottom: 4 }}>
            <Badge tone={tone}>{alert.severity}</Badge>
            <Badge tone="default">Sync</Badge>
          </div>
          <div style={{ fontSize: 14.5, fontWeight: 600, lineHeight: 1.4 }}>{alert.message}</div>
        </div>
      </div>
      <div className="row-sm" style={{ paddingLeft: 46 }}>
        <Button
          variant="ghost"
          size="sm"
          onClick={() =>
            ack.mutate(alert.id, {
              onSuccess: () => toast.success("Alert dismissed"),
              onError: () => toast.error("Could not dismiss alert"),
            })
          }
          loading={ack.isPending}
        >
          Dismiss
        </Button>
      </div>
    </Card>
  );
}

function TransferSuggestionCard({ suggestion }: { suggestion: TransferSuggestionInfo }) {
  const confirm = useConfirmSimpleFinTransfer();
  const reject = useRejectSimpleFinTransfer();
  const tone =
    suggestion.confidence === "high" ? "positive" :
    suggestion.confidence === "medium" ? "warning" :
    "default";

  return (
    <Card
      className="stack stack-sm"
      style={{
        padding: "18px 20px",
        borderLeftWidth: 3,
        borderLeftColor:
          suggestion.confidence === "high" ? "var(--positive)" :
          suggestion.confidence === "medium" ? "var(--warning)" :
          "var(--line)",
      }}
    >
      <div className="row-md" style={{ alignItems: "flex-start" }}>
        <div
          className="row"
          style={{
            width: 34,
            height: 34,
            borderRadius: 8,
            background: "var(--surface-2)",
            justifyContent: "center",
            flexShrink: 0,
          }}
          aria-hidden="true"
        >
          <I.ArrowRight width={16} height={16} style={{ color: "var(--accent)" }} />
        </div>
        <div className="grow stack stack-xs">
          <div className="row-sm wrap" style={{ marginBottom: 4 }}>
            <Badge tone={tone}>{suggestion.confidence} confidence</Badge>
            <Badge tone="default">Transfer</Badge>
          </div>
          <div style={{ fontSize: 14.5, fontWeight: 600, lineHeight: 1.4 }}>
            Transfer from {suggestion.fromAccountName} to {suggestion.toAccountName}
          </div>
        </div>
      </div>
      <p className="muted" style={{ fontSize: 13, lineHeight: 1.6, paddingLeft: 46, margin: 0 }}>
        {suggestion.fromMerchant} ({money(suggestion.fromAmountCents)}) →{" "}
        {suggestion.toMerchant} ({money(suggestion.toAmountCents)})
      </p>
      <div className="row-sm" style={{ paddingLeft: 46 }}>
        <Button
          variant="default"
          size="sm"
          onClick={() =>
            confirm.mutate(suggestion.id, {
              onSuccess: () => toast.success("Transfer confirmed"),
              onError: () => toast.error("Could not confirm transfer"),
            })
          }
          loading={confirm.isPending}
        >
          Confirm
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() =>
            reject.mutate(suggestion.id, {
              onSuccess: () => toast.success("Suggestion removed"),
              onError: () => toast.error("Could not remove suggestion"),
            })
          }
          loading={reject.isPending}
        >
          Not a transfer
        </Button>
      </div>
    </Card>
  );
}

function ImportReviewCard({ item }: { item: ImportCandidateWithMatches }) {
  const accept = useAcceptImportCandidateMatch();
  const createNew = useCreateImportCandidateTransaction();
  const dismiss = useDismissImportCandidate();
  const { candidate, matches } = item;
  const recommended = matches.find((m) => m.isRecommended) ?? matches[0] ?? null;
  const source = candidate.source === "simplefin" ? "SimpleFIN" : "CSV";

  return (
    <Card
      className="stack stack-sm"
      style={{
        padding: "18px 20px",
        borderLeftWidth: 3,
        borderLeftColor: candidate.confidence >= 85 ? "var(--warning)" : "var(--accent)",
      }}
    >
      <div className="row-md" style={{ alignItems: "flex-start" }}>
        <div
          className="row"
          style={{
            width: 34,
            height: 34,
            borderRadius: 8,
            background: "var(--surface-2)",
            justifyContent: "center",
            flexShrink: 0,
          }}
          aria-hidden="true"
        >
          <I.Flow width={16} height={16} style={{ color: "var(--accent)" }} />
        </div>
        <div className="grow stack stack-xs">
          <div className="row-sm wrap" style={{ marginBottom: 4 }}>
            <Badge tone="accent">{source}</Badge>
            <Badge tone={candidate.confidence >= 85 ? "warning" : "default"}>
              {candidate.confidence}% confidence
            </Badge>
          </div>
          <div style={{ fontSize: 14.5, fontWeight: 600, lineHeight: 1.4 }}>
            Review {candidate.merchantRaw}
          </div>
        </div>
        <div className="num money" style={{ fontSize: 13, color: "var(--ink-mute)", flexShrink: 0 }}>
          {money(candidate.amountCents)}
        </div>
      </div>

      <p className="muted" style={{ fontSize: 13, lineHeight: 1.6, paddingLeft: 46, margin: 0 }}>
        {candidate.reason}. Posted {new Date(candidate.postedAt).toLocaleDateString()}.
        {recommended && (
          <>
            {" "}Recommended match: transaction {recommended.transactionId.slice(0, 8)} · score {recommended.score}.
          </>
        )}
      </p>

      {matches.length > 1 && (
        <div className="stack stack-xs" style={{ paddingLeft: 46 }}>
          {matches.slice(0, 3).map((match) => (
            <button
              key={match.id}
              type="button"
              className="chip"
              onClick={() =>
                accept.mutate(
                  { candidateId: candidate.id, transactionId: match.transactionId },
                  {
                    onSuccess: () => toast.success("Import candidate matched"),
                    onError: () => toast.error("Could not match candidate"),
                  },
                )
              }
            >
              Match {match.transactionId.slice(0, 8)} · {match.score}
              {match.isRecommended ? " · recommended" : ""}
            </button>
          ))}
        </div>
      )}

      <div className="row-sm wrap" style={{ paddingLeft: 46 }}>
        {recommended && (
          <Button
            variant="default"
            size="sm"
            onClick={() =>
              accept.mutate(
                { candidateId: candidate.id, transactionId: recommended.transactionId },
                {
                  onSuccess: () => toast.success("Import candidate matched"),
                  onError: () => toast.error("Could not match candidate"),
                },
              )
            }
            loading={accept.isPending}
          >
            Accept match
          </Button>
        )}
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            createNew.mutate(candidate.id, {
              onSuccess: () => toast.success("Transaction created"),
              onError: () => toast.error("Could not create transaction"),
            })
          }
          loading={createNew.isPending}
        >
          Create new
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() =>
            dismiss.mutate(candidate.id, {
              onSuccess: () => toast.success("Candidate dismissed"),
              onError: () => toast.error("Could not dismiss candidate"),
            })
          }
          loading={dismiss.isPending}
        >
          Dismiss
        </Button>
      </div>
    </Card>
  );
}

export default function Inbox() {
  const { data: items = [], isLoading, error, dataUpdatedAt } = useActionItems();
  const { data: alerts = [] } = useSimpleFinAlerts();
  const { data: transfers = [] } = useSimpleFinTransferSuggestions();
  const { data: importReview = [] } = useImportReviewCandidates();
  const { data: unresolvedCounterparties = [] } = useUnresolvedCounterparties();
  const { data: notifications = [] } = useNotifications();
  const qc = useQueryClient();
  const rerunAi = useTriggerRecategorizeLowConfidence();

  const highItems = items.filter((i) => i.priority === "high");
  const mediumItems = items.filter((i) => i.priority === "medium");
  const lowItems = items.filter((i) => i.priority === "low");

  const allCount =
    items.length + alerts.length + transfers.length + importReview.length + unresolvedCounterparties.length + notifications.length;

  const lastUpdated = dataUpdatedAt
    ? new Date(dataUpdatedAt).toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })
    : null;

  const handleRefresh = () => {
    void qc.invalidateQueries({ queryKey: ["action-items"] });
    void qc.invalidateQueries({ queryKey: ["simplefin", "alerts"] });
    void qc.invalidateQueries({ queryKey: ["simplefin", "transfers"] });
    void qc.invalidateQueries({ queryKey: ["simplefin", "importReview"] });
    void qc.invalidateQueries({ queryKey: ["unresolved-counterparties"] });
    void qc.invalidateQueries({ queryKey: ["notifications"] });
  };

  const handleRerunAi = () => {
    rerunAi.mutate(undefined, {
      onSuccess: () => toast.success("Re-categorization queued", { description: "The AI will re-check uncertain categories shortly." }),
      onError: (e) => toast.error("Could not queue re-check", { description: String(e) }),
    });
  };

  if (isLoading) return <div className="stub">Scanning your finances…</div>;
  if (error) return <div className="stub">Error loading inbox.</div>;

  return (
    <div className="screen">
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Inbox · {allCount} item{allCount !== 1 ? "s" : ""}
          </div>
          <h1>What needs your attention.</h1>
        </div>
        <div className="row-md wrap">
          {allCount > 0 && (
            <CopilotNudge
              prompt="I have some action items in my financial inbox. Help me prioritize and tackle them one by one."
              label="Help me work through these"
              variant="accent"
            />
          )}
          <Button variant="ghost" size="icon" onClick={handleRefresh} title="Refresh inbox" aria-label="Refresh inbox">
            <I.Repeat width={14} height={14} />
          </Button>
        </div>
      </header>

      <p className="muted" style={{ maxWidth: 620, fontSize: 14, lineHeight: 1.6, marginTop: -12, marginBottom: 24 }}>
        Prioritized actions computed from your live data — no manual curation needed.
        {lastUpdated && (
          <span style={{ marginLeft: 8, fontSize: 12, color: "var(--ink-faint)" }}>Updated {lastUpdated}</span>
        )}
      </p>

      {importReview.length > 0 && (
        <section className="stack stack-md" style={{ marginBottom: 24 }} aria-labelledby="inbox-import-review">
          <div id="inbox-import-review" className="eyebrow">
            Import review · {importReview.length}
          </div>
          <div className="stack stack-md">
            {importReview.map((candidate) => (
              <ImportReviewCard key={candidate.candidate.id} item={candidate} />
            ))}
          </div>
        </section>
      )}

      {(alerts.length > 0 || transfers.length > 0) && (
        <section className="stack stack-md" style={{ marginBottom: 24 }} aria-labelledby="inbox-simplefin">
          <div id="inbox-simplefin" className="eyebrow">
            Bank sync
          </div>
          <div className="stack stack-md">
            {alerts.map((alert) => (
              <AlertCard key={alert.id} alert={alert} />
            ))}
            {transfers.map((t) => (
              <TransferSuggestionCard key={t.id} suggestion={t} />
            ))}
          </div>
        </section>
      )}

      {unresolvedCounterparties.length > 0 && (
        <div style={{ marginBottom: 24 }}>
          <UnresolvedPeopleCard />
        </div>
      )}

      <NotificationsInboxSection notifications={notifications} />

      {allCount === 0 ? (
        <EmptyState
          icon={<I.Check style={{ color: "var(--positive)", width: 40, height: 40 }} />}
          title="All clear"
          description="Your finances look healthy right now. Come back tomorrow — the inbox refreshes automatically as new data arrives."
          actions={
            <CopilotNudge
              prompt="My financial inbox is clear. What's one thing I should be working on right now to move forward on my financial journey?"
              label="What should I focus on next?"
              variant="accent"
            />
          }
        />
      ) : (
        <div className="stack stack-2xl">
          {highItems.length > 0 && (
            <section className="stack stack-md" aria-labelledby="inbox-high">
              <div id="inbox-high" className="eyebrow">
                <span
                  className="dot"
                  style={{ background: "var(--negative)", boxShadow: "0 0 6px var(--negative)" }}
                />
                High priority — {highItems.length} item{highItems.length !== 1 ? "s" : ""}
              </div>
              <div className="stack stack-md">
                {highItems.map((item) => (
                  <ActionItemCard
                    key={item.id}
                    item={item}
                    onRerunAi={item.id === "low-confidence-categorizations" ? handleRerunAi : undefined}
                  />
                ))}
              </div>
            </section>
          )}

          {mediumItems.length > 0 && (
            <section className="stack stack-md" aria-labelledby="inbox-medium">
              <div id="inbox-medium" className="eyebrow">
                <span
                  className="dot"
                  style={{ background: "var(--warning)", boxShadow: "0 0 6px var(--warning)" }}
                />
                Medium priority — {mediumItems.length} item{mediumItems.length !== 1 ? "s" : ""}
              </div>
              <div className="stack stack-md">
                {mediumItems.map((item) => (
                  <ActionItemCard
                    key={item.id}
                    item={item}
                    onRerunAi={item.id === "low-confidence-categorizations" ? handleRerunAi : undefined}
                  />
                ))}
              </div>
            </section>
          )}

          {lowItems.length > 0 && (
            <section className="stack stack-md" aria-labelledby="inbox-low">
              <div id="inbox-low" className="eyebrow">
                <span
                  className="dot"
                  style={{ background: "var(--ink-faint)", boxShadow: "none" }}
                />
                Low priority — {lowItems.length} item{lowItems.length !== 1 ? "s" : ""}
              </div>
              <div className="stack stack-md">
                {lowItems.map((item) => (
                  <ActionItemCard
                    key={item.id}
                    item={item}
                    onRerunAi={item.id === "low-confidence-categorizations" ? handleRerunAi : undefined}
                  />
                ))}
              </div>
            </section>
          )}
        </div>
      )}
    </div>
  );
}

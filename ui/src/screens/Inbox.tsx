import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import type { ActionItem } from "../api/client";
import { useActionItems } from "../api/hooks/inbox";
import { money } from "../utils/format";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";
import * as I from "../components/Icons";
import { CopilotNudge } from "../components/CopilotNudge";

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

function ActionItemCard({ item }: { item: ActionItem }) {
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

      <div style={{ paddingLeft: 46 }}>
        <Button variant="default" size="sm" onClick={handleAction}>
          {item.actionLabel} →
        </Button>
      </div>
    </Card>
  );
}

export default function Inbox() {
  const { data: items = [], isLoading, error, dataUpdatedAt } = useActionItems();
  const qc = useQueryClient();

  const highItems = items.filter((i) => i.priority === "high");
  const mediumItems = items.filter((i) => i.priority === "medium");
  const lowItems = items.filter((i) => i.priority === "low");

  const lastUpdated = dataUpdatedAt
    ? new Date(dataUpdatedAt).toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })
    : null;

  const handleRefresh = () => {
    void qc.invalidateQueries({ queryKey: ["action-items"] });
  };

  if (isLoading) return <div className="stub">Scanning your finances…</div>;
  if (error) return <div className="stub">Error loading inbox.</div>;

  return (
    <div className="screen">
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Inbox · {items.length} item{items.length !== 1 ? "s" : ""}
          </div>
          <h1>What needs your attention.</h1>
        </div>
        <div className="row-md wrap">
          {items.length > 0 && (
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

      {items.length === 0 ? (
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
                  <ActionItemCard key={item.id} item={item} />
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
                  <ActionItemCard key={item.id} item={item} />
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
                  <ActionItemCard key={item.id} item={item} />
                ))}
              </div>
            </section>
          )}
        </div>
      )}
    </div>
  );
}

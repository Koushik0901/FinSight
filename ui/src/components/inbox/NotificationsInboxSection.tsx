import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import type { Notification } from "../../api/client";
import { useMarkNotificationRead, useMarkAllNotificationsRead } from "../../api/hooks/notifications";
import Card from "../Card";
import Button from "../Button";
import Badge from "../Badge";
import * as I from "../Icons";

// Category → glyph. Unknown categories (a future NotificationCategory the UI
// hasn't been taught yet) fall back to the bell, so nothing renders blank.
const CATEGORY_ICONS: Record<string, React.FC<React.SVGProps<SVGSVGElement>>> = {
  cashflow_risk: I.Bolt,
  stale_data: I.Repeat,
  debt_deadline: I.Today,
  subscription_change: I.Repeat,
  categorization: I.Flow,
  goal_progress: I.Goal,
  month_end_review: I.Lego,
  security: I.Lock,
  sync_error: I.Bolt,
  account_activity: I.Spark,
};

const CATEGORY_LABELS: Record<string, string> = {
  cashflow_risk: "Cash-flow risk",
  stale_data: "Stale data",
  debt_deadline: "Debt deadline",
  subscription_change: "Subscription",
  categorization: "Categorization",
  goal_progress: "Goal progress",
  month_end_review: "Month-end review",
  security: "Security",
  sync_error: "Sync error",
  account_activity: "Account activity",
};

// Prettify an unmapped enum string ("some_new_kind" → "Some new kind") so a
// category added server-side still reads cleanly here without a UI change.
function prettify(cat: string): string {
  return cat.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());
}

function NotificationRow({ n }: { n: Notification }) {
  const navigate = useNavigate();
  const markRead = useMarkNotificationRead();
  const Icon = CATEGORY_ICONS[n.category] ?? I.Bell;
  const unread = n.readAt == null;
  const held = n.deliveredAt == null; // recorded but withheld (quiet hours) — waits here, never pushed.
  const critical = n.urgency === "critical";

  const open = () => {
    if (unread) markRead.mutate(n.id);
    if (n.route) {
      const [path, query] = n.route.split("?");
      navigate(path + (query ? `?${query}` : ""));
    }
  };

  return (
    <Card
      className="stack stack-sm"
      style={{
        padding: "16px 18px",
        borderLeftWidth: 3,
        borderLeftColor: critical ? "var(--negative)" : unread ? "var(--accent)" : "var(--line)",
        cursor: n.route ? "pointer" : "default",
        opacity: unread ? 1 : 0.7,
      }}
      onClick={n.route ? open : undefined}
    >
      <div className="row-md" style={{ alignItems: "flex-start" }}>
        <div
          className="row"
          style={{ width: 32, height: 32, borderRadius: 8, background: "var(--surface-2)", justifyContent: "center", flexShrink: 0 }}
          aria-hidden="true"
        >
          <Icon width={15} height={15} style={{ color: critical ? "var(--negative)" : "var(--accent)" }} />
        </div>

        <div className="grow stack stack-xs">
          <div className="row-sm wrap" style={{ marginBottom: 2 }}>
            {critical && <Badge tone="negative">Urgent</Badge>}
            <Badge tone="default">{CATEGORY_LABELS[n.category] ?? prettify(n.category)}</Badge>
            {held && <Badge tone="warning">Held · quiet hours</Badge>}
          </div>
          <div style={{ fontSize: 14, fontWeight: 600, lineHeight: 1.4 }}>{n.title}</div>
          <p className="muted" style={{ fontSize: 12.5, lineHeight: 1.55, margin: 0 }}>
            {n.body}
            {n.sensitive && (
              <>
                {" · "}
                <span className="money">{n.sensitive}</span>
              </>
            )}
          </p>
        </div>

        {unread && (
          <span
            title="Unread"
            aria-label="Unread"
            style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--accent)", display: "inline-block", flexShrink: 0, marginTop: 6 }}
          />
        )}
      </div>
    </Card>
  );
}

/**
 * The notification history surface inside the Inbox — the readable record of
 * everything `finsight-core::notify` decided to keep. Active (unresolved) items
 * only; held-overnight items show up here with a badge since they never pushed.
 */
export default function NotificationsInboxSection({ notifications }: { notifications: Notification[] }) {
  const markAll = useMarkAllNotificationsRead();
  if (notifications.length === 0) return null;
  const unreadCount = notifications.filter((n) => n.readAt == null).length;

  return (
    <section className="stack stack-md" style={{ marginBottom: 24 }} aria-labelledby="inbox-notifications">
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <div id="inbox-notifications" className="eyebrow">
          Notifications · {notifications.length}
        </div>
        {unreadCount > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() =>
              markAll.mutate(undefined, {
                onSuccess: () => toast.success("All notifications marked read"),
                onError: () => toast.error("Could not mark all read"),
              })
            }
            loading={markAll.isPending}
          >
            Mark all read
          </Button>
        )}
      </div>
      <div className="stack stack-md">
        {notifications.map((n) => (
          <NotificationRow key={n.id} n={n} />
        ))}
      </div>
    </section>
  );
}

import { useEffect } from "react";
import { useInboxBadgeCount } from "../api/hooks/inbox";
import { useNotificationUnreadCount } from "../api/hooks/notifications";
import { clearAppBadge, syncAppBadge } from "./badge";

/**
 * Keeps the installed PWA's icon badge in sync with the user's total "needs
 * attention" count: the Inbox action total plus unread notifications.
 *
 * Mounted once, app-wide (App.tsx) rather than on the Inbox screen — the whole
 * value of a badge is that it is correct while the user is somewhere else, so
 * binding it to the screen that already shows the number would defeat it.
 *
 * Scope note: this keeps the badge fresh while the app is OPEN. Updating it
 * with the app fully closed requires a push event to wake the service worker
 * and is not part of this hook.
 */
export function useAppBadge(): void {
  const { data: inbox } = useInboxBadgeCount();
  const { data: notifUnread } = useNotificationUnreadCount();
  const inboxTotal = inbox?.total;

  useEffect(() => {
    // `undefined` is "not loaded yet", which is NOT the same as zero. Writing 0
    // on first render would visibly clear a badge the OS is already showing and
    // then re-add it a moment later once the query resolves. Only skip while
    // BOTH sources are still loading — a badge from whichever lands first is
    // better than none, and it refines when the second resolves.
    if (inboxTotal === undefined && notifUnread === undefined) return;
    void syncAppBadge((inboxTotal ?? 0) + (notifUnread ?? 0));
  }, [inboxTotal, notifUnread]);

  // Drop the badge when the app tears down (logout unmounts the tree). Leaving
  // a stale count on a signed-out device advertises the previous user's
  // activity on a shared machine.
  useEffect(() => {
    return () => {
      void clearAppBadge();
    };
  }, []);
}

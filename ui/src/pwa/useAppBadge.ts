import { useEffect } from "react";
import { useInboxBadgeCount } from "../api/hooks/inbox";
import { clearAppBadge, syncAppBadge } from "./badge";

/**
 * Keeps the installed PWA's icon badge in sync with the Inbox's "needs
 * attention" total.
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
  const { data } = useInboxBadgeCount();
  const total = data?.total;

  useEffect(() => {
    // `undefined` is "not loaded yet", which is NOT the same as zero. Writing 0
    // on first render would visibly clear a badge the OS is already showing and
    // then re-add it a moment later once the query resolves.
    if (total === undefined) return;
    void syncAppBadge(total);
  }, [total]);

  // Drop the badge when the app tears down (logout unmounts the tree). Leaving
  // a stale count on a signed-out device advertises the previous user's
  // activity on a shared machine.
  useEffect(() => {
    return () => {
      void clearAppBadge();
    };
  }, []);
}

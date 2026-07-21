import { listen } from "@tauri-apps/api/event";
import { notificationPermission } from "./push";

/**
 * Native OS notifications for server-pushed events the user would want to know
 * about while they are looking at something else.
 *
 * The pre-pivot desktop app surfaced a native notification when the Copilot
 * finished a long answer with the window unfocused. That was dropped when the
 * app became a thin shell that just navigates to the server and consumes SSE
 * in the webview. This rebuilds it on the SSE pipeline — and because the served
 * frontend runs identically in the desktop shell, the browser, and the
 * installed PWA, it lights up in all three rather than only the desktop.
 *
 * It never prompts: it fires only when notification permission is ALREADY
 * granted (through the push-notification settings flow), and only when the tab
 * is genuinely in the background. Notifying someone who is already watching the
 * screen is noise, and prompting for permission out of nowhere is a dark
 * pattern.
 */

/** The completion events worth interrupting for, and their copy. */
const NOTIFIABLE = {
  "copilot-async-answer": {
    title: "Your analysis is ready",
    body: "FinSight finished the fuller answer.",
    tag: "copilot-async-answer",
  },
  "import-complete": {
    title: "Import finished",
    body: "Your transactions have been imported.",
    tag: "import-complete",
  },
} as const;

type NotifiableEvent = keyof typeof NOTIFIABLE;

/**
 * Whether a notification should fire right now. Pure and synchronous so the
 * decision is unit-testable without the Notification API or a real document.
 *
 * True only when notifications are permitted AND the page is hidden — the tab
 * is backgrounded, the window minimised, or another app is in front. A visible
 * page means the user can already see the result; the toast that renders
 * in-app is the right surface there, not an OS notification.
 */
export function shouldNotify(): boolean {
  if (notificationPermission() !== "granted") return false;
  // `document` is absent under SSR/tests that don't set up jsdom; treat that as
  // "not hidden" rather than throwing.
  if (typeof document === "undefined") return false;
  return document.visibilityState === "hidden";
}

/**
 * Fire one notification, guarded by [`shouldNotify`]. Clicking it brings the
 * window forward and dismisses the notification. Any failure (a webview that
 * declares the API but rejects construction) is swallowed — a missed
 * notification must never break the event pipeline it rides on.
 */
export function notify(title: string, body: string, tag: string): void {
  if (!shouldNotify()) return;
  try {
    // `tag` collapses repeats: a second import-complete replaces the first
    // rather than stacking, so a burst does not spam the tray.
    const n = new Notification(title, { body, tag });
    n.onclick = () => {
      try {
        window.focus();
      } catch {
        /* focus can throw in some webviews; the notification still dismisses */
      }
      n.close();
    };
  } catch {
    /* notifications are a nicety, never a hard dependency */
  }
}

/**
 * Subscribe to the notifiable server events. Returns a cleanup that removes
 * every listener; call it on unmount so a re-mount does not double-subscribe.
 */
export function startNativeNotifications(): () => void {
  const unlisteners: Array<() => void> = [];
  let disposed = false;

  for (const event of Object.keys(NOTIFIABLE) as NotifiableEvent[]) {
    const copy = NOTIFIABLE[event];
    void listen(event, () => notify(copy.title, copy.body, copy.tag)).then((un) => {
      // If cleanup already ran before this listener resolved, drop it
      // immediately rather than leaking it.
      if (disposed) un();
      else unlisteners.push(un);
    });
  }

  return () => {
    disposed = true;
    for (const un of unlisteners) un();
    unlisteners.length = 0;
  };
}

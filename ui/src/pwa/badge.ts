/**
 * Badging API wrapper — puts a "needs attention" count on the installed PWA's
 * home-screen / taskbar icon so the app itself becomes a passive at-a-glance
 * indicator, the way a native finance app's icon is.
 *
 * Reality of the platform, so callers don't expect more than they get:
 *
 * - The badge is only VISIBLE for an **installed** PWA. In a normal browser tab
 *   the calls succeed and paint nothing. There is no way to detect installed-ness
 *   reliably, so we always call and let the OS decide.
 * - Support is narrower than "Chromium" (verified against MDN browser-compat-data,
 *   2026-07):
 *     Chrome/Edge 81+   Windows and macOS only. ChromeOS since 91.
 *                       LINUX HAS NO OS-LEVEL BADGING — the promise resolves and
 *                       nothing is ever painted.
 *     Chrome Android 81+
 *     Safari 17+        installed web apps, macOS Sonoma and later.
 *     Safari iOS 16.4+  web apps saved to the home screen.
 *     Firefox           not implemented, any platform.
 *     Opera             not implemented, despite being Chromium.
 *   So "it silently does nothing" is the NORMAL case on a Linux desktop or in
 *   Opera, not a bug to chase.
 * - Updates only happen while a page (or service worker) of ours is RUNNING.
 *   With the app fully closed, the last-set value simply persists — refreshing
 *   it in that state needs a push event to wake the service worker, which is a
 *   separate feature.
 *
 * Every call is best-effort. `setAppBadge` rejects in several benign situations
 * (no installed app, permission not granted, embedded contexts), and a badge
 * failing is never worth surfacing to the user or breaking a render, so all
 * rejections are swallowed.
 */

type BadgeNavigator = Navigator & {
  setAppBadge?: (contents?: number) => Promise<void>;
  clearAppBadge?: () => Promise<void>;
};

function nav(): BadgeNavigator | null {
  return typeof navigator === "undefined" ? null : (navigator as BadgeNavigator);
}

/** True when this browser implements the Badging API at all. */
export function badgingSupported(): boolean {
  const n = nav();
  return Boolean(n && typeof n.setAppBadge === "function");
}

/**
 * Set the icon badge to `count`, or clear it when `count <= 0`.
 *
 * `setAppBadge(0)` is specified to clear the badge, but we route zero through
 * `clearAppBadge()` explicitly rather than trusting every implementation to
 * treat it identically — "no badge" and "a badge that says nothing" look very
 * different on a home screen.
 *
 * Non-integer or negative input is coerced rather than thrown on: this is fed
 * by a network query, and a malformed count should degrade to no badge.
 */
export async function syncAppBadge(count: number): Promise<void> {
  const n = nav();
  if (!n || typeof n.setAppBadge !== "function") return;

  const safe = Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
  try {
    if (safe === 0) {
      await n.clearAppBadge?.();
    } else {
      await n.setAppBadge(safe);
    }
  } catch {
    // Not installed, permission withheld, or an embedded context. Nothing the
    // user can act on and nothing worth logging on every poll.
  }
}

/**
 * Remove the badge outright. Called on logout/401 as well as on unmount: a
 * signed-out device must not keep advertising how many items the previous user
 * had waiting, which would leak activity on a shared computer.
 */
export async function clearAppBadge(): Promise<void> {
  const n = nav();
  if (!n || typeof n.clearAppBadge !== "function") return;
  try {
    await n.clearAppBadge();
  } catch {
    // Same rationale as syncAppBadge.
  }
}

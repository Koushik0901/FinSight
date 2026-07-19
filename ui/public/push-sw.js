/*
 * Web Push receiver, imported into the generated Workbox service worker via
 * `workbox.importScripts` (see ui/vite.config.ts).
 *
 * This is the only code in FinSight that runs with the app fully CLOSED. The
 * SSE stream (/api/events) reaches the app only while a tab is open and
 * connected; the moment the PWA is closed or the phone sleeps, that stream goes
 * nowhere. A push wakes this worker instead.
 *
 * Plain JS on purpose: `importScripts` loads it as-is, outside the Vite bundle.
 * Kept small and dependency-free; the payload contract it depends on is
 * produced by crates/finsight-api/src/commands/push.rs and pinned by
 * ui/src/pwa/push.test.ts.
 *
 * Payload shape (JSON):
 *   { title, body, url?, tag?, badgeCount? }
 */

/* eslint-disable no-undef */

function finsightPushPayload(event) {
  // A push with no payload is legal (some services strip it), and a malformed
  // one must not lose the notification entirely — `userVisibleOnly: true` means
  // the browser will show a generic "site updated in the background" notice if
  // we fail to show our own, which is worse than a plain fallback.
  const fallback = { title: "FinSight", body: "" };
  if (!event.data) return fallback;
  try {
    const parsed = event.data.json();
    if (parsed && typeof parsed === "object") return parsed;
    return fallback;
  } catch (_e) {
    try {
      return { title: "FinSight", body: event.data.text() };
    } catch (_e2) {
      return fallback;
    }
  }
}

self.addEventListener("push", function (event) {
  const data = finsightPushPayload(event);

  event.waitUntil(
    (async function () {
      await self.registration.showNotification(String(data.title || "FinSight"), {
        body: String(data.body || ""),
        icon: "/pwa-192x192.png",
        badge: "/pwa-64x64.png",
        // Same tag collapses repeats of one topic rather than stacking a pile
        // of near-identical notifications on the lock screen.
        tag: String(data.tag || "finsight"),
        data: { url: typeof data.url === "string" ? data.url : "/" },
      });

      // The piece the foreground badge hook cannot do: refresh the icon count
      // while the app is closed. Sent alongside the notification so the badge
      // and the notification can never disagree.
      if (typeof data.badgeCount === "number" && self.navigator) {
        try {
          if (data.badgeCount > 0 && self.navigator.setAppBadge) {
            await self.navigator.setAppBadge(data.badgeCount);
          } else if (self.navigator.clearAppBadge) {
            await self.navigator.clearAppBadge();
          }
        } catch (_e) {
          // Badging unsupported here; the notification still landed.
        }
      }
    })()
  );
});

self.addEventListener("notificationclick", function (event) {
  event.notification.close();
  const target =
    (event.notification.data && event.notification.data.url) || "/";

  event.waitUntil(
    (async function () {
      // Prefer an existing window: opening a second one would leave the user
      // with two copies of the app and lose whatever they had on screen.
      const windows = await self.clients.matchAll({
        type: "window",
        includeUncontrolled: true,
      });
      for (const client of windows) {
        if ("focus" in client) {
          await client.focus();
          if ("navigate" in client) {
            try {
              await client.navigate(target);
            } catch (_e) {
              // Cross-origin or otherwise refused — focus alone is fine.
            }
          }
          return;
        }
      }
      if (self.clients.openWindow) await self.clients.openWindow(target);
    })()
  );
});

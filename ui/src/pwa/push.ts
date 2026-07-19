/**
 * Client half of Web Push.
 *
 * The service worker (ui/public/push-sw.js) receives pushes; this module is
 * what gets the browser to hand us a subscription in the first place and keeps
 * the server's copy of it current.
 *
 * Platform notes that shape the API below:
 * - Push requires an ACTIVE service worker, so every call awaits
 *   `serviceWorker.ready` rather than assuming registration finished.
 * - `Notification.requestPermission()` must be triggered by a user gesture on
 *   most browsers, which is why permission is requested in `enablePush` (called
 *   from a click) and never on app boot.
 * - iOS/Safari supports this only for a Home-Screen-installed app.
 * - Subscriptions expire and rotate on the browser's own schedule, so the
 *   server upserts by endpoint and the client re-syncs on load.
 */

/** Shape the server needs to reach a device; mirrors `push_subscriptions`. */
export type PushSubscriptionPayload = {
  endpoint: string;
  p256dh: string;
  auth: string;
};

/** True when this browser can subscribe at all (not whether it has). */
export function pushSupported(): boolean {
  return (
    typeof window !== "undefined" &&
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}

/** Current notification permission, or "unsupported" where there is no API. */
export function notificationPermission(): NotificationPermission | "unsupported" {
  if (typeof Notification === "undefined") return "unsupported";
  return Notification.permission;
}

/**
 * Decode the VAPID public key the server sends (base64url) into the raw bytes
 * `pushManager.subscribe` wants. Browsers reject a string here, and they reject
 * standard base64 — the `-`/`_` → `+`/`/` swap and the padding are both
 * required.
 */
export function urlBase64ToUint8Array(base64UrlKey: string): Uint8Array<ArrayBuffer> {
  const padding = "=".repeat((4 - (base64UrlKey.length % 4)) % 4);
  const base64 = (base64UrlKey + padding).replace(/-/g, "+").replace(/_/g, "/");
  const raw = atob(base64);
  // Backed by an explicit ArrayBuffer, not the default ArrayBufferLike: a
  // `Uint8Array<ArrayBufferLike>` could be over a SharedArrayBuffer, which is
  // not a valid `BufferSource` for `applicationServerKey`.
  const out = new Uint8Array(new ArrayBuffer(raw.length));
  for (let i = 0; i < raw.length; i += 1) out[i] = raw.charCodeAt(i);
  return out;
}

/** Base64url-encode a subscription key buffer for transport to the server. */
function encodeKey(buffer: ArrayBuffer | null): string {
  if (!buffer) return "";
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Flatten a browser PushSubscription into the server's wire shape. */
export function toPayload(sub: PushSubscription): PushSubscriptionPayload {
  return {
    endpoint: sub.endpoint,
    p256dh: encodeKey(sub.getKey("p256dh")),
    auth: encodeKey(sub.getKey("auth")),
  };
}

/** The existing subscription for this device, if it already opted in. */
export async function currentSubscription(): Promise<PushSubscription | null> {
  if (!pushSupported()) return null;
  try {
    const reg = await navigator.serviceWorker.ready;
    return await reg.pushManager.getSubscription();
  } catch {
    return null;
  }
}

export type EnableResult =
  | { ok: true; payload: PushSubscriptionPayload }
  | { ok: false; reason: "unsupported" | "denied" | "failed" };

/**
 * Ask for permission and subscribe. Call from a click handler.
 *
 * `userVisibleOnly: true` is mandatory, not a choice — Chromium refuses a
 * subscription without it, and it is a promise that every push shows a
 * notification (which is why push-sw.js always calls `showNotification`).
 */
export async function enablePush(vapidPublicKey: string): Promise<EnableResult> {
  if (!pushSupported() || !vapidPublicKey) return { ok: false, reason: "unsupported" };

  try {
    const permission = await Notification.requestPermission();
    if (permission !== "granted") return { ok: false, reason: "denied" };

    const reg = await navigator.serviceWorker.ready;
    // Reuse an existing subscription rather than re-subscribing: browsers
    // reject `subscribe` with a different applicationServerKey while one is
    // active, and a needless churn invalidates the server's stored keys.
    const existing = await reg.pushManager.getSubscription();
    const sub =
      existing ??
      (await reg.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: urlBase64ToUint8Array(vapidPublicKey),
      }));

    return { ok: true, payload: toPayload(sub) };
  } catch {
    return { ok: false, reason: "failed" };
  }
}

/**
 * Unsubscribe this device. Returns the endpoint that was cancelled so the
 * caller can tell the server to drop its row — read BEFORE unsubscribing,
 * because the object's endpoint is not reliably readable afterwards.
 */
export async function disablePush(): Promise<string | null> {
  if (!pushSupported()) return null;
  try {
    const reg = await navigator.serviceWorker.ready;
    const sub = await reg.pushManager.getSubscription();
    if (!sub) return null;
    const { endpoint } = sub;
    await sub.unsubscribe();
    return endpoint;
  } catch {
    return null;
  }
}

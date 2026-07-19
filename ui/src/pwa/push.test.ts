import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import swSource from "../../public/push-sw.js?raw";
import {
  currentSubscription,
  disablePush,
  enablePush,
  notificationPermission,
  pushSupported,
  toPayload,
  urlBase64ToUint8Array,
} from "./push";

/** Build a stand-in for the browser's PushSubscription. */
function fakeSubscription(endpoint = "https://push.example/abc") {
  const keys: Record<string, ArrayBuffer> = {
    // "hello" and "hi" as raw key bytes — enough to assert the base64url encoding.
    p256dh: new TextEncoder().encode("hello").buffer as ArrayBuffer,
    auth: new TextEncoder().encode("hi").buffer as ArrayBuffer,
  };
  return {
    endpoint,
    getKey: (name: string) => keys[name] ?? null,
    unsubscribe: vi.fn().mockResolvedValue(true),
  } as unknown as PushSubscription & { unsubscribe: ReturnType<typeof vi.fn> };
}

const subscribe = vi.fn();
const getSubscription = vi.fn();
const requestPermission = vi.fn();

function installPushEnvironment({ supported = true } = {}) {
  const nav = navigator as unknown as Record<string, unknown>;
  if (!supported) {
    delete nav.serviceWorker;
    delete (window as unknown as Record<string, unknown>).PushManager;
    return;
  }
  nav.serviceWorker = {
    ready: Promise.resolve({ pushManager: { subscribe, getSubscription } }),
  };
  (window as unknown as Record<string, unknown>).PushManager = function () {};
  (window as unknown as Record<string, unknown>).Notification = {
    permission: "default",
    requestPermission,
  };
  vi.stubGlobal("Notification", { permission: "default", requestPermission });
}

beforeEach(() => {
  subscribe.mockReset();
  getSubscription.mockReset().mockResolvedValue(null);
  requestPermission.mockReset().mockResolvedValue("granted");
  installPushEnvironment();
});

afterEach(() => vi.unstubAllGlobals());

describe("pushSupported", () => {
  it("is true when the browser has service workers, PushManager and Notification", () => {
    expect(pushSupported()).toBe(true);
  });

  it("is false where push is unavailable", () => {
    installPushEnvironment({ supported: false });
    expect(pushSupported()).toBe(false);
  });
});

describe("notificationPermission", () => {
  it("reports the browser's current permission", () => {
    expect(notificationPermission()).toBe("default");
  });

  it("reports 'unsupported' where Notification does not exist", () => {
    vi.stubGlobal("Notification", undefined);
    expect(notificationPermission()).toBe("unsupported");
  });
});

describe("urlBase64ToUint8Array", () => {
  // Browsers reject both a plain string and standard base64 here, so the
  // alphabet swap and padding are load-bearing, not cosmetic.
  it("decodes base64url, including the -/_ alphabet", () => {
    // "--__" is url-safe for "++//": sextets 62,62,63,63
    // -> 111110 111110 111111 111111 -> 0xFB 0xEF 0xFF.
    // Standard atob() would reject '-' and '_' outright, so this is the swap
    // doing real work.
    expect(Array.from(urlBase64ToUint8Array("--__"))).toEqual([251, 239, 255]);
  });

  it("restores missing padding", () => {
    // "aGk" is "hi" with the trailing '=' stripped, as VAPID keys are served.
    expect(Array.from(urlBase64ToUint8Array("aGk"))).toEqual([104, 105]);
  });

  it("round-trips a realistic 65-byte VAPID key length", () => {
    const raw = new Uint8Array(65).map((_, i) => i);
    let binary = "";
    for (const b of raw) binary += String.fromCharCode(b);
    const b64url = btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    expect(Array.from(urlBase64ToUint8Array(b64url))).toEqual(Array.from(raw));
  });
});

describe("toPayload", () => {
  it("base64url-encodes both keys for transport", () => {
    const payload = toPayload(fakeSubscription());
    expect(payload.endpoint).toBe("https://push.example/abc");
    // btoa("hello") === "aGVsbG8=" -> unpadded url-safe "aGVsbG8"
    expect(payload.p256dh).toBe("aGVsbG8");
    expect(payload.auth).toBe("aGk");
  });

  it("never emits padding or the +/ alphabet the server would reject", () => {
    const payload = toPayload(fakeSubscription());
    expect(payload.p256dh).not.toMatch(/[+/=]/);
    expect(payload.auth).not.toMatch(/[+/=]/);
  });
});

describe("enablePush", () => {
  it("subscribes and returns the payload when permission is granted", async () => {
    subscribe.mockResolvedValue(fakeSubscription());
    const result = await enablePush("aGk");
    expect(result).toEqual({
      ok: true,
      payload: { endpoint: "https://push.example/abc", p256dh: "aGVsbG8", auth: "aGk" },
    });
    expect(subscribe).toHaveBeenCalledWith(
      expect.objectContaining({ userVisibleOnly: true })
    );
  });

  it("reports denial rather than throwing when the user says no", async () => {
    requestPermission.mockResolvedValue("denied");
    expect(await enablePush("aGk")).toEqual({ ok: false, reason: "denied" });
    expect(subscribe).not.toHaveBeenCalled();
  });

  // Re-subscribing while a subscription is live throws in Chromium and would
  // silently invalidate the keys the server already stored.
  it("reuses an existing subscription instead of subscribing again", async () => {
    getSubscription.mockResolvedValue(fakeSubscription("https://push.example/existing"));
    const result = await enablePush("aGk");
    expect(result.ok).toBe(true);
    expect(subscribe).not.toHaveBeenCalled();
    expect(result.ok && result.payload.endpoint).toBe("https://push.example/existing");
  });

  it("reports unsupported when there is no VAPID key configured", async () => {
    expect(await enablePush("")).toEqual({ ok: false, reason: "unsupported" });
  });

  it("reports failure instead of throwing when subscribe rejects", async () => {
    subscribe.mockRejectedValue(new Error("push service unreachable"));
    expect(await enablePush("aGk")).toEqual({ ok: false, reason: "failed" });
  });
});

describe("disablePush", () => {
  it("unsubscribes and returns the endpoint so the server can drop its row", async () => {
    const sub = fakeSubscription();
    getSubscription.mockResolvedValue(sub);
    expect(await disablePush()).toBe("https://push.example/abc");
    expect(sub.unsubscribe).toHaveBeenCalled();
  });

  it("returns null when this device was never subscribed", async () => {
    expect(await disablePush()).toBeNull();
  });
});

describe("currentSubscription", () => {
  it("returns the live subscription", async () => {
    getSubscription.mockResolvedValue(fakeSubscription());
    expect((await currentSubscription())?.endpoint).toBe("https://push.example/abc");
  });

  it("returns null when unsupported", async () => {
    installPushEnvironment({ supported: false });
    expect(await currentSubscription()).toBeNull();
  });
});

// The push worker is plain JS outside the bundle; its payload contract is
// produced by the Rust sender and can only drift silently. Pin the shape.
describe("push service worker contract", () => {
  it("handles both push and notificationclick", () => {
    expect(swSource).toContain('addEventListener("push"');
    expect(swSource).toContain('addEventListener("notificationclick"');
  });

  it("always shows a notification, as userVisibleOnly requires", () => {
    expect(swSource).toContain("showNotification");
  });

  it("reads every field the server sends", () => {
    expect(swSource).toContain("data.title");
    expect(swSource).toContain("data.body");
    expect(swSource).toContain("data.url");
    expect(swSource).toContain("data.tag");
    expect(swSource).toContain("data.badgeCount");
  });

  it("updates the app-icon badge, which the foreground hook cannot do when closed", () => {
    expect(swSource).toContain("setAppBadge");
    expect(swSource).toContain("clearAppBadge");
  });

  it("focuses an existing window rather than opening a duplicate", () => {
    expect(swSource).toContain("matchAll");
    expect(swSource).toContain("openWindow");
  });
});

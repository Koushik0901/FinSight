import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

/**
 * Native notifications should reach the user exactly when they can't already
 * see the result — permission granted, and the tab in the background — and
 * never otherwise. These tests pin that decision and the guards around firing.
 */

// The listen mock records handlers so a test can drive a server event, and
// returns an unlisten spy so cleanup can be asserted.
const listenHandlers = new Map<string, (e: unknown) => void>();
const unlistenSpies: Array<() => void> = [];
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: unknown) => void) => {
    listenHandlers.set(event, handler);
    const un = vi.fn();
    unlistenSpies.push(un);
    return Promise.resolve(un);
  }),
}));

let permission: NotificationPermission | "unsupported" = "granted";
vi.mock("./push", () => ({
  notificationPermission: () => permission,
}));

// A minimal Notification double: records construction and click wiring.
const notificationCtor = vi.fn();
class FakeNotification {
  onclick: (() => void) | null = null;
  close = vi.fn();
  constructor(public title: string, public options: NotificationOptions) {
    notificationCtor(title, options);
  }
}

function setVisibility(state: DocumentVisibilityState) {
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => state,
  });
}

beforeEach(() => {
  listenHandlers.clear();
  unlistenSpies.length = 0;
  notificationCtor.mockClear();
  permission = "granted";
  setVisibility("hidden");
  vi.stubGlobal("Notification", FakeNotification);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("shouldNotify", () => {
  it("fires only when permission is granted and the tab is hidden", async () => {
    const { shouldNotify } = await import("./nativeNotify");
    expect(shouldNotify()).toBe(true);
  });

  it("stays silent while the tab is visible — the in-app toast covers that", async () => {
    setVisibility("visible");
    const { shouldNotify } = await import("./nativeNotify");
    expect(shouldNotify()).toBe(false);
  });

  it("stays silent when permission has not been granted", async () => {
    permission = "default";
    const { shouldNotify } = await import("./nativeNotify");
    expect(shouldNotify()).toBe(false);
  });

  it("stays silent when notifications are denied", async () => {
    permission = "denied";
    const { shouldNotify } = await import("./nativeNotify");
    expect(shouldNotify()).toBe(false);
  });
});

describe("notify", () => {
  it("constructs a notification with a tag so repeats collapse", async () => {
    const { notify } = await import("./nativeNotify");
    notify("Title", "Body", "some-tag");
    expect(notificationCtor).toHaveBeenCalledWith("Title", { body: "Body", tag: "some-tag" });
  });

  it("does nothing when it should not notify", async () => {
    setVisibility("visible");
    const { notify } = await import("./nativeNotify");
    notify("Title", "Body", "t");
    expect(notificationCtor).not.toHaveBeenCalled();
  });

  it("focuses the window and dismisses when clicked", async () => {
    const created: FakeNotification[] = [];
    class CapturingNotification extends FakeNotification {
      constructor(title: string, options: NotificationOptions) {
        super(title, options);
        created.push(this);
      }
    }
    vi.stubGlobal("Notification", CapturingNotification);
    const focus = vi.fn();
    Object.defineProperty(window, "focus", { configurable: true, value: focus });

    const { notify } = await import("./nativeNotify");
    notify("Title", "Body", "t");

    const n = created[0]!;
    n.onclick?.();
    expect(focus).toHaveBeenCalled();
    expect(n.close).toHaveBeenCalled();
  });

  it("swallows a Notification constructor that throws", async () => {
    vi.stubGlobal(
      "Notification",
      class {
        constructor() {
          throw new Error("webview rejects construction");
        }
      },
    );
    // Re-grant permission via the stub's static, if read; our push mock covers it.
    const { notify } = await import("./nativeNotify");
    expect(() => notify("Title", "Body", "t")).not.toThrow();
  });
});

describe("startNativeNotifications", () => {
  it("notifies when a Copilot async answer arrives in the background", async () => {
    const { startNativeNotifications } = await import("./nativeNotify");
    startNativeNotifications();
    // Let the listen() promises resolve.
    await Promise.resolve();

    listenHandlers.get("copilot-async-answer")?.({ payload: {} });
    expect(notificationCtor).toHaveBeenCalledWith(
      "Your analysis is ready",
      expect.objectContaining({ tag: "copilot-async-answer" }),
    );
  });

  it("notifies when an import finishes in the background", async () => {
    const { startNativeNotifications } = await import("./nativeNotify");
    startNativeNotifications();
    await Promise.resolve();

    listenHandlers.get("import-complete")?.({ payload: {} });
    expect(notificationCtor).toHaveBeenCalledWith(
      "Import finished",
      expect.objectContaining({ tag: "import-complete" }),
    );
  });

  it("does not notify for a background event while the tab is visible", async () => {
    setVisibility("visible");
    const { startNativeNotifications } = await import("./nativeNotify");
    startNativeNotifications();
    await Promise.resolve();

    listenHandlers.get("copilot-async-answer")?.({ payload: {} });
    expect(notificationCtor).not.toHaveBeenCalled();
  });

  it("removes every listener on cleanup", async () => {
    const { startNativeNotifications } = await import("./nativeNotify");
    const stop = startNativeNotifications();
    await Promise.resolve();
    expect(unlistenSpies.length).toBe(2);

    stop();
    for (const un of unlistenSpies) expect(un).toHaveBeenCalled();
  });

  it("drops a listener that resolves after cleanup rather than leaking it", async () => {
    // Cleanup can run before the async listen() resolves; the late unlisten
    // must still fire so nothing is left subscribed.
    const { startNativeNotifications } = await import("./nativeNotify");
    const stop = startNativeNotifications();
    stop(); // before the awaited listen() promises resolve
    await Promise.resolve();
    await Promise.resolve();
    for (const un of unlistenSpies) expect(un).toHaveBeenCalled();
  });
});

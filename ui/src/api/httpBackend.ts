/**
 * PRODUCTION browser transport: installs an HTTP-backed `__TAURI_INTERNALS__`
 * so the generated bindings.ts works unchanged against finsight-server.
 * - invoke(cmd, args)        → POST /api/rpc/{cmd}
 * - plugin:event|listen      → registry + one shared EventSource(/api/events)
 * Mirrors ui/src/dev/mockBackend.ts (the proven shape for this trick), but
 * unlike the mock this shim runs in BOTH dev and production builds — it's
 * the real transport whenever the app isn't hosted inside Tauri.
 *
 * Verified against the installed @tauri-apps/api ^2.11.0 source
 * (node_modules/@tauri-apps/api/event.js):
 * - `listen()` invokes `plugin:event|listen` with `{ event, target, handler }`
 *   (we only need `event` and `handler`; `target` is ignored here — FinSight
 *   has a single window/webview, so per-target routing doesn't apply).
 * - `_unlisten()` invokes `plugin:event|unlisten` with `{ event, eventId }`.
 * - The eventId resolved from `plugin:event|listen` is later passed back as
 *   `eventId` to unlisten, so returning `handler` as that id (as below) is
 *   consistent — unlisten will delete the same id we registered.
 * - The real Tauri backend calls registered callbacks with `{event, id, payload}`
 *   (an `Event<T>`), which this shim mirrors when dispatching SSE frames.
 */
type AnyRec = Record<string, unknown>;

export function installHttpBackend(): void {
  const w = window as unknown as AnyRec;
  if (w.__TAURI_INTERNALS__) return; // never shadow a real Tauri runtime

  let cbSeq = 0;
  // event name → callback ids; SSE frames fan out to window[`_${id}`]
  const listeners = new Map<string, Set<number>>();
  let es: EventSource | null = null;

  function ensureEventSource() {
    if (es) return;
    es = new EventSource("/api/events");
    es.onmessage = (msg) => {
      const { event, payload } = JSON.parse(msg.data) as { event: string; payload: unknown };
      for (const id of listeners.get(event) ?? []) {
        const cb = w[`_${id}`] as ((e: unknown) => void) | undefined;
        // Shape mirrors @tauri-apps/api v2 event delivery: {event, id, payload}
        cb?.({ event, id, payload });
      }
    };
  }

  const invoke = async (cmd: string, args?: AnyRec): Promise<unknown> => {
    if (cmd.startsWith("plugin:")) {
      if (cmd === "plugin:event|listen") {
        const { event, handler } = (args ?? {}) as { event: string; handler: number };
        if (!listeners.has(event)) listeners.set(event, new Set());
        listeners.get(event)!.add(handler);
        ensureEventSource();
        return handler; // unlisten id
      }
      if (cmd === "plugin:event|unlisten") {
        const { event, eventId } = (args ?? {}) as { event: string; eventId: number };
        listeners.get(event)?.delete(eventId);
        return null;
      }
      return null; // other plugin traffic (dialog, notification) resolves harmlessly
    }
    const res = await fetch(`/api/rpc/${cmd}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(args ?? {}),
    });
    // A non-JSON body (reverse-proxy 502 HTML, crash page) must not surface as
    // a SyntaxError from res.json() — bindings.ts rethrows Error instances
    // instead of returning {status:"error"}, which would crash the caller.
    let body: unknown = null;
    if (res.status !== 204) {
      try {
        body = await res.json();
      } catch {
        body = { code: "rpc.transport", message: `HTTP ${res.status} with non-JSON body` };
      }
    }
    // Throw the plain AppError object so bindings.ts's catch returns
    // {status:"error", error} exactly as it does under real Tauri.
    if (!res.ok) throw body;
    return body;
  };

  w.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: unknown) => {
      const id = ++cbSeq;
      w[`_${id}`] = cb;
      return id;
    },
    unregisterCallback: () => {},
    unregisterListener: () => {},
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
}

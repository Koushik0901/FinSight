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
import { isTauriRuntime } from "../utils/runtime";

type AnyRec = Record<string, unknown>;

/// Plugin namespaces that are genuinely fire-and-forget on the web: there is
/// nothing to do and nothing to report, so resolving null is honest. Anything
/// NOT listed here throws (see the invoke handler) so an un-ported native call
/// surfaces instead of silently pretending to succeed.
const NOOP_PLUGINS = ["plugin:notification"];

export function installHttpBackend(): void {
  const w = window as unknown as AnyRec;
  // never shadow a real Tauri runtime (origin-aware, Phase 4) — a Tauri
  // webview navigated to a remote server no longer passes isTauriRuntime()
  // even though window.__TAURI_INTERNALS__ is still present, so the shim
  // correctly installs there instead of bailing out on stale bridge presence.
  if (isTauriRuntime()) return;
  // NOTE: `__FINSIGHT_HTTP__` is set at the END of this function, only once the
  // shim is actually installed — setting it up here left a half-applied state
  // on the read-only-bridge failure path below (the app claimed server mode
  // while the native bridge was still the transport).

  let cbSeq = 0;
  // event name → callback ids; SSE frames fan out to window[`_${id}`]
  const listeners = new Map<string, Set<number>>();
  let es: EventSource | null = null;

  function ensureEventSource() {
    // A CLOSED EventSource must be replaced, not reused: per spec the browser
    // does NOT auto-reconnect when the response is non-200 (e.g. /api/events
    // 401s after a server restart drops the in-memory session), it fires
    // `error` and closes for good. Guarding on `es` alone left a dead object in
    // place, so every later listen() silently no-op'd and Copilot streaming /
    // import progress stayed broken until a full page reload.
    if (es && es.readyState !== EventSource.CLOSED) return;
    es = new EventSource("/api/events");
    es.onerror = () => {
      if (es && es.readyState === EventSource.CLOSED) es = null;
    };
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
        // Drop the callback too — the real bridge's unregisterCallback does
        // this. Without it every listen→unlisten cycle leaked the closure (and
        // its whole React/AG-UI scope) on `window` for the page's lifetime;
        // TauriAgUiAgent re-listens per Copilot run, so it accumulated per
        // message sent.
        delete w[`_${eventId}`];
        return null;
      }
      // Fire-and-forget plugins we deliberately no-op in server mode.
      if (NOOP_PLUGINS.some((p) => cmd.startsWith(p))) return null;
      // Everything else (notably `plugin:dialog|open`, the native file picker)
      // has NO server-mode implementation. Resolving null here made those calls
      // look like "user cancelled", which turned CSV import into a silent dead
      // button. Fail loudly so an un-ported native path is visible instead.
      throw {
        code: "rpc.unsupported_plugin",
        message: `${cmd} has no server-mode implementation`,
      };
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
    if (!res.ok) {
      // The session cookie is missing/expired: notify the app (AuthGate
      // listens for this to route back to the login screen from anywhere)
      // and close the shared EventSource so the browser stops silently
      // auto-reconnecting it against a now-401'ing endpoint. `listeners`
      // stays intact — the next `plugin:event|listen` call (naturally fired
      // when the app remounts after re-login) calls ensureEventSource()
      // again and reopens it.
      if (res.status === 401 && typeof body === "object" && body !== null && (body as AnyRec).code === "auth.required") {
        window.dispatchEvent(new CustomEvent("finsight:auth-required"));
        es?.close();
        es = null;
      }
      throw body;
    }
    return body;
  };

  const shim = {
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
  // In a plain browser / PWA / server the property is absent or writable, so
  // this simple assignment installs the shim. Guarded because a REAL Tauri
  // webview defines window.__TAURI_INTERNALS__ as a read-only, non-configurable
  // property (and its `invoke` is a locked own property) — assigning to it
  // THROWS in strict mode and would white-screen boot(). This shouldn't be
  // reached now that isTauriRuntime() is true on Tauri's own origins (so the
  // shim isn't installed there) and remote server origins get no injected
  // bridge; the try/catch is defense-in-depth so a stray locked bridge degrades
  // (native IPC stays in place) instead of crashing the whole app.
  try {
    w.__TAURI_INTERNALS__ = shim;
  } catch {
    // eslint-disable-next-line no-console
    console.warn(
      "installHttpBackend: __TAURI_INTERNALS__ is read-only (real Tauri webview); " +
        "leaving the native bridge in place. RPC over HTTP is unavailable at this origin.",
    );
    return;
  }
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  // Set LAST: this flag means "the HTTP shim is installed and is the
  // transport". isServerMode()/isBackendAvailable() and isTauriRuntime() all
  // key off it, so setting it before the assignment above could succeed left
  // the app claiming server mode over a native bridge on the failure path.
  w.__FINSIGHT_HTTP__ = true;
}

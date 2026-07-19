import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installHttpBackend } from "./httpBackend";

type AnyRec = Record<string, unknown>;
const w = window as unknown as AnyRec;

// installHttpBackend() now gates on isTauriRuntime() (Phase 4 Task 6) rather
// than raw window.__TAURI_INTERNALS__ presence. isTauriRuntime() itself
// short-circuits to `true` under vitest (untouched by Task 6, see
// utils/runtime.test.ts), which would make every test below wrongly bail out
// before doing anything. This suite is about the browser/server-mode
// transport, not about isTauriRuntime()'s own bridge/origin logic (that's
// covered separately in utils/runtime.test.ts) — so it mocks the module to
// behave like a real non-Tauri browser (false) by default, matching exactly
// what the raw `w.__TAURI_INTERNALS__` check used to do here before Task 6.
vi.mock("../utils/runtime", () => ({ isTauriRuntime: vi.fn(() => false) }));

describe("httpBackend shim", () => {
  beforeEach(() => {
    delete w.__TAURI_INTERNALS__;
    vi.stubGlobal("EventSource", class {
      static last: unknown;
      onmessage: ((e: { data: string }) => void) | null = null;
      constructor(public url: string) { (this.constructor as unknown as AnyRec).last = this; }
      close() {}
    });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    delete w.__FINSIGHT_HTTP__;
  });

  it("sets window.__FINSIGHT_HTTP__ = true on install", () => {
    vi.stubGlobal("fetch", vi.fn());
    expect(w.__FINSIGHT_HTTP__).toBeUndefined();
    installHttpBackend();
    expect(w.__FINSIGHT_HTTP__).toBe(true);
  });

  it("routes invoke to POST /api/rpc/{cmd} and returns parsed JSON", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify([{ id: "a1" }]), { status: 200 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    const out = await internals.invoke("list_accounts", {});
    expect(fetch).toHaveBeenCalledWith("/api/rpc/list_accounts", expect.objectContaining({
      method: "POST",
      headers: expect.objectContaining({ "content-type": "application/json" }),
      body: "{}",
    }));
    expect(out).toEqual([{ id: "a1" }]);
  });

  it("throws the parsed AppError object (not an Error) on non-2xx", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ code: "core.db", message: "boom" }), { status: 500 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    // bindings.ts does `if (e instanceof Error) throw e; else return {status:"error", error:e}`
    // so the thrown value MUST be the plain AppError object.
    await expect(internals.invoke("list_accounts", {})).rejects.toEqual({ code: "core.db", message: "boom" });
  });

  it("throws a plain rpc.transport object (not a SyntaxError) on non-JSON error bodies", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response("<html>502 Bad Gateway</html>", { status: 502 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    // A reverse-proxy error page is not JSON; res.json() throwing a SyntaxError
    // (an Error instance) would crash through bindings.ts instead of returning
    // {status:"error"}. The shim must synthesize a plain AppError-shaped object.
    await expect(internals.invoke("list_accounts", {})).rejects.toEqual({
      code: "rpc.transport",
      message: "HTTP 502 with non-JSON body",
    });
  });

  it("dispatches SSE frames to listeners registered via plugin:event|listen", async () => {
    vi.stubGlobal("fetch", vi.fn());
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as {
      invoke: (c: string, a?: AnyRec) => Promise<unknown>;
      transformCallback: (cb: unknown) => number;
    };
    const received: unknown[] = [];
    const handler = internals.transformCallback((e: unknown) => received.push(e));
    await internals.invoke("plugin:event|listen", { event: "copilot-stream-frame", handler });
    const es = (globalThis.EventSource as unknown as AnyRec).last as {
      onmessage: (e: { data: string }) => void;
    };
    es.onmessage({ data: JSON.stringify({ event: "copilot-stream-frame", payload: { type: "text", delta: "hi" } }) });
    expect(received).toHaveLength(1);
    expect((received[0] as AnyRec).payload).toEqual({ type: "text", delta: "hi" });
  });

  it("dispatches finsight:auth-required and closes the shared EventSource on an RPC 401 auth.required", async () => {
    const closeSpy = vi.fn();
    vi.stubGlobal("EventSource", class {
      static last: unknown;
      onmessage: ((e: { data: string }) => void) | null = null;
      constructor(public url: string) { (this.constructor as unknown as AnyRec).last = this; }
      close() { closeSpy(); }
    });
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ code: "auth.required", message: "no session" }), { status: 401 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as {
      invoke: (c: string, a?: AnyRec) => Promise<unknown>;
      transformCallback: (cb: unknown) => number;
    };
    // Establish the shared EventSource first, via a normal listener registration.
    const handler = internals.transformCallback(() => {});
    await internals.invoke("plugin:event|listen", { event: "copilot-stream-frame", handler });
    expect(closeSpy).not.toHaveBeenCalled();

    const dispatchSpy = vi.spyOn(window, "dispatchEvent");
    await expect(internals.invoke("list_accounts", {})).rejects.toEqual({
      code: "auth.required",
      message: "no session",
    });

    expect(closeSpy).toHaveBeenCalledTimes(1);
    expect(dispatchSpy).toHaveBeenCalledWith(expect.objectContaining({ type: "finsight:auth-required" }));
  });

  it("reopens the EventSource on the next listen after a 401 closed it (listener map is preserved)", async () => {
    let constructed = 0;
    vi.stubGlobal("EventSource", class {
      static last: unknown;
      onmessage: ((e: { data: string }) => void) | null = null;
      constructor(public url: string) { constructed += 1; (this.constructor as unknown as AnyRec).last = this; }
      close() {}
    });
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ code: "auth.required", message: "no session" }), { status: 401 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as {
      invoke: (c: string, a?: AnyRec) => Promise<unknown>;
      transformCallback: (cb: unknown) => number;
    };
    const handler = internals.transformCallback(() => {});
    await internals.invoke("plugin:event|listen", { event: "copilot-stream-frame", handler });
    expect(constructed).toBe(1);

    await expect(internals.invoke("list_accounts", {})).rejects.toEqual({
      code: "auth.required",
      message: "no session",
    });

    // A fresh listen (e.g. after re-login remounts the app) opens a new ES.
    await internals.invoke("plugin:event|listen", { event: "copilot-stream-frame", handler });
    expect(constructed).toBe(2);
  });

  it("does not dispatch finsight:auth-required for non-auth 401/other errors", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ code: "core.not_found", message: "nope" }), { status: 404 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");
    await expect(internals.invoke("list_accounts", {})).rejects.toEqual({ code: "core.not_found", message: "nope" });
    expect(dispatchSpy).not.toHaveBeenCalledWith(expect.objectContaining({ type: "finsight:auth-required" }));
  });

  it("installs over a stale __TAURI_INTERNALS__ bridge when isTauriRuntime() says false " +
     "(the Phase 4 shell-navigated-to-a-remote-server scenario)", async () => {
    // Simulate a Tauri webview that has navigated to a remote self-hosted
    // server: the IPC bridge object is still present (Tauri injects it on
    // any origin) but isTauriRuntime() (origin-aware per Task 6, mocked here
    // to reflect that) correctly reports false. The old raw
    // `if (w.__TAURI_INTERNALS__) return;` guard would have wrongly bailed
    // out here and left the stale/inert bridge in place; the fixed guard
    // must install the real HTTP shim over it instead.
    const staleInvoke = vi.fn();
    w.__TAURI_INTERNALS__ = { invoke: staleInvoke };
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify([{ id: "a1" }]), { status: 200 })));

    installHttpBackend();

    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    expect(internals.invoke).not.toBe(staleInvoke);
    const out = await internals.invoke("list_accounts", {});
    expect(fetch).toHaveBeenCalledWith("/api/rpc/list_accounts", expect.anything());
    expect(staleInvoke).not.toHaveBeenCalled();
    expect(out).toEqual([{ id: "a1" }]);
  });
});

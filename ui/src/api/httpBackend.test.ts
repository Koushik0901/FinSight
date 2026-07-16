import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installHttpBackend } from "./httpBackend";

type AnyRec = Record<string, unknown>;
const w = window as unknown as AnyRec;

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
  afterEach(() => vi.unstubAllGlobals());

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
});

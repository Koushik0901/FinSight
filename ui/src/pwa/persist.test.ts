import { describe, it, expect, vi, beforeEach } from "vitest";

// See cacheCrypto.test.ts: vitest exposes real WebCrypto on the global even
// though jsdom has no SubtleCrypto of its own.
const realCrypto = globalThis.crypto;
/** A `crypto` with NO `subtle` — what an http:// LAN origin actually gets. */
const insecureCrypto = {
  getRandomValues: realCrypto.getRandomValues.bind(realCrypto),
} as unknown as Crypto;

const store: Record<string, unknown> = {};
vi.mock("idb-keyval", () => ({
  get: vi.fn(async (k: string) => store[k]),
  set: vi.fn(async (k: string, v: unknown) => { store[k] = v; }),
  del: vi.fn(async (k: string) => { delete store[k]; }),
  update: vi.fn(async (k: string, fn: (cur: unknown) => unknown) => { store[k] = fn(store[k]); }),
}));

import { createIdbPersister, purgePersistedCache, PERSIST_KEY } from "./persist";
import { CACHE_KEY_HANDLE, __resetKeyCacheForTests } from "./cacheCrypto";

const client = (timestamp: number) => ({
  buster: "",
  timestamp,
  clientState: { mutations: [], queries: [] },
});

beforeEach(() => {
  vi.stubGlobal("crypto", realCrypto);
  for (const k of Object.keys(store)) delete store[k];
  __resetKeyCacheForTests();
});

describe("idb persister", () => {
  it("persists and restores a client value", async () => {
    const p = createIdbPersister();
    await p.persistClient(client(1));
    const back = await p.restoreClient();
    expect(back?.timestamp).toBe(1);
  });

  it("purgePersistedCache removes the stored key", async () => {
    const p = createIdbPersister();
    await p.persistClient(client(2));
    await purgePersistedCache();
    expect(store[PERSIST_KEY]).toBeUndefined();
  });
});

describe("at-rest encryption", () => {
  // The regression test for the whole feature: what lands in IndexedDB must not
  // be readable.
  it("writes ciphertext, not the serialized cache", async () => {
    const p = createIdbPersister();
    await p.persistClient({
      buster: "",
      timestamp: 3,
      clientState: {
        mutations: [],
        queries: [
          {
            queryKey: ["accounts"],
            queryHash: '["accounts"]',
            state: { data: [{ name: "CHEQUING", balanceCents: 918273 }] },
          },
        ],
      } as never,
    });

    const raw = store[PERSIST_KEY];
    expect(typeof raw).not.toBe("string");
    const flat = new TextDecoder().decode(new Uint8Array((raw as { ct: ArrayBuffer }).ct));
    expect(flat).not.toContain("CHEQUING");
    expect(flat).not.toContain("918273");
  });

  it("round-trips an encrypted cache back through restore", async () => {
    const p = createIdbPersister();
    await p.persistClient(client(4));
    expect(typeof store[PERSIST_KEY]).not.toBe("string");
    expect((await p.restoreClient())?.timestamp).toBe(4);
  });

  it("purgePersistedCache destroys the key as well as the cache", async () => {
    const p = createIdbPersister();
    await p.persistClient(client(5));
    expect(store[CACHE_KEY_HANDLE]).toBeDefined();
    await purgePersistedCache();
    expect(store[CACHE_KEY_HANDLE]).toBeUndefined();
    expect(store[PERSIST_KEY]).toBeUndefined();
  });
});

describe("migration from a pre-encryption build", () => {
  it("discards a plaintext cache instead of reading it, and deletes it", async () => {
    store[PERSIST_KEY] = JSON.stringify(client(6));
    const p = createIdbPersister();
    expect(await p.restoreClient()).toBeUndefined();
    expect(store[PERSIST_KEY]).toBeUndefined();
  });
});

describe("insecure origin (no crypto.subtle)", () => {
  it("persists NOTHING rather than falling back to plaintext", async () => {
    vi.stubGlobal("crypto", insecureCrypto);
    __resetKeyCacheForTests();
    const p = createIdbPersister();
    await p.persistClient(client(7));
    expect(store[PERSIST_KEY]).toBeUndefined();
  });

  it("drops a previously written blob it can no longer refresh", async () => {
    const p = createIdbPersister();
    await p.persistClient(client(8));
    expect(store[PERSIST_KEY]).toBeDefined();

    vi.stubGlobal("crypto", insecureCrypto);
    __resetKeyCacheForTests();
    await p.persistClient(client(9));
    expect(store[PERSIST_KEY]).toBeUndefined();
  });
});

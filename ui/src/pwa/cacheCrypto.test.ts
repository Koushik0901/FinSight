import { describe, it, expect, vi, beforeEach } from "vitest";

// jsdom itself ships no SubtleCrypto, but vitest exposes the platform's real
// WebCrypto on the global. Capture it up front so the "insecure origin" tests
// can stub it away and every other test can be restored to the genuine
// implementation — these assertions exercise real AES-GCM, not a stub that
// could hide a bug.
const realCrypto = globalThis.crypto;
/** A `crypto` with NO `subtle` — what an http:// LAN origin actually gets. */
const insecureCrypto = {
  getRandomValues: realCrypto.getRandomValues.bind(realCrypto),
} as unknown as Crypto;

const store: Record<string, unknown> = {};
vi.mock("idb-keyval", () => ({
  get: vi.fn(async (k: string) => store[k]),
  set: vi.fn(async (k: string, v: unknown) => {
    store[k] = v;
  }),
  del: vi.fn(async (k: string) => {
    delete store[k];
  }),
  // Mirrors idb-keyval's single-transaction get-then-put.
  update: vi.fn(async (k: string, fn: (cur: unknown) => unknown) => {
    store[k] = fn(store[k]);
  }),
}));

import {
  CACHE_KEY_HANDLE,
  cacheEncryptionAvailable,
  decryptCachePayload,
  encryptCachePayload,
  isLegacyPlaintext,
  purgeCacheKey,
  __resetKeyCacheForTests,
} from "./cacheCrypto";

beforeEach(() => {
  vi.stubGlobal("crypto", realCrypto);
  for (const k of Object.keys(store)) delete store[k];
  __resetKeyCacheForTests();
});

describe("cacheEncryptionAvailable", () => {
  it("is true where crypto.subtle exists", () => {
    expect(cacheEncryptionAvailable()).toBe(true);
  });

  it("is false on an insecure origin, where crypto.subtle is undefined", () => {
    vi.stubGlobal("crypto", insecureCrypto);
    expect(cacheEncryptionAvailable()).toBe(false);
  });
});

describe("encrypt / decrypt round trip", () => {
  it("returns the original payload", async () => {
    const payload = JSON.stringify({ balance: 123456, merchant: "LOBLAWS" });
    const env = await encryptCachePayload(payload);
    expect(env).not.toBeNull();
    expect(await decryptCachePayload(env)).toBe(payload);
  });

  // The property the whole issue is about.
  it("stores no recognisable plaintext in the envelope", async () => {
    const env = await encryptCachePayload(JSON.stringify({ merchant: "LOBLAWS", cents: 4210 }));
    const bytes = new Uint8Array(env!.ct);
    const asText = new TextDecoder().decode(bytes);
    expect(asText).not.toContain("LOBLAWS");
    expect(asText).not.toContain("4210");
  });

  it("uses a fresh IV per write, so AES-GCM nonces are never reused", async () => {
    const a = await encryptCachePayload("same input");
    const b = await encryptCachePayload("same input");
    expect(Array.from(a!.iv)).not.toEqual(Array.from(b!.iv));
    // Identical plaintext must not produce identical ciphertext.
    expect(Array.from(new Uint8Array(a!.ct))).not.toEqual(Array.from(new Uint8Array(b!.ct)));
  });

  it("reuses one device key across writes rather than generating per call", async () => {
    await encryptCachePayload("first");
    const key = store[CACHE_KEY_HANDLE];
    await encryptCachePayload("second");
    expect(store[CACHE_KEY_HANDLE]).toBe(key);
  });

  it("generates a NON-extractable key, so script can never read its bytes", async () => {
    await encryptCachePayload("x");
    const key = store[CACHE_KEY_HANDLE] as CryptoKey;
    expect(key.extractable).toBe(false);
    await expect(realCrypto.subtle.exportKey("raw", key)).rejects.toThrow();
  });
});

describe("decryptCachePayload failure paths all read as a cache miss", () => {
  it("returns null once the key has been destroyed (crypto-shredding)", async () => {
    const env = await encryptCachePayload("secret");
    await purgeCacheKey();
    __resetKeyCacheForTests();
    // A new key is generated on next use; the old ciphertext must not decrypt.
    expect(await decryptCachePayload(env)).toBeNull();
  });

  it("returns null on tampered ciphertext instead of trusting it", async () => {
    const env = await encryptCachePayload("secret");
    const bytes = new Uint8Array(env!.ct);
    bytes[0] = (bytes[0] ?? 0) ^ 0xff;
    expect(await decryptCachePayload({ ...env!, ct: bytes.buffer })).toBeNull();
  });

  it("returns null for an envelope from a different version", async () => {
    const env = await encryptCachePayload("secret");
    expect(await decryptCachePayload({ ...env!, v: 99 })).toBeNull();
  });

  it("returns null for a raw string (a pre-encryption cache)", async () => {
    expect(await decryptCachePayload("plain json")).toBeNull();
  });

  it("returns null for junk", async () => {
    expect(await decryptCachePayload(null)).toBeNull();
    expect(await decryptCachePayload({ v: 1 })).toBeNull();
  });
});

describe("insecure origin", () => {
  it("refuses to encrypt rather than returning the plaintext", async () => {
    vi.stubGlobal("crypto", insecureCrypto);
    __resetKeyCacheForTests();
    expect(await encryptCachePayload("secret")).toBeNull();
  });
});

describe("isLegacyPlaintext", () => {
  it("flags a stored string and nothing else", async () => {
    expect(isLegacyPlaintext("{}")).toBe(true);
    expect(isLegacyPlaintext(await encryptCachePayload("x"))).toBe(false);
    expect(isLegacyPlaintext(undefined)).toBe(false);
  });
});

describe("transient storage failure", () => {
  // Memoising a failed key lookup would silently disable cache persistence for
  // the rest of the session, long after the cause cleared.
  it("retries after a transient IndexedDB failure instead of giving up", async () => {
    const idb = await import("idb-keyval");
    const get = vi.mocked(idb.get);

    get.mockRejectedValueOnce(new Error("IndexedDB blocked by another tab"));
    expect(await encryptCachePayload("first")).toBeNull();

    // Storage recovers; the very next write must succeed.
    const env = await encryptCachePayload("second");
    expect(env).not.toBeNull();
    expect(await decryptCachePayload(env)).toBe("second");
  });
});

describe("purgeCacheKey", () => {
  it("removes the key from storage", async () => {
    await encryptCachePayload("x");
    expect(store[CACHE_KEY_HANDLE]).toBeDefined();
    await purgeCacheKey();
    expect(store[CACHE_KEY_HANDLE]).toBeUndefined();
  });
});

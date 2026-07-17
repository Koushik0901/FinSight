import { describe, it, expect, vi, beforeEach } from "vitest";

const store: Record<string, unknown> = {};
vi.mock("idb-keyval", () => ({
  get: vi.fn(async (k: string) => store[k]),
  set: vi.fn(async (k: string, v: unknown) => { store[k] = v; }),
  del: vi.fn(async (k: string) => { delete store[k]; }),
}));

import { createIdbPersister, purgePersistedCache, PERSIST_KEY } from "./persist";

beforeEach(() => { for (const k of Object.keys(store)) delete store[k]; });

describe("idb persister", () => {
  it("persists and restores a client value", async () => {
    const p = createIdbPersister();
    await p.persistClient({ buster: "", timestamp: 1, clientState: { mutations: [], queries: [] } });
    const back = await p.restoreClient();
    expect(back?.timestamp).toBe(1);
  });
  it("purgePersistedCache removes the stored key", async () => {
    const p = createIdbPersister();
    await p.persistClient({ buster: "", timestamp: 2, clientState: { mutations: [], queries: [] } });
    await purgePersistedCache();
    expect(store[PERSIST_KEY]).toBeUndefined();
  });
});

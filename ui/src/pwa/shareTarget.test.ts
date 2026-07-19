import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { IDBFactory } from "fake-indexeddb";
// `?raw` gives us the service worker's source as a string. It doubles as an
// existence check: if public/share-target-sw.js is ever deleted or renamed this
// import fails the suite — which matters, because workbox's importScripts
// throws on a 404 and a throwing service worker never installs, taking offline
// support down with it.
import swSource from "../../public/share-target-sw.js?raw";
import viteConfigSource from "../../vite.config.ts?raw";
import {
  SHARE_DB_NAME,
  SHARE_STORE,
  SHARE_KEY,
  SHARE_MAX_AGE_MS,
  clearShareFlag,
  purgeSharedFiles,
  readShareFlag,
  sweepStaleSharedFiles,
  takeSharedFile,
  type SharedFileRecord,
} from "./shareTarget";

// jsdom has no IndexedDB; fake-indexeddb gives real transaction semantics, so
// the read-and-delete-in-one-transaction behaviour is genuinely exercised.
beforeEach(() => {
  globalThis.indexedDB = new IDBFactory();
});

/** Write a record the way the service worker would. */
async function stash(record: SharedFileRecord): Promise<void> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(SHARE_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, "readwrite");
    tx.objectStore(SHARE_STORE).put(record, SHARE_KEY);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
  db.close();
}

/** jsdom's Blob implements neither `.text()` nor `.arrayBuffer()`; FileReader
 *  it does. Real browsers are fine — production never reads the File directly,
 *  it hands it straight to FormData. */
function readText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}

function csvRecord(text = "Date,Merchant,Amount\n2026-01-02,LOBLAWS,-42.10\n"): SharedFileRecord {
  return {
    name: "statement.csv",
    type: "text/csv",
    buffer: new TextEncoder().encode(text).buffer as ArrayBuffer,
    // "just arrived" — the realistic case, since a hand-off takes seconds.
    receivedAt: Date.now(),
  };
}

/** Look at the parked record WITHOUT consuming it. */
async function peek(): Promise<SharedFileRecord | undefined> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(SHARE_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  const record = await new Promise<SharedFileRecord | undefined>((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, "readonly");
    const req = tx.objectStore(SHARE_STORE).get(SHARE_KEY);
    req.onsuccess = () => resolve(req.result as SharedFileRecord | undefined);
    tx.onerror = () => reject(tx.error);
  });
  db.close();
  return record;
}

describe("readShareFlag", () => {
  it("maps each outcome the service worker can redirect with", () => {
    expect(readShareFlag("?shared=1")).toBe("file");
    expect(readShareFlag("?shared=empty")).toBe("empty");
    expect(readShareFlag("?shared=error")).toBe("error");
  });

  it("is 'none' for a normal launch", () => {
    expect(readShareFlag("")).toBe("none");
    expect(readShareFlag("?tab=budget")).toBe("none");
    expect(readShareFlag("?shared=bogus")).toBe("none");
  });
});

describe("clearShareFlag", () => {
  const original = window.location.href;
  afterEach(() => window.history.replaceState(null, "", original));

  it("removes the flag so a refresh doesn't re-run the share handler", () => {
    window.history.replaceState(null, "", "/accounts?shared=1");
    clearShareFlag();
    expect(window.location.search).toBe("");
    expect(window.location.pathname).toBe("/accounts");
  });

  it("preserves other query params", () => {
    window.history.replaceState(null, "", "/?tab=budget&shared=1");
    clearShareFlag();
    expect(window.location.search).toBe("?tab=budget");
  });

  it("is a no-op when no flag is present", () => {
    window.history.replaceState(null, "", "/goals");
    clearShareFlag();
    expect(window.location.pathname).toBe("/goals");
  });
});

describe("takeSharedFile", () => {
  it("returns the shared CSV as a File with its name and type intact", async () => {
    await stash(csvRecord());
    const file = await takeSharedFile();
    expect(file).not.toBeNull();
    expect(file!.name).toBe("statement.csv");
    expect(file!.type).toBe("text/csv");
    expect(await readText(file!)).toContain("LOBLAWS");
  });

  // The one that protects against a double import of a bank statement.
  it("consumes the record, so a second call finds nothing", async () => {
    await stash(csvRecord());
    expect(await takeSharedFile()).not.toBeNull();
    expect(await takeSharedFile()).toBeNull();
  });

  it("returns null when nothing was shared", async () => {
    expect(await takeSharedFile()).toBeNull();
  });

  it("falls back to a sensible filename when the OS supplied none", async () => {
    await stash({ ...csvRecord(), name: "", type: "" });
    const file = await takeSharedFile();
    expect(file!.name).toBe("shared.csv");
    expect(file!.type).toBe("text/csv");
  });

  it("returns null instead of throwing when IndexedDB is unavailable", async () => {
    // Private browsing / storage disabled.
    globalThis.indexedDB = undefined as unknown as IDBFactory;
    expect(await takeSharedFile()).toBeNull();
  });

  it("refuses a record older than the TTL, and still consumes it", async () => {
    await stash({ ...csvRecord(), receivedAt: Date.now() - SHARE_MAX_AGE_MS - 1 });
    expect(await takeSharedFile()).toBeNull();
    // Must not be left behind for a later caller to pick up.
    expect(await peek()).toBeUndefined();
  });

  it("refuses a record with no usable timestamp rather than trusting it", async () => {
    await stash({ ...csvRecord(), receivedAt: Number.NaN });
    expect(await takeSharedFile()).toBeNull();
  });
});

// A share is answered by the service worker with no knowledge of whether anyone
// is signed in. If it lands while logged out, ShareTargetImport never mounts
// (AuthGate renders the login screen instead of the app), so nothing claims the
// file — these are what stop a bank statement living in cleartext forever.
describe("purging unclaimed shares", () => {
  it("purgeSharedFiles removes a parked file even while it is still fresh", async () => {
    await stash(csvRecord());
    await purgeSharedFiles();
    expect(await peek()).toBeUndefined();
  });

  it("purgeSharedFiles is a no-op when nothing is parked", async () => {
    await expect(purgeSharedFiles()).resolves.toBeUndefined();
  });

  it("sweepStaleSharedFiles drops an expired file", async () => {
    await stash({ ...csvRecord(), receivedAt: Date.now() - SHARE_MAX_AGE_MS - 1 });
    await sweepStaleSharedFiles();
    expect(await peek()).toBeUndefined();
  });

  // The sweep runs at every app boot, including the boot that is about to
  // legitimately consume the share — it must not eat it.
  it("sweepStaleSharedFiles leaves a fresh file alone", async () => {
    await stash(csvRecord());
    await sweepStaleSharedFiles();
    expect(await peek()).toBeDefined();
    expect(await takeSharedFile()).not.toBeNull();
  });

  it("neither call throws when IndexedDB is unavailable", async () => {
    globalThis.indexedDB = undefined as unknown as IDBFactory;
    await expect(purgeSharedFiles()).resolves.toBeUndefined();
    await expect(sweepStaleSharedFiles()).resolves.toBeUndefined();
  });
});

// The service worker is plain JS outside the bundle and cannot import the
// constants above, so the two files agree only by convention. Pin it.
describe("service worker contract", () => {
  it("uses the same IndexedDB database, store, and key as this module", () => {
    expect(swSource).toContain(`"${SHARE_DB_NAME}"`);
    expect(swSource).toContain(`"${SHARE_STORE}"`);
    expect(swSource).toContain(`"${SHARE_KEY}"`);
  });

  it("reads the same multipart field name the manifest declares and the API expects", () => {
    expect(swSource).toContain('form.get("file")');
  });

  it("redirects with the flags readShareFlag understands", () => {
    expect(swSource).toContain("/?shared=1");
    expect(swSource).toContain("/?shared=empty");
    expect(swSource).toContain("/?shared=error");
  });

  it("only intercepts POSTs to the share-target action, leaving other fetches to workbox", () => {
    expect(swSource).toContain('request.method !== "POST"');
    expect(swSource).toContain('url.pathname !== "/share-target"');
  });

  it("uses a 303 redirect so a reload cannot re-submit the share", () => {
    expect(swSource).toContain("303");
  });
});

// Without these the OS never offers FinSight in the share sheet at all, and the
// failure is silent — nothing errors, the app just never appears as a target.
describe("web app manifest declares the share target", () => {
  it("points the share action at the path the service worker intercepts", () => {
    expect(viteConfigSource).toContain("share_target");
    expect(viteConfigSource).toContain('action: "/share-target"');
    expect(viteConfigSource).toContain('method: "POST"');
    expect(viteConfigSource).toContain('enctype: "multipart/form-data"');
  });

  it("accepts CSV, including the MIME aliases Android actually sends", () => {
    expect(viteConfigSource).toContain('"text/csv"');
    expect(viteConfigSource).toContain('"application/vnd.ms-excel"');
    expect(viteConfigSource).toContain('".csv"');
  });

  it("pulls the share-target worker into the generated service worker", () => {
    expect(viteConfigSource).toContain('"share-target-sw.js"');
    expect(viteConfigSource).toContain("importScripts:");
  });
});

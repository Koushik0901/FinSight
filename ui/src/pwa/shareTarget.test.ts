import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
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
  SHARE_CRYPTO_KEY,
  SHARE_MAX_AGE_MS,
  MAX_SHARE_MB,
  clearShareFlag,
  purgeSharedFiles,
  readShareFlag,
  sweepStaleSharedFiles,
  takeSharedFile,
  type StoredShare,
} from "./shareTarget";

// jsdom ships no SubtleCrypto, but vitest exposes the platform's real
// WebCrypto — so these exercise actual AES-GCM, not a stub.
const realCrypto = globalThis.crypto;

// jsdom has no IndexedDB; fake-indexeddb gives real transaction semantics, so
// the read-and-delete-in-one-transaction behaviour is genuinely exercised.
beforeEach(() => {
  globalThis.indexedDB = new IDBFactory();
  vi.stubGlobal("crypto", realCrypto);
});

async function openDb(): Promise<IDBDatabase> {
  return new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(SHARE_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function put(value: unknown, key: string): Promise<void> {
  const db = await openDb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, "readwrite");
    tx.objectStore(SHARE_STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
  db.close();
}

/**
 * Park a share exactly the way the service worker does: a non-extractable key
 * in the store, and the file sealed in the same envelope layout. If this and
 * share-target-sw.js ever disagree, these tests break — which is the point.
 */
async function stash(
  file: { name: string; type: string; text: string },
  receivedAt = Date.now()
): Promise<void> {
  return stashBytes(file.name, new TextEncoder().encode(file.text), receivedAt, file.type);
}

/** Byte-level variant, for content that is not valid UTF-8. */
async function stashBytes(
  name: string,
  body: Uint8Array,
  receivedAt = Date.now(),
  type = "text/csv"
): Promise<void> {
  let key = await readKey();
  if (!key) {
    key = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, [
      "encrypt",
      "decrypt",
    ]);
    await put(key, SHARE_CRYPTO_KEY);
  }

  const header = new TextEncoder().encode(JSON.stringify({ name, type }));
  const plain = new Uint8Array(4 + header.length + body.length);
  new DataView(plain.buffer).setUint32(0, header.length, false);
  plain.set(header, 4);
  plain.set(body, 4 + header.length);

  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, plain);
  await put({ v: 1, iv, ct, receivedAt }, SHARE_KEY);
}

async function readKey(): Promise<CryptoKey | null> {
  const db = await openDb();
  const key = await new Promise<CryptoKey | null>((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, "readonly");
    const req = tx.objectStore(SHARE_STORE).get(SHARE_CRYPTO_KEY);
    req.onsuccess = () => resolve((req.result as CryptoKey | undefined) ?? null);
    tx.onerror = () => reject(tx.error);
  });
  db.close();
  return key;
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

function readArrayBuffer(file: File): Promise<ArrayBuffer> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(file);
  });
}

function csvFile(text = "Date,Merchant,Amount\n2026-01-02,SYNTHETIC MERCHANT,-42.10\n") {
  return { name: "statement.csv", type: "text/csv", text };
}

/**
 * Deliberately varied shapes a brand-new user could hand us. Names, currencies,
 * separators, scripts, and column orders here are all invented — nothing in the
 * implementation may depend on any of them, and these exist to prove that.
 */
const SYNTHETIC_SHARES = [
  {
    label: "comma-separated, negative amounts",
    name: "export.csv",
    text: "Date,Description,Amount\n2026-03-01,MERCHANT A,-12.34\n",
  },
  {
    label: "semicolon-separated with comma decimals (common in the EU)",
    name: "kontoauszug.csv",
    text: "Datum;Beschreibung;Betrag\n01.03.2026;HÄNDLER;-1.234,56\n",
  },
  {
    label: "non-Latin script and a unicode filename",
    name: "取引明細.csv",
    text: "日付,摘要,金額\n2026-03-01,店舗,-1200\n",
  },
  {
    label: "separate debit/credit columns, no sign",
    name: "statement.CSV",
    text: "Posted,Payee,Debit,Credit\n2026/03/01,PAYEE,50.00,\n",
  },
  {
    label: "quoted fields containing commas and newlines",
    name: "quoted.csv",
    text: 'Date,Description,Amount\n2026-03-01,"MERCHANT, INC.\nSUITE 2",-9.99\n',
  },
  {
    label: "single header row and no transactions at all",
    name: "empty-but-valid.csv",
    text: "Date,Description,Amount\n",
  },
  {
    label: "very long single field",
    name: "long.csv",
    text: `Date,Description,Amount\n2026-03-01,${"X".repeat(5000)},-1.00\n`,
  },
] as const;

/** Look at the parked record WITHOUT consuming it. */
async function peek(): Promise<StoredShare | undefined> {
  const db = await openDb();
  const record = await new Promise<StoredShare | undefined>((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, "readonly");
    const req = tx.objectStore(SHARE_STORE).get(SHARE_KEY);
    req.onsuccess = () => resolve(req.result as StoredShare | undefined);
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

  it("maps the rejection outcomes the worker screens for", () => {
    expect(readShareFlag("?shared=toolarge")).toBe("toolarge");
    expect(readShareFlag("?shared=unsupported")).toBe("unsupported");
  });

  it("is 'none' for a normal launch", () => {
    expect(readShareFlag("")).toBe("none");
    expect(readShareFlag("?tab=budget")).toBe("none");
  });

  // A page from an older build can be live while a newer service worker is
  // already active, so unknown values must degrade to "nothing happened"
  // rather than throwing on a launch the user did not initiate.
  it("treats an unrecognised value as a normal launch", () => {
    expect(readShareFlag("?shared=bogus")).toBe("none");
    expect(readShareFlag("?shared=")).toBe("none");
    expect(readShareFlag("?shared=1&shared=empty")).toBe("file");
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
    await stash(csvFile());
    const file = await takeSharedFile();
    expect(file).not.toBeNull();
    expect(file!.name).toBe("statement.csv");
    expect(file!.type).toBe("text/csv");
    expect(await readText(file!)).toContain("SYNTHETIC MERCHANT");
  });

  // The one that protects against a double import of a bank statement.
  it("consumes the record, so a second call finds nothing", async () => {
    await stash(csvFile());
    expect(await takeSharedFile()).not.toBeNull();
    expect(await takeSharedFile()).toBeNull();
  });

  it("returns null when nothing was shared", async () => {
    expect(await takeSharedFile()).toBeNull();
  });

  it("falls back to a sensible filename when the OS supplied none", async () => {
    await stash({ name: "", type: "", text: "a,b\n" });
    const file = await takeSharedFile();
    expect(file!.name).toBe("shared.csv");
    expect(file!.type).toBe("text/csv");
  });

  it("returns null instead of throwing when IndexedDB is unavailable", async () => {
    // Private browsing / storage disabled.
    globalThis.indexedDB = undefined as unknown as IDBFactory;
    expect(await takeSharedFile()).toBeNull();
  });

  // The hand-off must be format-agnostic: it moves bytes, it does not parse.
  // Anything here that failed would mean the transport had opinions about
  // delimiters, encodings, or column layouts that it has no business having.
  describe.each(SYNTHETIC_SHARES)("round-trips $label", (sample) => {
    it("preserves the filename and every byte", async () => {
      await stash({ name: sample.name, type: "text/csv", text: sample.text });

      const file = await takeSharedFile();
      expect(file).not.toBeNull();
      expect(file!.name).toBe(sample.name);
      expect(await readText(file!)).toBe(sample.text);
    });
  });

  // A CSV is not guaranteed to be UTF-8 — Windows-1252 and UTF-16 exports are
  // both real. The transport must not corrupt bytes it cannot interpret.
  it("preserves bytes that are not valid UTF-8", async () => {
    const raw = new Uint8Array([0x44, 0x61, 0x74, 0x65, 0x0a, 0xff, 0xfe, 0x80, 0x00, 0x41]);
    await stashBytes("legacy-encoding.csv", raw);

    const file = await takeSharedFile();
    const out = new Uint8Array(await readArrayBuffer(file!));
    expect(Array.from(out)).toEqual(Array.from(raw));
  });

  it("refuses a record older than the TTL, and still consumes it", async () => {
    await stash(csvFile(), Date.now() - SHARE_MAX_AGE_MS - 1);
    expect(await takeSharedFile()).toBeNull();
    // Must not be left behind for a later caller to pick up.
    expect(await peek()).toBeUndefined();
  });

  it("refuses a record with no usable timestamp rather than trusting it", async () => {
    await stash(csvFile(), Number.NaN);
    expect(await takeSharedFile()).toBeNull();
  });
});

// A share is answered by the service worker with no knowledge of whether anyone
// is signed in. If it lands while logged out, ShareTargetImport never mounts
// (AuthGate renders the login screen instead of the app), so nothing claims the
// file — these are what stop a bank statement living in cleartext forever.
// The reason this feature encrypts at all: a share can sit unclaimed (parked
// while signed out, then the login screen is abandoned), and it is a whole bank
// statement. These are the assertions that would fail if it went back to
// storing raw bytes.
describe("the parked file is encrypted at rest", () => {
  it("stores no recognisable plaintext — not the rows, not the filename", async () => {
    await stash({
      name: "acct-1234-chequing.csv",
      type: "text/csv",
      text: "Date,Merchant,Amount\n2026-01-02,SYNTHETIC MERCHANT,-42.10\n",
    });

    const record = (await peek())!;
    const asText = new TextDecoder().decode(new Uint8Array(record.ct));
    expect(asText).not.toContain("SYNTHETIC MERCHANT");
    expect(asText).not.toContain("42.10");
    // The filename is itself a disclosure, so it goes inside the envelope too.
    expect(asText).not.toContain("acct-1234");
    expect(JSON.stringify(record)).not.toContain("acct-1234");
  });

  it("keeps the key non-extractable, so a storage dump cannot yield its bytes", async () => {
    await stash(csvFile());
    const key = (await readKey())!;
    expect(key.extractable).toBe(false);
    await expect(realCrypto.subtle.exportKey("raw", key)).rejects.toThrow();
  });

  it("leaves receivedAt outside the ciphertext so the sweep works without the key", async () => {
    const when = Date.now() - 1000;
    await stash(csvFile(), when);
    expect((await peek())!.receivedAt).toBe(when);
  });

  it("returns null on tampered ciphertext rather than trusting it", async () => {
    await stash(csvFile());
    const record = (await peek())!;
    const bytes = new Uint8Array(record.ct);
    bytes[0] = (bytes[0] ?? 0) ^ 0xff;
    await put({ ...record, ct: bytes.buffer }, SHARE_KEY);

    expect(await takeSharedFile()).toBeNull();
  });

  it("returns null once the key is destroyed, even if ciphertext survives", async () => {
    await stash(csvFile());
    const record = (await peek())!;
    await purgeSharedFiles();
    // Put the ciphertext back; without its key it must stay unreadable.
    await put(record, SHARE_KEY);

    expect(await takeSharedFile()).toBeNull();
  });

  it("returns null for an envelope from another version", async () => {
    await stash(csvFile());
    const record = (await peek())!;
    await put({ ...record, v: 99 }, SHARE_KEY);
    expect(await takeSharedFile()).toBeNull();
  });
});

describe("purging unclaimed shares", () => {
  it("purgeSharedFiles removes a parked file even while it is still fresh", async () => {
    await stash(csvFile());
    await purgeSharedFiles();
    expect(await peek()).toBeUndefined();
  });

  it("purgeSharedFiles crypto-shreds the key alongside the file", async () => {
    await stash(csvFile());
    expect(await readKey()).not.toBeNull();
    await purgeSharedFiles();
    expect(await readKey()).toBeNull();
  });

  it("purgeSharedFiles is a no-op when nothing is parked", async () => {
    await expect(purgeSharedFiles()).resolves.toBeUndefined();
  });

  it("sweepStaleSharedFiles drops an expired file", async () => {
    await stash(csvFile(), Date.now() - SHARE_MAX_AGE_MS - 1);
    await sweepStaleSharedFiles();
    expect(await peek()).toBeUndefined();
  });

  // The sweep runs at every app boot, including the boot that is about to
  // legitimately consume the share — it must not eat it.
  it("sweepStaleSharedFiles leaves a fresh file alone", async () => {
    await stash(csvFile());
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
  it("uses the same IndexedDB database, store, and record keys as this module", () => {
    expect(swSource).toContain(`"${SHARE_DB_NAME}"`);
    expect(swSource).toContain(`"${SHARE_STORE}"`);
    expect(swSource).toContain(`"${SHARE_KEY}"`);
    expect(swSource).toContain(`"${SHARE_CRYPTO_KEY}"`);
  });

  // If the worker ever stops encrypting, the app would still "work" — it would
  // just be storing bank statements in the clear again. Pin it at the source.
  it("encrypts before storing, with a non-extractable AES-GCM key", () => {
    // Whitespace-tolerant: the property is the call and its arguments, not how
    // the formatter happened to break the lines.
    expect(swSource).toMatch(/crypto\.subtle\s*\.\s*encrypt\(/);
    expect(swSource).toMatch(/crypto\.subtle\s*\.\s*generateKey\(/);
    // `false` is the non-extractability argument — the load-bearing part.
    expect(swSource).toMatch(
      /generateKey\(\s*\{\s*name:\s*"AES-GCM",\s*length:\s*256\s*\}\s*,\s*false/
    );
    // A fresh 12-byte IV per share.
    expect(swSource).toMatch(/getRandomValues\(\s*new Uint8Array\(12\)\s*\)/);
  });

  // These screens run before anything is read into memory or parked, so an
  // unusable share fails immediately instead of after an upload round trip.
  it("screens size and file type before parking anything", () => {
    expect(swSource).toContain("/?shared=toolarge");
    expect(swSource).toContain("/?shared=unsupported");
    // Mirrors MAX_CSV_UPLOAD_BYTES in crates/finsight-server/src/uploads.rs.
    expect(swSource).toContain("25 * 1024 * 1024");
    expect(swSource).toContain(`${MAX_SHARE_MB} * 1024 * 1024`);
    // Extension, not MIME — MIME for .csv is unreliable across platforms, and
    // the server's own check is on the extension too.
    expect(swSource).toMatch(/\\\.csv\$\/i/);
    // Size is checked against file.size, i.e. before arrayBuffer() is called.
    expect(swSource).toContain("file.size > FINSIGHT_MAX_SHARE_BYTES");
  });

  it("never writes the raw file buffer to storage", () => {
    // The pre-encryption implementation stashed `buffer: await file.arrayBuffer()`.
    expect(swSource).not.toMatch(/buffer:\s*await file\.arrayBuffer\(\)/);
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

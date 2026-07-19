/**
 * App-side half of the OS share-target flow.
 *
 * The service worker (ui/public/share-target-sw.js) answers the share sheet's
 * POST, parks the file in IndexedDB, and redirects here with a `?shared=` flag.
 * This module picks the file back up in the page, where the session cookie
 * actually applies, so the normal authenticated CSV import can run.
 *
 * The constants below are a CONTRACT with that service-worker file, which is
 * plain JS outside the bundle and so cannot import them. `shareTarget.test.ts`
 * pins both sides.
 */

export const SHARE_DB_NAME = "finsight-share-target";
export const SHARE_STORE = "incoming";
export const SHARE_KEY = "pending";
/** Non-extractable AES-GCM key the worker writes, in the same store. */
export const SHARE_CRYPTO_KEY = "key";
const SHARE_ENVELOPE_VERSION = 1;

/**
 * Size ceiling shown to the user, in MB. Mirrors `FINSIGHT_MAX_SHARE_BYTES` in
 * the service worker, which in turn mirrors `MAX_CSV_UPLOAD_BYTES` in
 * crates/finsight-server/src/uploads.rs — the authority. Kept as a plain number
 * so the message reads naturally in any locale.
 */
export const MAX_SHARE_MB = 25;

/**
 * How long a parked share may sit unclaimed before it is thrown away.
 *
 * This matters more than it looks. The service worker answers the share POST
 * with no idea whether anyone is signed in — SW fetch handlers run before any
 * app code. If the user shares while logged out, `AuthGate` renders the login
 * screen INSTEAD of the app, so `ShareTargetImport` never mounts and never
 * claims the file. Abandon the login screen and a full bank statement would
 * otherwise sit in cleartext IndexedDB indefinitely — the very thing the
 * encrypted query cache exists to prevent.
 *
 * A hand-off that works takes seconds, so fifteen minutes is generous.
 *
 * The TTL bounds the window; it is not the only protection. The parked file is
 * also ENCRYPTED at rest (see the service worker), so even inside that window,
 * and even if the sweep never gets to run because the app is never opened
 * again, a dump of browser storage yields ciphertext rather than a statement.
 */
export const SHARE_MAX_AGE_MS = 15 * 60 * 1000;

/**
 * What the service worker writes: an AES-GCM envelope.
 *
 * `receivedAt` is deliberately OUTSIDE the ciphertext so the TTL sweep can
 * discard a stale share without holding the key — the sweep must work even
 * when decryption would not.
 */
export type StoredShare = {
  v: number;
  iv: Uint8Array;
  ct: ArrayBuffer;
  receivedAt: number;
};

/** Outcome encoded in the redirect URL by the service worker. */
export type ShareOutcome =
  | "file"
  | "empty"
  | "toolarge"
  | "unsupported"
  | "error"
  | "none";

/**
 * Read `?shared=` from the current URL.
 *
 * `1` — a file is waiting in IndexedDB.
 * `empty` — the share carried no file, or a zero-byte one.
 * `toolarge` — over the server's upload limit; rejected before being read.
 * `unsupported` — not a CSV (a PDF statement, most likely).
 * `error` — the worker failed to park it.
 *
 * Anything else, including a value from a NEWER worker this build does not
 * know about, is treated as a normal launch rather than a crash. A stale page
 * and a fresh service worker can coexist during an update.
 */
export function readShareFlag(search: string = window.location.search): ShareOutcome {
  const value = new URLSearchParams(search).get("shared");
  if (value === "1") return "file";
  if (value === "empty") return "empty";
  if (value === "toolarge") return "toolarge";
  if (value === "unsupported") return "unsupported";
  if (value === "error") return "error";
  return "none";
}

/**
 * Strip the `?shared=` flag from the address bar without a navigation.
 *
 * Necessary, not cosmetic: leaving it there means a refresh re-enters the
 * share-handling path and shows the import UI again for a file that was already
 * consumed. Other query params are preserved.
 */
export function clearShareFlag(): void {
  try {
    const url = new URL(window.location.href);
    if (!url.searchParams.has("shared")) return;
    url.searchParams.delete("shared");
    window.history.replaceState(
      window.history.state,
      "",
      url.pathname + (url.searchParams.toString() ? `?${url.searchParams}` : "") + url.hash
    );
  } catch {
    // Non-fatal: a stale flag costs a duplicate prompt, not correctness.
  }
}

function openShareDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(SHARE_STORE)) {
        req.result.createObjectStore(SHARE_STORE);
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/**
 * Claim the shared file: read it, DELETE it, and return it as a `File`.
 *
 * Read-and-delete in ONE readwrite transaction so an interrupted launch cannot
 * leave a file that gets imported twice. The record is a one-shot hand-off, and
 * a duplicate import of a bank statement is a genuinely bad outcome — the
 * importer dedupes, but a second silent import is still not something to risk
 * on transaction timing.
 *
 * Returns null when nothing is waiting or IndexedDB is unavailable.
 */
export async function takeSharedFile(): Promise<File | null> {
  const record = await claimRecord();
  if (!record) return null;

  // Defence in depth alongside the boot-time sweep: never hand a caller a file
  // that has been sitting around, even if the sweep did not get to run.
  // Checked BEFORE decrypting — a stale share should not even be unwrapped.
  if (isExpired(record)) return null;

  return decryptShare(record);
}

/**
 * Unwrap the worker's envelope back into a File.
 *
 * Returns null on every failure — wrong/rotated key, tampered bytes, an
 * envelope from another version — which the caller already treats as "nothing
 * was shared". Never fall back to trusting the bytes.
 */
async function decryptShare(record: StoredShare): Promise<File | null> {
  if (record.v !== SHARE_ENVELOPE_VERSION) return null;
  if (typeof crypto === "undefined" || !crypto.subtle) return null;

  const key = await readShareKey();
  if (!key) return null;

  try {
    // Copy into this realm's views first: binary crossing a structured clone
    // (or a test runner's realm boundary) fails `instanceof` but is valid.
    const plainBuf = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: new Uint8Array(record.iv) },
      key,
      new Uint8Array(record.ct)
    );
    const plain = new Uint8Array(plainBuf);

    // [4-byte BE header length][UTF-8 JSON {name,type}][raw file bytes]
    const headerLen = new DataView(plain.buffer, plain.byteOffset, plain.byteLength).getUint32(
      0,
      false
    );
    if (headerLen <= 0 || headerLen + 4 > plain.length) return null;

    const header = JSON.parse(
      new TextDecoder().decode(plain.subarray(4, 4 + headerLen))
    ) as { name?: string; type?: string };
    const body = plain.subarray(4 + headerLen);

    return new File([body], header.name || "shared.csv", {
      type: header.type || "text/csv",
    });
  } catch {
    return null;
  }
}

/** The worker's key. The app only ever READS it — it never creates one. */
async function readShareKey(): Promise<CryptoKey | null> {
  let db: IDBDatabase | null = null;
  try {
    db = await openShareDb();
    const handle = db;
    return await new Promise<CryptoKey | null>((resolve, reject) => {
      const tx = handle.transaction(SHARE_STORE, "readonly");
      const req = tx.objectStore(SHARE_STORE).get(SHARE_CRYPTO_KEY);
      req.onsuccess = () => resolve((req.result as CryptoKey | undefined) ?? null);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } catch {
    return null;
  } finally {
    db?.close();
  }
}

function isExpired(record: StoredShare): boolean {
  // A missing/garbled timestamp is treated as expired: we cannot prove the file
  // is fresh, and refusing it costs one re-share while keeping it costs a
  // plaintext statement on disk.
  if (typeof record.receivedAt !== "number" || !Number.isFinite(record.receivedAt)) return true;
  return Date.now() - record.receivedAt > SHARE_MAX_AGE_MS;
}

/**
 * Read-and-delete the parked record in ONE readwrite transaction.
 *
 * Atomic on purpose: an interrupted launch must not leave a file that gets
 * imported twice. The importer dedupes, but a second silent import of a bank
 * statement is not something to risk on transaction timing.
 */
async function claimRecord(): Promise<StoredShare | undefined> {
  let db: IDBDatabase | null = null;
  try {
    db = await openShareDb();
    const handle = db;
    return await new Promise<StoredShare | undefined>((resolve, reject) => {
      const tx = handle.transaction(SHARE_STORE, "readwrite");
      const store = tx.objectStore(SHARE_STORE);
      const getReq = store.get(SHARE_KEY);
      getReq.onsuccess = () => {
        if (getReq.result) store.delete(SHARE_KEY);
      };
      tx.oncomplete = () => resolve(getReq.result as StoredShare | undefined);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } catch {
    return undefined;
  } finally {
    db?.close();
  }
}

/**
 * Throw away any parked share unconditionally.
 *
 * Called when a session ends AND when a new one begins. The second is not
 * redundant: on a shared device, one person can park a statement, abandon the
 * login screen, and someone else can then sign in — without this, that second
 * person's app would silently import the first person's bank statement. This
 * mirrors why `purgePersistedCache` already runs on both transitions.
 *
 * Destroys the encryption key alongside the file — crypto-shredding, same as
 * `purgeCacheKey`. Any ciphertext that outlives the delete (a browser-internal
 * copy, an unflushed page of the LevelDB log) becomes permanently
 * undecryptable rather than merely unlinked. The worker simply mints a new key
 * for the next share.
 */
export async function purgeSharedFiles(): Promise<void> {
  await claimRecord();

  let db: IDBDatabase | null = null;
  try {
    db = await openShareDb();
    const handle = db;
    await new Promise<void>((resolve, reject) => {
      const tx = handle.transaction(SHARE_STORE, "readwrite");
      tx.objectStore(SHARE_STORE).delete(SHARE_CRYPTO_KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } catch {
    // The file itself is already gone; losing the key delete is not fatal.
  } finally {
    db?.close();
  }
}

/**
 * Drop a parked share that has outlived `SHARE_MAX_AGE_MS`, leaving a fresh one
 * alone. Run at app boot REGARDLESS of auth state, so a share abandoned at the
 * login screen is cleaned up on the next launch rather than living forever.
 */
export async function sweepStaleSharedFiles(): Promise<void> {
  let db: IDBDatabase | null = null;
  try {
    db = await openShareDb();
    const handle = db;
    await new Promise<void>((resolve, reject) => {
      const tx = handle.transaction(SHARE_STORE, "readwrite");
      const store = tx.objectStore(SHARE_STORE);
      const getReq = store.get(SHARE_KEY);
      getReq.onsuccess = () => {
        // Reads only `receivedAt`, which the worker leaves outside the
        // ciphertext — the sweep must work without holding the key.
        const record = getReq.result as StoredShare | undefined;
        if (record && isExpired(record)) store.delete(SHARE_KEY);
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } catch {
    // Nothing actionable; the TTL check in takeSharedFile still applies.
  } finally {
    db?.close();
  }
}
